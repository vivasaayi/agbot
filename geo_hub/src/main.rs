use anyhow::{bail, Context};
use geo_hub::config::LogFormat;
use geo_hub::{catalog, db, serve, HubConfig};
use tracing::info;

/// Initialize tracing. `RUST_LOG` controls levels (default `info`); the config
/// selects text (default) or JSON output for log aggregation.
fn init_logging(config: &HubConfig) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = fmt().with_env_filter(filter);
    match config.observability.log_format {
        LogFormat::Json => builder.json().init(),
        LogFormat::Text => builder.init(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration (from GEO_HUB_* env vars or geo_hub.{toml}) before
    // logging so the log format can be configured.
    let config = HubConfig::load().context("failed to load hub config")?;
    init_logging(&config);
    config
        .ensure_data_dirs()
        .context("failed to create data directories")?;

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Database file management runs before opening a pool or bootstrapping:
    // restore replaces the database file, and bootstrap would otherwise
    // recreate/seed it first.
    match args.as_slice() {
        [cmd, dest] if cmd == "backup" => {
            let path = geo_hub::backup::backup_database(&config, std::path::Path::new(dest))
                .await
                .context("backup failed")?;
            println!("backup written to {}", path.display());
            return Ok(());
        }
        [cmd, src] if cmd == "restore" => {
            geo_hub::backup::restore_database(&config, std::path::Path::new(src))
                .context("restore failed")?;
            println!("restored database from {src}");
            return Ok(());
        }
        _ => {}
    }

    // Connect to database pool
    let pool = db::connect_pool(&config)
        .await
        .context("failed to connect database")?;

    // First-run bootstrap: seed a default org/account/access code on a fresh
    // server so the farmer PWA is usable immediately. No-op once provisioned.
    geo_hub::bootstrap::ensure_bootstrap(&pool, &config.bootstrap)
        .await
        .context("first-run bootstrap failed")?;

    // Subcommands: `geo_hub catalog register <dir>` walks product_record.json
    // sidecars and registers them; no args runs the server.
    match args.as_slice() {
        [] => {
            info!(
                bind = %config.bind_address,
                runtime_mode = %config.runtime_mode,
                landsat_source = %config.landsat.source,
                "starting geo_hub"
            );
            serve(config, pool).await
        }
        [cmd, sub, dir] if cmd == "catalog" && sub == "register" => {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let report = catalog::register_sidecar_dir(&pool, std::path::Path::new(dir), &now)
                .await
                .context("failed to register sidecar directory")?;
            println!(
                "registered {} product(s), {} failed",
                report.registered.len(),
                report.failed.len()
            );
            for (path, reason) in &report.failed {
                eprintln!("  FAILED {path}: {reason}");
            }
            if report.failed.is_empty() {
                Ok(())
            } else {
                bail!("{} sidecar(s) failed to register", report.failed.len())
            }
        }
        [cmd, sub, input, rest @ ..] if cmd == "sen2cor" && sub == "run" && rest.len() <= 1 => {
            let output_dir = match rest {
                [dir] => std::path::PathBuf::from(dir),
                _ => config.data_root.join("sen2cor"),
            };
            let outcome = geo_hub::sen2cor::run_sen2cor(
                &pool,
                &geo_hub::sen2cor::SystemProcessRunner,
                &geo_hub::sen2cor::Sen2CorConfig::from_env(),
                std::path::Path::new(input),
                &output_dir,
            )
            .await
            .context("sen2cor run failed")?;
            println!(
                "scene {}: L2A at {} — {} band product(s) registered (L0 {})",
                outcome.scene_id,
                outcome.l2a_safe.display(),
                outcome.band_products.len(),
                outcome.l0_product_id,
            );
            Ok(())
        }
        _ => {
            bail!("usage: geo_hub [catalog register <dir> | sen2cor run <input.SAFE> [output_dir] | backup <dest.db> | restore <src.db>]")
        }
    }
}
