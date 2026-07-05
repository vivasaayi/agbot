//! Sen2Cor L1C -> L2A atmospheric-correction orchestration (satellite
//! pipeline batch 13).
//!
//! Doctrine: **no in-process atmospheric correction** — Sen2Cor's
//! `L2A_Process` CLI runs as a subprocess (typically containerized), and this
//! module owns everything around it: input validation, command templating,
//! output-layout validation, and catalog registration with lineage and
//! run evidence. The subprocess boundary is an injectable [`ProcessRunner`]
//! so tests fabricate outputs without executing anything.
//!
//! Registration mirrors the Earth Search path (`satellite_derivation`): the
//! L1C scene registers as the L0 `raw_scene`, and every L2A band file
//! registers as an L1 `band_*` product with lineage to it, all through
//! `ingest_contract::commit_ingest`. Downstream index derivation can then
//! consume the bands like any other cataloged source. (The L2A JP2000 band
//! files are registered as artifacts; `raster_io` reads GeoTIFF/COG, so
//! JP2 decoding — or a `gdal_translate` step — is future work before these
//! bands feed the local index pipeline.)
//!
//! Runs take tens of minutes on a full scene, so the entry point is the
//! `geo_hub sen2cor run` CLI subcommand, not an HTTP route.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use thiserror::Error;

use crate::db::DbPool;
use crate::ingest_contract::{commit_ingest, IngestError, IngestScene, NormalizedIngest};

/// Source id stamped on Sen2Cor registrations.
pub const SEN2COR_SOURCE_ID: &str = "sen2cor:l2a";
/// Default command template; `{input}` / `{output_dir}` are substituted.
/// Override with `GEO_HUB_SEN2COR_COMMAND` (whitespace-split), e.g.
/// `docker run --rm -v /data:/data sen2cor:2.12 L2A_Process --output_dir {output_dir} {input}`.
pub const DEFAULT_SEN2COR_COMMAND: &str = "L2A_Process --output_dir {output_dir} {input}";
const ALGORITHM_VERSION: &str = "1.0.0";
/// How much stderr to keep in errors/evidence.
const STDERR_TAIL_BYTES: usize = 2048;

#[derive(Debug, Error)]
pub enum Sen2CorError {
    #[error("input {0} is not a .SAFE directory")]
    InputNotSafe(PathBuf),
    #[error("input {0} has no MTD_MSIL1C.xml — not a Sentinel-2 L1C product")]
    MissingL1cMetadata(PathBuf),
    #[error("SAFE name {0:?} does not parse as S2 (expected e.g. S2A_MSIL1C_YYYYMMDDTHHMMSS_...)")]
    BadSceneName(String),
    #[error("sen2cor command template is empty")]
    EmptyCommand,
    #[error("sen2cor exited with status {status:?}; stderr tail:\n{stderr_tail}")]
    CommandFailed {
        status: Option<i32>,
        stderr_tail: String,
    },
    #[error("sen2cor reported success but {output_dir} contains no L2A .SAFE (MTD_MSIL2A.xml)")]
    OutputMissing { output_dir: PathBuf },
    #[error("L2A output {0} has no band images under GRANULE/*/IMG_DATA")]
    NoBands(PathBuf),
    #[error("failed to launch sen2cor ({program}): {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog registration failed: {0}")]
    Ingest(#[from] IngestError),
}

// ---------------------------------------------------------------------------
// Subprocess seam
// ---------------------------------------------------------------------------

/// Captured subprocess result.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// Exit code; `None` when killed by a signal.
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }
}

/// Seam for running the sen2cor command so tests fabricate outputs without
/// executing anything (mirrors the fetcher/COG-resolver patterns).
pub trait ProcessRunner: Send + Sync {
    fn run(&self, program: &str, args: &[String]) -> Result<ProcessOutput, std::io::Error>;
}

/// Production runner: `std::process::Command`, capturing output.
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<ProcessOutput, std::io::Error> {
        let output = std::process::Command::new(program).args(args).output()?;
        Ok(ProcessOutput {
            status: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// Sen2Cor invocation configuration.
#[derive(Debug, Clone)]
pub struct Sen2CorConfig {
    /// Whitespace-split command template with `{input}` / `{output_dir}`
    /// placeholders.
    pub command: Vec<String>,
}

impl Sen2CorConfig {
    /// From `GEO_HUB_SEN2COR_COMMAND` or [`DEFAULT_SEN2COR_COMMAND`].
    pub fn from_env() -> Self {
        let template = std::env::var("GEO_HUB_SEN2COR_COMMAND")
            .unwrap_or_else(|_| DEFAULT_SEN2COR_COMMAND.to_string());
        Self {
            command: template.split_whitespace().map(str::to_string).collect(),
        }
    }

    /// Substitute placeholders and split into (program, args).
    pub fn render(
        &self,
        input: &Path,
        output_dir: &Path,
    ) -> Result<(String, Vec<String>), Sen2CorError> {
        let substitute = |token: &str| {
            token
                .replace("{input}", &input.to_string_lossy())
                .replace("{output_dir}", &output_dir.to_string_lossy())
        };
        let mut tokens = self.command.iter();
        let program = tokens.next().ok_or(Sen2CorError::EmptyCommand)?;
        Ok((substitute(program), tokens.map(|t| substitute(t)).collect()))
    }
}

// ---------------------------------------------------------------------------
// SAFE parsing / validation
// ---------------------------------------------------------------------------

/// Scene identity parsed from a Sentinel-2 SAFE directory name, e.g.
/// `S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649.SAFE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeSceneName {
    /// Full name without the `.SAFE` suffix (the scene id).
    pub scene_id: String,
    /// Platform token, e.g. `S2A`.
    pub platform: String,
    /// Sensing start as RFC3339 UTC.
    pub acquired_at: String,
    /// MGRS tile token, e.g. `T43PFN` (when present).
    pub tile: Option<String>,
}

/// Parse the SAFE naming convention. Only the fields registration needs are
/// extracted; unknown segments are tolerated.
pub fn parse_safe_name(dir_name: &str) -> Result<SafeSceneName, Sen2CorError> {
    let stem = dir_name
        .strip_suffix(".SAFE")
        .ok_or_else(|| Sen2CorError::BadSceneName(dir_name.to_string()))?;
    let segments: Vec<&str> = stem.split('_').collect();
    let bad = || Sen2CorError::BadSceneName(dir_name.to_string());
    if segments.len() < 3 || !segments[0].starts_with("S2") {
        return Err(bad());
    }
    let stamp = segments[2];
    if stamp.len() != 15 || stamp.as_bytes()[8] != b'T' {
        return Err(bad());
    }
    let datetime =
        chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S").map_err(|_| bad())?;
    Ok(SafeSceneName {
        scene_id: stem.to_string(),
        platform: segments[0].to_string(),
        acquired_at: format!("{}Z", datetime.format("%Y-%m-%dT%H:%M:%S")),
        tile: segments
            .iter()
            .find(|segment| {
                segment.len() == 6
                    && segment.starts_with('T')
                    && segment[1..3].chars().all(|c| c.is_ascii_digit())
            })
            .map(|segment| segment.to_string()),
    })
}

fn dir_name(path: &Path) -> Result<&str, Sen2CorError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Sen2CorError::InputNotSafe(path.to_path_buf()))
}

/// Validate an L1C input SAFE and parse its scene name.
pub fn validate_l1c_input(input: &Path) -> Result<SafeSceneName, Sen2CorError> {
    let name = dir_name(input)?;
    if !name.ends_with(".SAFE") || !input.is_dir() {
        return Err(Sen2CorError::InputNotSafe(input.to_path_buf()));
    }
    if !input.join("MTD_MSIL1C.xml").is_file() {
        return Err(Sen2CorError::MissingL1cMetadata(input.to_path_buf()));
    }
    parse_safe_name(name)
}

/// One discovered L2A band image.
#[derive(Debug, Clone, PartialEq)]
pub struct L2aBand {
    /// e.g. `B04_10m` (parsed from `*_B04_10m.jp2`).
    pub band_key: String,
    pub path: PathBuf,
    /// Ground sample distance parsed from the resolution suffix.
    pub gsd_m_per_px: Option<f64>,
}

/// Locate the L2A `.SAFE` produced under `output_dir` and enumerate its band
/// images (`GRANULE/*/IMG_DATA/**/*.jp2`), sorted by band key.
pub fn scan_l2a_output(output_dir: &Path) -> Result<(PathBuf, Vec<L2aBand>), Sen2CorError> {
    let io_err = |path: &Path, source: std::io::Error| Sen2CorError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut l2a_safe: Option<PathBuf> = None;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(output_dir)
        .map_err(|source| io_err(output_dir, source))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir()
            && entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".SAFE"))
            && entry.join("MTD_MSIL2A.xml").is_file()
        {
            l2a_safe = Some(entry);
            break;
        }
    }
    let l2a_safe = l2a_safe.ok_or_else(|| Sen2CorError::OutputMissing {
        output_dir: output_dir.to_path_buf(),
    })?;

    let mut bands: Vec<L2aBand> = Vec::new();
    let granule_root = l2a_safe.join("GRANULE");
    let granules: Vec<PathBuf> = std::fs::read_dir(&granule_root)
        .map_err(|source| io_err(&granule_root, source))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    for granule in granules {
        let img_data = granule.join("IMG_DATA");
        if !img_data.is_dir() {
            continue;
        }
        // Bands sit either directly in IMG_DATA or in R10m/R20m/R60m.
        let mut stack = vec![img_data];
        while let Some(dir) = stack.pop() {
            let mut children: Vec<PathBuf> = std::fs::read_dir(&dir)
                .map_err(|source| io_err(&dir, source))?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            children.sort();
            for child in children {
                if child.is_dir() {
                    stack.push(child);
                } else if child
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jp2"))
                {
                    if let Some(band_key) = band_key_from_filename(&child) {
                        let gsd = band_key
                            .rsplit('_')
                            .next()
                            .and_then(|res| res.strip_suffix('m'))
                            .and_then(|res| res.parse::<f64>().ok());
                        bands.push(L2aBand {
                            band_key,
                            path: child,
                            gsd_m_per_px: gsd,
                        });
                    }
                }
            }
        }
    }
    bands.sort_by(|a, b| a.band_key.cmp(&b.band_key));
    if bands.is_empty() {
        return Err(Sen2CorError::NoBands(l2a_safe));
    }
    Ok((l2a_safe, bands))
}

/// `..._B04_10m.jp2` -> `B04_10m`; `..._SCL_20m.jp2` -> `SCL_20m`. Files
/// without a recognizable band token (e.g. TCI previews keep `TCI_10m`).
fn band_key_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let segments: Vec<&str> = stem.split('_').collect();
    if segments.len() >= 2 {
        Some(segments[segments.len() - 2..].join("_"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

/// Outcome of one sen2cor run + registration.
#[derive(Debug, Clone)]
pub struct Sen2CorOutcome {
    pub scene_id: String,
    pub l2a_safe: PathBuf,
    pub l0_product_id: String,
    /// (band key, product id), band order.
    pub band_products: Vec<(String, String)>,
    /// Run evidence: command line, exit status, output digests.
    pub evidence: serde_json::Value,
}

/// Run sen2cor on one L1C SAFE and register the L2A output. Blocking
/// (subprocess + file scanning); callers on a runtime should wrap in
/// `spawn_blocking` — the CLI calls it directly.
pub async fn run_sen2cor(
    pool: &DbPool,
    runner: &dyn ProcessRunner,
    config: &Sen2CorConfig,
    input: &Path,
    output_dir: &Path,
) -> Result<Sen2CorOutcome, Sen2CorError> {
    let scene = validate_l1c_input(input)?;
    std::fs::create_dir_all(output_dir).map_err(|source| Sen2CorError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;

    let (program, args) = config.render(input, output_dir)?;
    let output = runner
        .run(&program, &args)
        .map_err(|source| Sen2CorError::Spawn {
            program: program.clone(),
            source,
        })?;
    if !output.success() {
        let tail_start = output.stderr.len().saturating_sub(STDERR_TAIL_BYTES);
        return Err(Sen2CorError::CommandFailed {
            status: output.status,
            stderr_tail: String::from_utf8_lossy(&output.stderr[tail_start..]).to_string(),
        });
    }

    let (l2a_safe, bands) = scan_l2a_output(output_dir)?;

    // --- Registration through the normalized ingest contract.
    let command_line = std::iter::once(program.clone())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    let evidence = serde_json::json!({
        "tool": "sen2cor",
        "command": command_line,
        "exit_status": output.status,
        "stdout_sha256": format!("{:x}", Sha256::digest(&output.stdout)),
        "stderr_sha256": format!("{:x}", Sha256::digest(&output.stderr)),
        "l2a_safe": l2a_safe.to_string_lossy(),
        "band_count": bands.len(),
    });

    let l1c_metadata = input.join("MTD_MSIL1C.xml");
    let scope = |scene: &SafeSceneName| ProductScope {
        farm_id: None,
        field_id: None,
        season_id: None,
        scene_id: Some(scene.scene_id.clone()),
        temporal_start: scene.acquired_at.clone(),
        temporal_end: scene.acquired_at.clone(),
    };
    let l0 = ProductRecordDraft {
        level: ProductLevel::L0,
        kind: "raw_scene".to_string(),
        algorithm_id: "sen2cor.l1c.input".to_string(),
        algorithm_version: ALGORITHM_VERSION.to_string(),
        parameters: serde_json::json!({
            "scene_id": scene.scene_id,
            "platform": scene.platform,
            "tile": scene.tile,
            "level": "L1C",
        }),
        inputs: Vec::new(),
        scope: scope(&scene),
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            format: "xml".to_string(),
            path: l1c_metadata.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(SEN2COR_SOURCE_ID.to_string()),
    };
    let l0_id = l0.product_id();

    let mut l1_products = Vec::new();
    let mut band_keys: BTreeMap<String, ProductRecordDraft> = BTreeMap::new();
    for band in &bands {
        let draft = ProductRecordDraft {
            level: ProductLevel::L1,
            kind: format!("band_{}", band.band_key.to_ascii_lowercase()),
            algorithm_id: "sen2cor.l2a_process".to_string(),
            algorithm_version: ALGORITHM_VERSION.to_string(),
            parameters: serde_json::json!({
                "scene_id": scene.scene_id,
                "band": band.band_key,
                "evidence": evidence,
            }),
            inputs: vec![ProductInputRef {
                product_id: l0_id.clone(),
                role: "raw_scene".to_string(),
            }],
            scope: scope(&scene),
            spatial_ref: None,
            gsd_m_per_px: band.gsd_m_per_px,
            artifact: Some(ProductArtifact {
                format: "jp2".to_string(),
                path: band.path.to_string_lossy().to_string(),
                checksum_sha256: None,
            }),
            quality_mask: None,
            confidence: None,
            confidence_method: None,
            quality_summary: None,
            evidence_digests: Vec::new(),
            source_id: Some(SEN2COR_SOURCE_ID.to_string()),
        };
        band_keys.insert(band.band_key.clone(), draft);
    }
    let mut band_products = Vec::new();
    for (band_key, draft) in band_keys {
        band_products.push((band_key, draft.product_id()));
        l1_products.push(draft);
    }

    let ingest = NormalizedIngest {
        source_id: SEN2COR_SOURCE_ID.to_string(),
        source_kind: "satellite".to_string(),
        platform: Some(scene.platform.clone()),
        sensor: Some("Sentinel-2 MSI".to_string()),
        source_config: Some(evidence.clone()),
        scene: Some(IngestScene {
            scene_id: scene.scene_id.clone(),
            owner: None,
            sensor: "sentinel-2-l2a-sen2cor".to_string(),
            acquired_at: scene.acquired_at.clone(),
            data_path: l2a_safe.to_string_lossy().to_string(),
            metadata_json: evidence.to_string(),
            cloud_cover: None,
        }),
        l0_products: vec![l0],
        l1_products,
        quality: None,
    };
    let actor = provenance::ActorIdentity::system("geo_hub:sen2cor");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    commit_ingest(pool, &ingest, &actor, &created_at).await?;

    Ok(Sen2CorOutcome {
        scene_id: scene.scene_id,
        l2a_safe,
        l0_product_id: l0_id,
        band_products,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_parsing_is_pinned() {
        let name = "S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649.SAFE";
        let parsed = parse_safe_name(name).unwrap();
        assert_eq!(
            parsed.scene_id,
            "S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649"
        );
        assert_eq!(parsed.platform, "S2A");
        assert_eq!(parsed.acquired_at, "2024-06-01T05:16:51Z");
        assert_eq!(parsed.tile.as_deref(), Some("T43PFN"));

        for bad in [
            "S2A_MSIL1C_20240601T051651",        // no .SAFE
            "LC08_L1TP_20240601.SAFE",           // not S2
            "S2A_MSIL1C_20241301T051651_X.SAFE", // month 13
            "S2A_MSIL1C_2024061T05165_X.SAFE",   // truncated stamp
        ] {
            assert!(parse_safe_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn command_template_substitutes_placeholders() {
        let config = Sen2CorConfig {
            command: "docker run --rm -v {input}:/in sen2cor L2A_Process --output_dir {output_dir} {input}"
                .split_whitespace()
                .map(str::to_string)
                .collect(),
        };
        let (program, args) = config
            .render(Path::new("/data/scene.SAFE"), Path::new("/out"))
            .unwrap();
        assert_eq!(program, "docker");
        assert_eq!(
            args,
            vec![
                "run",
                "--rm",
                "-v",
                "/data/scene.SAFE:/in",
                "sen2cor",
                "L2A_Process",
                "--output_dir",
                "/out",
                "/data/scene.SAFE"
            ]
        );
        assert!(matches!(
            Sen2CorConfig { command: vec![] }.render(Path::new("a"), Path::new("b")),
            Err(Sen2CorError::EmptyCommand)
        ));
    }

    #[test]
    fn band_keys_parse_from_l2a_filenames() {
        assert_eq!(
            band_key_from_filename(Path::new("T43PFN_20240601T051651_B04_10m.jp2")),
            Some("B04_10m".to_string())
        );
        assert_eq!(
            band_key_from_filename(Path::new("T43PFN_20240601T051651_SCL_20m.jp2")),
            Some("SCL_20m".to_string())
        );
    }
}
