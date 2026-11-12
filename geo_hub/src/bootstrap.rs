//! First-run bootstrap: seed a default org + account + portal access code (and
//! an optional demo farm/field) so the farmer PWA works out-of-the-box on a
//! fresh server.
//!
//! Seeding runs only when the `marketplace_accounts` table is empty, so it is a
//! true first-run operation and never mutates a deployment that already has
//! real accounts. All inserts use fixed IDs with `INSERT OR IGNORE`, making the
//! whole routine idempotent across restarts.

use crate::config::BootstrapConfig;
use crate::db::DbPool;
use crate::portal_auth::hash_token;
use anyhow::{Context, Result};
use tracing::{info, warn};

/// What a bootstrap attempt did, for logging and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapOutcome {
    /// Bootstrap disabled by config.
    Disabled,
    /// Accounts already exist; nothing seeded.
    AlreadyProvisioned,
    /// A fresh server was seeded. Carries the plaintext access code so the
    /// caller can surface it to the operator.
    Seeded { access_code: String },
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Shared field-boundary shape for the sample California Landsat area.
const DEMO_FIELD_BOUNDARY: &str = r#"{"crs":"EPSG:4326","coordinates":[{"longitude":-119.425,"latitude":36.775},{"longitude":-119.415,"latitude":36.775},{"longitude":-119.415,"latitude":36.785},{"longitude":-119.425,"latitude":36.785},{"longitude":-119.425,"latitude":36.775}]}"#;

/// Seed a fresh server if appropriate. Idempotent and safe to call on every
/// startup after migrations.
pub async fn ensure_bootstrap(pool: &DbPool, config: &BootstrapConfig) -> Result<BootstrapOutcome> {
    if !config.enabled {
        return Ok(BootstrapOutcome::Disabled);
    }

    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_accounts")
        .fetch_one(pool)
        .await
        .context("counting marketplace accounts")?;
    if account_count > 0 {
        return Ok(BootstrapOutcome::AlreadyProvisioned);
    }

    let now = now_rfc3339();

    // Default org + account. org_id is a free-text column (no orgs table), so
    // the account row is what materialises the org.
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO marketplace_accounts
            (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at)
        VALUES (?1, ?2, 'farmer', '[]', 'active', ?3, ?3)
        "#,
    )
    .bind(&config.account_id)
    .bind(&config.org_id)
    .bind(&now)
    .execute(pool)
    .await
    .context("seeding default account")?;

    // Portal access code (only the hash is stored). code_hash is UNIQUE, so a
    // repeat with the same plaintext is ignored rather than duplicated.
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO portal_access_codes
            (code_id, code_hash, account_id, org_id, label, created_at)
        VALUES (?1, ?2, ?3, ?4, 'bootstrap default', ?5)
        "#,
    )
    .bind(format!("portal-code-bootstrap-{}", config.account_id))
    .bind(hash_token(&config.access_code))
    .bind(&config.account_id)
    .bind(&config.org_id)
    .bind(&now)
    .execute(pool)
    .await
    .context("seeding default access code")?;

    if config.seed_demo {
        seed_demo_farm_field(pool, &config.org_id, &now)
            .await
            .context("seeding demo farm/field")?;
    }

    warn!(
        access_code = %config.access_code,
        org_id = %config.org_id,
        account_id = %config.account_id,
        "first-run bootstrap seeded a default account; log in at /portal with this access code and change it before exposing the server"
    );
    info!(
        "bootstrap: open /portal and sign in with access code `{}`",
        config.access_code
    );

    Ok(BootstrapOutcome::Seeded {
        access_code: config.access_code.clone(),
    })
}

/// Add the labelled sample field to an existing local account. Existing rows
/// remain intact and collisions belonging to another organization are rejected.
pub async fn seed_demo(pool: &DbPool, org_id: &str) -> Result<()> {
    seed_demo_farm_field(pool, org_id, &now_rfc3339()).await
}

async fn seed_demo_farm_field(pool: &DbPool, org_id: &str, now: &str) -> Result<()> {
    let boundary: shared::schemas::FieldBoundary = serde_json::from_str(DEMO_FIELD_BOUNDARY)?;
    shared::schemas::validate_field_boundary(&boundary)?;
    let mut tx = pool.begin().await?;
    let accounts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM marketplace_accounts WHERE org_id=? AND status='active'",
    )
    .bind(org_id)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        accounts > 0,
        "sample data requires an existing active account for organization {org_id}"
    );
    for (table, column, id) in [
        ("farms", "farm_id", "farm-demo"),
        ("fields", "field_id", "field-demo"),
    ] {
        let owner: Option<String> =
            sqlx::query_scalar(&format!("SELECT owner FROM {table} WHERE {column}=?"))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        anyhow::ensure!(
            owner.as_deref().is_none_or(|owner| owner == org_id),
            "sample ID {id} belongs to another organization"
        );
    }
    sqlx::query("INSERT OR IGNORE INTO farms (farm_id,owner,name,notes,status,created_at,updated_at) VALUES ('farm-demo',?,'Sample Landsat Farm','Sample area in California; satellite observations are real.','active',?,?)")
        .bind(org_id).bind(now).bind(now).execute(&mut *tx).await?;
    sqlx::query("INSERT OR IGNORE INTO fields (field_id,farm_id,owner,name,crop,season,notes,boundary_json,status,created_at,updated_at) VALUES ('field-demo','farm-demo',?,'Sample California Field',NULL,'2025','Sample boundary for exploring Landsat layers; crop unspecified.',?,'active',?,?)")
        .bind(org_id).bind(DEMO_FIELD_BOUNDARY).bind(now).bind(now).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HubConfig;
    use crate::db;
    use tempfile::TempDir;

    #[tokio::test]
    async fn demo_seed_supports_existing_accounts_and_is_repeatable() {
        let (_tmp, pool) = fresh_pool().await;
        let config = BootstrapConfig {
            seed_demo: false,
            ..Default::default()
        };
        ensure_bootstrap(&pool, &config).await.unwrap();
        seed_demo(&pool, &config.org_id).await.unwrap();
        sqlx::query("UPDATE fields SET name='My edited sample' WHERE field_id='field-demo'")
            .execute(&pool)
            .await
            .unwrap();
        seed_demo(&pool, &config.org_id).await.unwrap();
        let (name, boundary): (String, String) =
            sqlx::query_as("SELECT name,boundary_json FROM fields WHERE field_id='field-demo'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "My edited sample");
        let boundary: shared::schemas::FieldBoundary = serde_json::from_str(&boundary).unwrap();
        shared::schemas::validate_field_boundary(&boundary).unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fields")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn demo_seed_rolls_back_on_foreign_field_collision() {
        let (_tmp, pool) = fresh_pool().await;
        let config = BootstrapConfig {
            seed_demo: false,
            ..Default::default()
        };
        ensure_bootstrap(&pool, &config).await.unwrap();
        sqlx::query("INSERT INTO fields(field_id,owner,name,boundary_json,created_at) VALUES ('field-demo','other','Keep me','{}','now')").execute(&pool).await.unwrap();
        assert!(seed_demo(&pool, &config.org_id).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM farms")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    async fn fresh_pool() -> (TempDir, DbPool) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("geo_hub_test.db");
        let config = HubConfig {
            database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
            data_root: tmp.path().join("data"),
            ..HubConfig::default()
        };
        config.ensure_data_dirs().unwrap();
        let pool = db::connect_pool(&config).await.unwrap();
        (tmp, pool)
    }

    #[tokio::test]
    async fn seeds_a_fresh_server_and_is_idempotent() {
        let (_tmp, pool) = fresh_pool().await;
        let cfg = BootstrapConfig::default();

        let first = ensure_bootstrap(&pool, &cfg).await.unwrap();
        assert!(matches!(first, BootstrapOutcome::Seeded { .. }));

        // The access code must authenticate: its hash is stored.
        let stored: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM portal_access_codes WHERE code_hash = ?1")
                .bind(hash_token(&cfg.access_code))
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 1);

        // Demo farm + field seeded for the org.
        let farms: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM farms WHERE owner = ?1")
            .bind(&cfg.org_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(farms, 1);

        // Second run: already provisioned, no duplication.
        let second = ensure_bootstrap(&pool, &cfg).await.unwrap();
        assert_eq!(second, BootstrapOutcome::AlreadyProvisioned);
        let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(accounts, 1);
    }

    #[tokio::test]
    async fn skips_when_accounts_already_exist() {
        let (_tmp, pool) = fresh_pool().await;
        sqlx::query(
            "INSERT INTO marketplace_accounts (account_id, org_id, party_type, role_refs_json, status, created_at, updated_at) VALUES ('a','o','farmer','[]','active','t','t')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let outcome = ensure_bootstrap(&pool, &BootstrapConfig::default())
            .await
            .unwrap();
        assert_eq!(outcome, BootstrapOutcome::AlreadyProvisioned);
    }

    #[tokio::test]
    async fn disabled_config_does_nothing() {
        let (_tmp, pool) = fresh_pool().await;
        let cfg = BootstrapConfig {
            enabled: false,
            ..BootstrapConfig::default()
        };
        let outcome = ensure_bootstrap(&pool, &cfg).await.unwrap();
        assert_eq!(outcome, BootstrapOutcome::Disabled);
        let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(accounts, 0);
    }
}
