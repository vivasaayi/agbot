//! Integration test of the Sen2Cor L1C -> L2A orchestration (satellite
//! pipeline batch 13), fully subprocess-free: a fake [`ProcessRunner`]
//! fabricates the L2A `.SAFE` layout that `L2A_Process` would produce, and
//! the module must validate it, register the scene + band products through
//! the ingest contract, and trace L1 bands back to the L0 raw scene.

use anyhow::Result;
use geo_hub::catalog;
use geo_hub::sen2cor::{run_sen2cor, ProcessOutput, ProcessRunner, Sen2CorConfig, Sen2CorError};
use geo_hub::{db, HubConfig};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

const L1C_NAME: &str = "S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649.SAFE";
const L2A_NAME: &str = "S2A_MSIL2A_20240601T051651_N0510_R062_T43PFN_20240601T080000.SAFE";

async fn pool(tmp: &TempDir) -> Result<geo_hub::db::DbPool> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("sen2cor.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    Ok(db::connect_pool(&config).await?)
}

fn make_l1c_input(tmp: &TempDir) -> Result<PathBuf> {
    let input = tmp.path().join(L1C_NAME);
    std::fs::create_dir_all(&input)?;
    std::fs::write(input.join("MTD_MSIL1C.xml"), b"<l1c/>")?;
    Ok(input)
}

/// Fabricate the L2A SAFE skeleton sen2cor would write.
fn fabricate_l2a(output_dir: &Path) {
    let img_root = output_dir
        .join(L2A_NAME)
        .join("GRANULE")
        .join("L2A_T43PFN_A046739_20240601T051651")
        .join("IMG_DATA");
    for (res, bands) in [("R10m", vec!["B04", "B08"]), ("R20m", vec!["B11", "SCL"])] {
        let dir = img_root.join(res);
        std::fs::create_dir_all(&dir).unwrap();
        for band in bands {
            let suffix = res.trim_start_matches('R');
            std::fs::write(
                dir.join(format!("T43PFN_20240601T051651_{band}_{suffix}.jp2")),
                band.as_bytes(),
            )
            .unwrap();
        }
    }
    std::fs::write(output_dir.join(L2A_NAME).join("MTD_MSIL2A.xml"), b"<l2a/>").unwrap();
}

/// Runner that fabricates output (or not) and records its invocation.
struct FakeRunner {
    exit_status: Option<i32>,
    fabricate: bool,
    stderr: &'static [u8],
    calls: AtomicUsize,
    expected_output_dir: PathBuf,
}

impl ProcessRunner for FakeRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<ProcessOutput, std::io::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(program, "fake-l2a-process");
        assert!(
            args.iter().any(|a| a.ends_with(".SAFE")),
            "input must be substituted: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| Path::new(a) == self.expected_output_dir),
            "output dir must be substituted: {args:?}"
        );
        if self.fabricate {
            fabricate_l2a(&self.expected_output_dir);
        }
        Ok(ProcessOutput {
            status: self.exit_status,
            stdout: b"Progress[%]: 100.00 : Application terminated successfully.".to_vec(),
            stderr: self.stderr.to_vec(),
        })
    }
}

fn config() -> Sen2CorConfig {
    Sen2CorConfig {
        command: "fake-l2a-process --output_dir {output_dir} {input}"
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    }
}

#[tokio::test]
async fn successful_run_registers_scene_and_band_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let input = make_l1c_input(&tmp)?;
    let output_dir = tmp.path().join("out");
    let runner = FakeRunner {
        exit_status: Some(0),
        fabricate: true,
        stderr: b"",
        calls: AtomicUsize::new(0),
        expected_output_dir: output_dir.clone(),
    };

    let outcome = run_sen2cor(&pool, &runner, &config(), &input, &output_dir).await?;
    assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.scene_id, L1C_NAME.trim_end_matches(".SAFE"));
    assert!(outcome.l2a_safe.ends_with(L2A_NAME));
    let keys: Vec<&str> = outcome
        .band_products
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(keys, vec!["B04_10m", "B08_10m", "B11_20m", "SCL_20m"]);
    assert_eq!(outcome.evidence["exit_status"], 0);
    assert_eq!(outcome.evidence["band_count"], 4);

    // Catalog: L1 band products exist with jp2 artifacts, correct GSD, and
    // lineage to the L0 raw scene.
    let (b04_key, b04_id) = &outcome.band_products[0];
    assert_eq!(b04_key, "B04_10m");
    let b04 = catalog::get_product(&pool, b04_id).await?.expect("b04");
    assert_eq!(b04.kind, "band_b04_10m");
    assert_eq!(b04.format.as_deref(), Some("jp2"));
    assert_eq!(b04.gsd_m_per_px, Some(10.0));
    assert_eq!(b04.source_id.as_deref(), Some("sen2cor:l2a"));
    assert_eq!(b04.temporal_start.as_deref(), Some("2024-06-01T05:16:51Z"));
    let edges = catalog::trace_inputs(&pool, b04_id).await?;
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].input_product_id, outcome.l0_product_id);
    assert_eq!(edges[0].role, "raw_scene");

    // Idempotent: a second run re-registers the same content-addressed ids.
    let again = run_sen2cor(&pool, &runner, &config(), &input, &output_dir).await?;
    assert_eq!(again.band_products, outcome.band_products);
    assert_eq!(again.l0_product_id, outcome.l0_product_id);
    Ok(())
}

#[tokio::test]
async fn failure_modes_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let output_dir = tmp.path().join("out");

    // Not a .SAFE directory.
    let plain = tmp.path().join("scene");
    std::fs::create_dir_all(&plain)?;
    let runner = FakeRunner {
        exit_status: Some(0),
        fabricate: true,
        stderr: b"",
        calls: AtomicUsize::new(0),
        expected_output_dir: output_dir.clone(),
    };
    assert!(matches!(
        run_sen2cor(&pool, &runner, &config(), &plain, &output_dir).await,
        Err(Sen2CorError::InputNotSafe(_))
    ));

    // .SAFE without L1C metadata.
    let empty_safe = tmp.path().join(L1C_NAME);
    std::fs::create_dir_all(&empty_safe)?;
    assert!(matches!(
        run_sen2cor(&pool, &runner, &config(), &empty_safe, &output_dir).await,
        Err(Sen2CorError::MissingL1cMetadata(_))
    ));
    assert_eq!(
        runner.calls.load(Ordering::SeqCst),
        0,
        "invalid input must never launch the subprocess"
    );

    // Nonzero exit carries the stderr tail.
    let input = make_l1c_input(&tmp)?;
    let failing = FakeRunner {
        exit_status: Some(2),
        fabricate: false,
        stderr: b"L2A_Process: SEVERE: no GIPP configuration found",
        calls: AtomicUsize::new(0),
        expected_output_dir: output_dir.clone(),
    };
    match run_sen2cor(&pool, &failing, &config(), &input, &output_dir).await {
        Err(Sen2CorError::CommandFailed {
            status,
            stderr_tail,
        }) => {
            assert_eq!(status, Some(2));
            assert!(stderr_tail.contains("GIPP"), "{stderr_tail}");
        }
        other => panic!("expected CommandFailed, got {other:?}"),
    }

    // Exit 0 but no L2A output.
    let silent = FakeRunner {
        exit_status: Some(0),
        fabricate: false,
        stderr: b"",
        calls: AtomicUsize::new(0),
        expected_output_dir: tmp.path().join("out-empty"),
    };
    assert!(matches!(
        run_sen2cor(
            &pool,
            &silent,
            &config(),
            &input,
            &tmp.path().join("out-empty")
        )
        .await,
        Err(Sen2CorError::OutputMissing { .. })
    ));
    Ok(())
}
