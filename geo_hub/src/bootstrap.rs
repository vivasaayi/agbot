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

/// A minimal but valid GeoJSON polygon for the demo field boundary (a ~200 m
/// square placed over farmland). Kept deterministic so seeded state is stable.
const DEMO_FIELD_BOUNDARY: &str = r#"{"type":"Polygon","coordinates":[[[-93.640,41.990],[-93.638,41.990],[-93.638,41.992],[-93.640,41.992],[-93.640,41.990]]]}"#;

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

async fn seed_demo_farm_field(pool: &DbPool, org_id: &str, now: &str) -> Result<()> {
    let farm_id = "farm-demo";
    let field_id = "field-demo";

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO farms
            (farm_id, owner, name, notes, status, created_at, updated_at)
        VALUES (?1, ?2, 'Home Farm', 'Seeded demo farm', 'active', ?3, ?3)
        "#,
    )
    .bind(farm_id)
    .bind(org_id)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO fields
            (field_id, farm_id, owner, name, crop, season, notes, boundary_json, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'North Field', 'corn', '2026', 'Seeded demo field', ?4, 'active', ?5, ?5)
        "#,
    )
    .bind(field_id)
    .bind(farm_id)
    .bind(org_id)
    .bind(DEMO_FIELD_BOUNDARY)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HubConfig;
    use crate::db;
    use tempfile::TempDir;

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
