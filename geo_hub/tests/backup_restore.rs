//! `geo_hub backup` / `restore` round-trip (batch 3.3b): a snapshot taken with
//! VACUUM INTO restores the exact database contents, and restore refuses a
//! non-SQLite file before clobbering the live database.

use anyhow::Result;
use geo_hub::backup::{backup_database, restore_database};
use geo_hub::{db, HubConfig};
use tempfile::TempDir;

fn config_for(db_path: &std::path::Path, data_root: &std::path::Path) -> HubConfig {
    HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: data_root.to_path_buf(),
        ..HubConfig::default()
    }
}

async fn account_count(config: &HubConfig) -> Result<i64> {
    let pool = db::connect_pool(config).await?;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM marketplace_accounts")
        .fetch_one(&pool)
        .await?;
    pool.close().await;
    Ok(count)
}

async fn insert_account(config: &HubConfig, account_id: &str) -> Result<()> {
    let pool = db::connect_pool(config).await?;
    sqlx::query(
        r#"
        INSERT INTO marketplace_accounts
            (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at)
        VALUES (?1, 'org-1', 'farmer', '[]', 'active', '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z')
        "#,
    )
    .bind(account_id)
    .execute(&pool)
    .await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn backup_then_restore_round_trips_the_database() -> Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("geo_hub.db");
    let backup_path = tmp.path().join("snapshot.db");
    let config = config_for(&db_path, &tmp.path().join("data"));
    config.ensure_data_dirs()?;

    // Seed one account, then snapshot.
    insert_account(&config, "acct-keep").await?;
    let written = backup_database(&config, &backup_path).await?;
    assert_eq!(written, backup_path);
    assert!(backup_path.exists(), "backup file must be written");

    // Mutate the live database out from under the snapshot.
    {
        let pool = db::connect_pool(&config).await?;
        sqlx::query("DELETE FROM marketplace_accounts")
            .execute(&pool)
            .await?;
        pool.close().await;
    }
    assert_eq!(
        account_count(&config).await?,
        0,
        "row deleted before restore"
    );

    // Restore brings the row back.
    restore_database(&config, &backup_path)?;
    assert_eq!(
        account_count(&config).await?,
        1,
        "restore must bring back the seeded account"
    );
    Ok(())
}

#[tokio::test]
async fn backup_refuses_to_overwrite_existing_destination() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = config_for(&tmp.path().join("geo_hub.db"), &tmp.path().join("data"));
    config.ensure_data_dirs()?;
    insert_account(&config, "acct-1").await?;

    let dest = tmp.path().join("exists.db");
    std::fs::write(&dest, b"do not clobber me")?;
    let err = backup_database(&config, &dest).await.unwrap_err();
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[tokio::test]
async fn restore_rejects_a_non_sqlite_file() -> Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("geo_hub.db");
    let config = config_for(&db_path, &tmp.path().join("data"));
    config.ensure_data_dirs()?;
    insert_account(&config, "acct-keep").await?;

    let bogus = tmp.path().join("not-a-db.txt");
    std::fs::write(&bogus, b"this is not a sqlite database")?;
    let err = restore_database(&config, &bogus).unwrap_err();
    assert!(
        err.to_string().contains("not a valid SQLite database"),
        "unexpected error: {err}"
    );

    // The live database is untouched.
    assert_eq!(account_count(&config).await?, 1);
    Ok(())
}
