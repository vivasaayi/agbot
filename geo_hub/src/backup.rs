//! SQLite backup / restore for the geo_hub appliance.
//!
//! Backup uses SQLite's `VACUUM INTO`, which writes a single consistent
//! snapshot file even while the database is in WAL mode and being read — so it
//! is safe to run against a live server. Restore copies a validated snapshot
//! back over the configured database file and clears any stale `-wal` / `-shm`
//! sidecars that would otherwise shadow the restored contents; run it with the
//! server stopped.
//!
//! Both operate on the file-backed `sqlite://` path from [`HubConfig`], so they
//! are no-ops for in-memory or non-sqlite URLs (which error clearly).

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sqlx::sqlite::SqlitePoolOptions;

use crate::config::HubConfig;

/// Leading bytes of every SQLite database file ("SQLite format 3\0").
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\x00";

/// Write a consistent snapshot of the configured database to `dest` using
/// `VACUUM INTO`. Fails if `dest` already exists (backups are never
/// overwritten) or if the database URL is not a file-backed sqlite path.
/// Returns the destination path.
pub async fn backup_database(config: &HubConfig, dest: &Path) -> Result<PathBuf> {
    let source = config
        .database_file_path()
        .context("backup requires a file-backed sqlite database_url")?;
    if !source.exists() {
        bail!("source database {} does not exist", source.display());
    }
    if dest.exists() {
        bail!(
            "backup destination {} already exists; refusing to overwrite",
            dest.display()
        );
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating backup directory {}", parent.display()))?;
        }
    }

    // Read-write open (the file exists) so WAL-mode databases attach cleanly;
    // VACUUM INTO does not modify the source.
    let url = format!("sqlite://{}?mode=rw", source.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .with_context(|| format!("opening source database {}", source.display()))?;

    // VACUUM INTO takes a string-literal target, not a bound parameter; escape
    // single quotes in the path defensively.
    let escaped = dest.to_string_lossy().replace('\'', "''");
    let result = sqlx::query(&format!("VACUUM INTO '{escaped}'"))
        .execute(&pool)
        .await
        .context("VACUUM INTO failed");
    pool.close().await;
    result?;

    Ok(dest.to_path_buf())
}

/// Replace the configured database file with a validated snapshot from
/// `source`. Run with the server stopped. Clears `-wal` / `-shm` sidecars so
/// the restored file is authoritative.
pub fn restore_database(config: &HubConfig, source: &Path) -> Result<()> {
    let target = config
        .database_file_path()
        .context("restore requires a file-backed sqlite database_url")?;
    validate_sqlite_file(source)?;

    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating database directory {}", parent.display()))?;
        }
    }

    std::fs::copy(source, &target)
        .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;

    // A leftover WAL/SHM from the previous database would shadow the restored
    // file on next open; remove them best-effort.
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", target.display()));
        if sidecar.exists() {
            let _ = std::fs::remove_file(&sidecar);
        }
    }
    Ok(())
}

/// Confirm `path` exists and begins with the SQLite file magic, so restore
/// refuses a truncated or wrong-type file before clobbering the live database.
fn validate_sqlite_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("backup file {} does not exist", path.display());
    }
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("opening backup file {}", path.display()))?;
    let mut header = [0u8; 16];
    file.read_exact(&mut header)
        .with_context(|| format!("reading header of {}", path.display()))?;
    if &header != SQLITE_MAGIC {
        bail!(
            "{} is not a valid SQLite database (bad header)",
            path.display()
        );
    }
    Ok(())
}
