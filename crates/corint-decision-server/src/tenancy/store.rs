//! Shared control state and platform credentials. Runtime operations bind deployment scope.
#[cfg(test)]
#[path = "store_credential_tests.rs"]
mod credential_tests;
use super::{
    config::Scope,
    queries::{self, Operation},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{any::AnyPoolOptions, AnyPool, ConnectOptions, Row};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoreConfig {
    Sqlite { path: PathBuf },
    Postgres { url_env: String, schema: String },
}
#[derive(Clone)]
pub struct Store {
    pool: AnyPool,
    scopes: BTreeSet<Scope>,
}
#[derive(Serialize)]
pub struct RuntimeState {
    pub paused: bool,
    pub revision: i64,
}
impl Store {
    pub async fn open(root: &Path, config: &StoreConfig, scopes: &BTreeSet<Scope>) -> Result<Self> {
        let root = root.canonicalize()?;
        for scope in scopes {
            scope.validate()?;
        }
        sqlx::any::install_default_drivers();
        let pool = match config {
            StoreConfig::Sqlite { path } => {
                ensure!(
                    !path.as_os_str().is_empty() && path != Path::new(":memory:"),
                    "Control store requires a persistent file"
                );
                let path = root.join(path);
                ensure!(
                    path.parent().unwrap().canonicalize()?.starts_with(&root),
                    "Control store escapes platform root"
                );
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    ensure!(
                        meta.is_file() && !meta.file_type().is_symlink(),
                        "Control store must be a regular file"
                    );
                }
                let url = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .to_url_lossy();
                let pool = AnyPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(5))
                    .after_connect(|c, _| {
                        Box::pin(async move {
                            for sql in [
                                "PRAGMA journal_mode=WAL",
                                "PRAGMA synchronous=FULL",
                                "PRAGMA busy_timeout=5000",
                            ] {
                                sqlx::query(sql).execute(&mut *c).await?;
                            }
                            Ok(())
                        })
                    })
                    .connect(url.as_str())
                    .await?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
                }
                pool
            }
            StoreConfig::Postgres { url_env, schema } => {
                ensure!(
                    !schema.is_empty()
                        && schema.len() <= 63
                        && schema != "public"
                        && !schema.starts_with("pg_")
                        && schema.bytes().enumerate().all(|(i, b)| b == b'_'
                            || b.is_ascii_lowercase()
                            || i > 0 && b.is_ascii_digit()),
                    "Invalid dedicated control schema"
                );
                let url = std::env::var(url_env)
                    .map_err(|_| anyhow::anyhow!("Missing control store credential"))?;
                ensure!(
                    url.starts_with("postgres://") || url.starts_with("postgresql://"),
                    "Control store requires PostgreSQL"
                );
                let setup = format!("SET search_path TO \"{schema}\", pg_catalog");
                let pool = AnyPoolOptions::new()
                    .max_connections(4)
                    .acquire_timeout(Duration::from_secs(5))
                    .after_connect(move |c, _| {
                        let setup = setup.clone();
                        Box::pin(async move {
                            sqlx::query(&setup).execute(&mut *c).await?;
                            sqlx::query("SET statement_timeout='5s'")
                                .execute(&mut *c)
                                .await?;
                            sqlx::query("SET synchronous_commit=on")
                                .execute(&mut *c)
                                .await?;
                            Ok(())
                        })
                    })
                    .connect(&url)
                    .await
                    .map_err(|_| anyhow::anyhow!("Control store connection failed"))?;
                pool
            }
        };
        let mut tx = pool.begin().await?;
        if let StoreConfig::Postgres { schema, .. } = config {
            sqlx::query("DO $$ BEGIN PERFORM pg_advisory_xact_lock(1283497392); END $$")
                .execute(&mut *tx)
                .await?;
            sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\""))
                .execute(&mut *tx)
                .await?;
        }
        // Names are private to this dedicated control store; data is always scoped.
        sqlx::query("CREATE TABLE IF NOT EXISTS tenant_runtimes (scope_key TEXT PRIMARY KEY, paused BIGINT NOT NULL, revision BIGINT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS tenant_management_audit (scope_key TEXT NOT NULL, revision BIGINT NOT NULL, event TEXT NOT NULL, PRIMARY KEY(scope_key,revision))").execute(&mut *tx).await?;
        // Platform-only credential registry. A single versioned document makes reads
        // atomic across SQLite and PostgreSQL; it contains hashes, never bearer tokens.
        sqlx::query("CREATE TABLE IF NOT EXISTS tenant_credentials (id BIGINT PRIMARY KEY CHECK(id=1), revision BIGINT NOT NULL, document TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS tenant_credential_audit (revision BIGINT PRIMARY KEY, actor TEXT NOT NULL, action TEXT NOT NULL, principal_id TEXT NOT NULL, occurred_at_ms BIGINT NOT NULL)").execute(&mut *tx).await?;
        Self::migrate_tenants(&mut tx, matches!(config, StoreConfig::Postgres { .. })).await?;
        for scope in scopes {
            sqlx::query("INSERT INTO tenant_runtimes(tenant_id,scope_key,paused,revision) VALUES($1,$2,0,0) ON CONFLICT DO NOTHING")
                .bind(&scope.tenant_id).bind(scope.key())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Self {
            pool,
            scopes: scopes.clone(),
        })
    }
    pub(super) async fn credentials(&self) -> Result<Option<(i64, super::auth::Registry)>> {
        let started = std::time::Instant::now();
        let row = sqlx::query("SELECT revision,document FROM tenant_credentials WHERE id=1")
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| {
            let mut registry: super::auth::Registry =
                serde_json::from_str(&row.try_get::<String, _>("document")?)?;
            registry.validate_persisted()?;
            registry.confirmed_since(started);
            Ok((row.try_get("revision")?, registry))
        })
        .transpose()
    }
    pub(super) async fn bootstrap_credentials(
        &self,
        registry: &super::auth::Registry,
    ) -> Result<()> {
        sqlx::query("INSERT INTO tenant_credentials(id,revision,document) VALUES(1,0,$1) ON CONFLICT DO NOTHING")
            .bind(serde_json::to_string(registry)?).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn save_credentials(
        &self,
        revision: i64,
        registry: &super::auth::Registry,
        actor: &str,
        action: &str,
        principal_id: &str,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query("UPDATE tenant_credentials SET revision=revision+1,document=$1 WHERE id=1 AND revision=$2")
            .bind(serde_json::to_string(registry)?).bind(revision).execute(&mut *tx).await?.rows_affected();
        if changed == 0 {
            return Ok(false);
        }
        sqlx::query("INSERT INTO tenant_credential_audit(revision,actor,action,principal_id,occurred_at_ms) VALUES($1,$2,$3,$4,$5)")
            .bind(revision+1).bind(actor).bind(action).bind(principal_id).bind(chrono::Utc::now().timestamp_millis())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    /// One-time operator migration: decode the already persisted full scope,
    /// never infer an owner from the current caller or default to local.
    async fn migrate_tenants(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        postgres: bool,
    ) -> Result<()> {
        sqlx::query("CREATE TABLE IF NOT EXISTS tenant_control_schema (id BIGINT PRIMARY KEY CHECK(id=1), version BIGINT NOT NULL)").execute(&mut **tx).await?;
        let version: Option<i64> =
            sqlx::query_scalar("SELECT version FROM tenant_control_schema WHERE id=1")
                .fetch_optional(&mut **tx)
                .await?;
        if let Some(version) = version {
            ensure!(version == 1, "Unsupported tenant control schema");
            return Ok(());
        }
        for table in ["tenant_runtimes", "tenant_management_audit"] {
            sqlx::query(&format!(
                "ALTER TABLE {table} ADD COLUMN tenant_id TEXT NOT NULL DEFAULT ''"
            ))
            .execute(&mut **tx)
            .await?;
            let keys: Vec<String> =
                sqlx::query_scalar(&format!("SELECT DISTINCT scope_key FROM {table}"))
                    .fetch_all(&mut **tx)
                    .await?;
            for key in keys {
                let scope: Scope = serde_json::from_str(&key)?;
                scope.validate()?;
                sqlx::query(&format!(
                    "UPDATE {table} SET tenant_id=$1 WHERE tenant_id='' AND scope_key=$2"
                ))
                .bind(scope.tenant_id)
                .bind(key)
                .execute(&mut **tx)
                .await?;
            }
            if postgres {
                sqlx::query(&format!(
                    "ALTER TABLE {table} ALTER COLUMN tenant_id DROP DEFAULT"
                ))
                .execute(&mut **tx)
                .await?;
                sqlx::query(&format!("ALTER TABLE {table} ADD CONSTRAINT {table}_tenant_matches CHECK (tenant_id<>'' AND (scope_key::jsonb->>'tenant_id') IS NOT DISTINCT FROM tenant_id)")).execute(&mut **tx).await?;
            } else {
                for action in ["INSERT", "UPDATE"] {
                    sqlx::query(&format!("CREATE TRIGGER {table}_tenant_matches_{action} BEFORE {action} ON {table} WHEN NEW.tenant_id='' OR json_extract(NEW.scope_key,'$.tenant_id') IS NOT NEW.tenant_id BEGIN SELECT RAISE(ABORT,'scope tenant mismatch'); END")).execute(&mut **tx).await?;
                }
            }
        }
        sqlx::query("CREATE INDEX tenant_runtime_owner ON tenant_runtimes(tenant_id,scope_key)")
            .execute(&mut **tx)
            .await?;
        sqlx::query("CREATE INDEX tenant_audit_owner ON tenant_management_audit(tenant_id,scope_key,revision)").execute(&mut **tx).await?;
        sqlx::query("INSERT INTO tenant_control_schema VALUES(1,1)")
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    fn query<'q>(
        &self,
        scope: &Scope,
        operation: Operation,
    ) -> Result<sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>> {
        ensure!(
            self.scopes.contains(scope),
            "Unregistered database tenant scope"
        );
        queries::query(scope, operation)
    }
    pub async fn state(&self, scope: &Scope) -> Result<RuntimeState> {
        let row = self
            .query(scope, Operation::State)?
            .fetch_one(&self.pool)
            .await?;
        let (paused, revision) = (
            row.try_get::<i64, _>("paused")?,
            row.try_get::<i64, _>("revision")?,
        );
        Ok(RuntimeState {
            paused: paused != 0,
            revision,
        })
    }
    pub async fn set_paused(
        &self,
        scope: &Scope,
        expected: i64,
        paused: bool,
        actor: &Value,
    ) -> Result<bool> {
        ensure!(
            (0..i64::MAX).contains(&expected),
            "Invalid control revision"
        );
        ensure!(
            actor["tenant_id"] == scope.tenant_id
                && actor["environment"] == scope.environment
                && actor["deployment"] == scope.deployment,
            "Audit actor tenant scope mismatch"
        );
        let mut tx = self.pool.begin().await?;
        let n = self
            .query(scope, Operation::Pause)?
            .bind(i64::from(paused))
            .bind(expected)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if n == 0 {
            return Ok(false);
        }
        let event = json!({"scope":scope,"revision":expected+1,"paused":paused,"actor":actor,"at_ms":chrono::Utc::now().timestamp_millis()});
        self.query(scope, Operation::InsertAudit)?
            .bind(expected + 1)
            .bind(event.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn audit(&self, scope: &Scope) -> Result<Vec<Value>> {
        let rows = self
            .query(scope, Operation::Audit)?
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|r| Ok(serde_json::from_str(r.try_get("event")?)?))
            .collect()
    }
}
