//! Database-specific setup and transaction serialization. Journal owns the
//! shared decision, request-idempotency and export protocol above this layer.
use super::{contract, validate_decision_record, JournalBackend, JournalConfig};
use anyhow::{ensure, Context};
use serde_json::Value;
use sqlx::{
    any::AnyPoolOptions, sqlite::SqliteConnectOptions, Any, AnyPool, ConnectOptions, Row,
    Transaction,
};
use std::{path::Path, time::Duration};

#[derive(Clone)]
pub(super) struct Storage {
    pool: AnyPool,
    tenant: String,
    postgres: bool,
}

impl Storage {
    pub async fn open(root: &Path, config: &JournalConfig) -> anyhow::Result<Self> {
        sqlx::any::install_default_drivers();
        match &config.backend {
            JournalBackend::Sqlite {} => Self::sqlite(root, config).await,
            JournalBackend::Postgres { url_env, schema } => {
                ensure!(
                    config.path.as_os_str().is_empty(),
                    "PostgreSQL journal must not specify a SQLite path"
                );
                ensure!(
                    !schema.is_empty() && schema.len() <= 63 && schema != "public"
                        && !schema.starts_with("pg_")
                        && schema.bytes().enumerate().all(|(i, b)| b == b'_' || b.is_ascii_lowercase() || i > 0 && b.is_ascii_digit()),
                    "Journal schema must be a dedicated lowercase SQL identifier (not public or pg_*)"
                );
                ensure!(
                    !url_env.is_empty()
                        && url_env.len() <= 128
                        && url_env.bytes().enumerate().all(|(i, b)| b == b'_'
                            || b.is_ascii_alphabetic()
                            || i > 0 && b.is_ascii_digit()),
                    "Invalid PostgreSQL journal URL environment name"
                );
                let url = std::env::var(url_env)
                    .context("Missing PostgreSQL journal URL environment variable")?;
                ensure!(
                    url.starts_with("postgres://") || url.starts_with("postgresql://"),
                    "Journal URL must use PostgreSQL"
                );
                let search_path = format!("SET search_path TO \"{schema}\", pg_catalog");
                let pool = AnyPoolOptions::new()
                    .max_connections(8)
                    .acquire_timeout(Duration::from_secs(5))
                    .after_connect(move |conn, _| {
                        let search_path = search_path.clone();
                        Box::pin(async move {
                            sqlx::query(&search_path).execute(&mut *conn).await?;
                            sqlx::query("SET statement_timeout = '5s'")
                                .execute(&mut *conn)
                                .await?;
                            sqlx::query("SET lock_timeout = '5s'")
                                .execute(&mut *conn)
                                .await?;
                            Ok(())
                        })
                    })
                    .connect(&url)
                    .await
                    .map_err(|_| anyhow::anyhow!("Cannot connect to PostgreSQL journal"))?;
                let storage = Self {
                    pool,
                    tenant: config.tenant_id.clone(),
                    postgres: true,
                };
                storage.initialize_postgres(config, schema).await?;
                Ok(storage)
            }
        }
    }

    async fn sqlite(root: &Path, config: &JournalConfig) -> anyhow::Result<Self> {
        ensure!(
            !config.path.as_os_str().is_empty() && config.path != Path::new(":memory:"),
            "SQLite journal requires a persistent path"
        );
        let path = root.join(&config.path);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "Journal must be a regular file"
            );
        }
        let url = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .to_url_lossy();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|conn, _| {
                Box::pin(async move {
                    sqlx::query("PRAGMA busy_timeout=5000")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query("PRAGMA journal_mode=WAL")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query("PRAGMA synchronous=FULL")
                        .execute(&mut *conn)
                        .await?;
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
        let storage = Self {
            pool,
            tenant: config.tenant_id.clone(),
            postgres: false,
        };
        storage.initialize_sqlite(config).await?;
        Ok(storage)
    }

    pub fn name(&self) -> &'static str {
        if self.postgres {
            "postgres"
        } else {
            "sqlite"
        }
    }

    /// Bind the whole file/schema to one environment/deployment as well as tenant.
    /// An unscoped opener must never silently adopt a scoped journal.
    pub async fn bind_scope(&self, scope: Option<&serde_json::Value>) -> anyhow::Result<()> {
        let mut tx = self.begin().await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_scope (id BIGINT PRIMARY KEY CHECK(id=1), scope_key TEXT NOT NULL)").execute(&mut *tx).await?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT scope_key FROM journal_scope WHERE id=1")
                .fetch_optional(&mut *tx)
                .await?;
        let expected = scope.map(Value::to_string).unwrap_or_default();
        if let Some(existing) = existing {
            ensure!(existing == expected, "Journal deployment scope mismatch");
        } else {
            if scope.is_some() {
                let count: i64 = sqlx::query_scalar("SELECT record_count+(SELECT COUNT(*) FROM request_keys) FROM journal_usage WHERE id=1").fetch_one(&mut *tx).await?;
                ensure!(
                    count == 0,
                    "An existing unscoped journal requires an explicit migration"
                );
            }
            sqlx::query("INSERT INTO journal_scope VALUES(1,$1)")
                .bind(expected)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// All writers lock the same usage row before looking up or reserving keys.
    /// PostgreSQL row locks span processes/hosts, unlike an in-process mutex.
    /// Explicit isolation avoids stale reads if a role defaults to repeatable read.
    pub async fn begin(&self) -> anyhow::Result<Transaction<'static, Any>> {
        if !self.postgres {
            return Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?);
        }
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await?;
        // A user/role may default this to off. A durable response requires WAL flush.
        sqlx::query("SET LOCAL synchronous_commit = on")
            .execute(&mut *tx)
            .await?;
        self.query(super::queries::Operation::LockUsage)
            .fetch_one(&mut *tx)
            .await?;
        Ok(tx)
    }

    /// The only online query constructor: SQL is selected from a closed enum and
    /// the first parameter is always bound here, never by the business caller.
    pub fn query<'q>(
        &self,
        op: super::queries::Operation,
    ) -> sqlx::query::Query<'q, Any, sqlx::any::AnyArguments<'q>> {
        sqlx::query(op.sql(self.postgres)).bind(self.tenant.clone())
    }
    pub async fn status(&self, now: i64) -> anyhow::Result<sqlx::any::AnyRow> {
        Ok(self
            .query(super::queries::Operation::Status)
            .bind(now)
            .fetch_one(&self.pool)
            .await?)
    }

    async fn initialize_postgres(
        &self,
        config: &JournalConfig,
        schema: &str,
    ) -> anyhow::Result<()> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await?;
        sqlx::query("SET LOCAL synchronous_commit = on")
            .execute(&mut *tx)
            .await?;
        // Only schema initialization takes this database-wide lock. Concurrent
        // first starts cannot race CREATE SCHEMA/TABLE/INDEX or run migrations twice.
        sqlx::query("DO $$ BEGIN PERFORM pg_advisory_xact_lock(1283497391); END $$")
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\""))
            .execute(&mut *tx)
            .await?;
        // Never adopt an unrelated, populated schema or reset its accounting.
        let (relations, metadata): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*),COUNT(*) FILTER (WHERE c.relname='journal_meta' AND c.relkind='r') FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1"
        ).bind(schema).fetch_one(&mut *tx).await?;
        ensure!(
            relations == 0 || metadata == 1,
            "PostgreSQL journal requires an empty schema or an existing Journal schema"
        );
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_meta (id BIGINT PRIMARY KEY CHECK(id=1), tenant TEXT NOT NULL, version BIGINT NOT NULL)").execute(&mut *tx).await?;
        let created = sqlx::query("INSERT INTO journal_meta VALUES(1,$1,1) ON CONFLICT DO NOTHING")
            .bind(&config.tenant_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
        let (tenant, version): (String, i64) =
            sqlx::query_as("SELECT tenant,version FROM journal_meta WHERE id=1")
                .fetch_one(&mut *tx)
                .await?;
        ensure!(
            tenant == config.tenant_id,
            "Journal belongs to a different tenant"
        );
        ensure!(
            version == 1,
            "Unsupported PostgreSQL journal schema version"
        );
        if !created {
            self.upgrade_tenant_columns(&mut tx, config, Some(schema))
                .await?;
            Self::check_capacity(&mut tx, config).await?;
            tx.commit().await?;
            return Ok(());
        }
        sqlx::query("CREATE TABLE IF NOT EXISTS events (seq BIGSERIAL PRIMARY KEY, kind TEXT NOT NULL, digest TEXT NOT NULL UNIQUE, body TEXT NOT NULL, input TEXT, bytes BIGINT NOT NULL, delivered BIGINT NOT NULL DEFAULT 0, lease TEXT, retry_at BIGINT NOT NULL DEFAULT 0, attempts BIGINT NOT NULL DEFAULT 0, response TEXT, http_status BIGINT)").execute(&mut *tx).await?;
        sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS journal_decision_identity ON events((body::jsonb->>'tenant_id'),(body::jsonb->>'decision_id')) WHERE kind='decision-record'").execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS journal_pending_decisions ON events(seq) WHERE kind='decision-record' AND delivered=0").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_usage (id BIGINT PRIMARY KEY CHECK(id=1), record_count BIGINT NOT NULL, total_bytes BIGINT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("INSERT INTO journal_usage VALUES(1,0,0) ON CONFLICT DO NOTHING")
            .execute(&mut *tx)
            .await?;
        // Counters stay correct for explicit retention/deletion as well as inserts.
        sqlx::query(&format!(r#"CREATE OR REPLACE FUNCTION "{schema}".maintain_journal_usage() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                IF TG_OP = 'INSERT' THEN
                    UPDATE "{schema}".journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1;
                ELSIF TG_OP = 'DELETE' THEN
                    UPDATE "{schema}".journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1;
                ELSE
                    UPDATE "{schema}".journal_usage SET total_bytes=total_bytes+NEW.bytes-OLD.bytes WHERE id=1;
                END IF;
                RETURN NULL;
            END $$"#)).execute(&mut *tx).await?;
        // Idempotent trigger installation in the initialization transaction.
        sqlx::query("DROP TRIGGER IF EXISTS journal_usage_change ON events")
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!("CREATE TRIGGER journal_usage_change AFTER INSERT OR DELETE OR UPDATE OF bytes ON events FOR EACH ROW EXECUTE FUNCTION \"{schema}\".maintain_journal_usage()")).execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS request_keys (key TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, owner TEXT NOT NULL, expires BIGINT NOT NULL, decision_id TEXT)").execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS pending_request_expiry ON request_keys(expires) WHERE decision_id IS NULL").execute(&mut *tx).await?;
        self.upgrade_tenant_columns(&mut tx, config, Some(schema))
            .await?;
        Self::check_capacity(&mut tx, config).await?;
        tx.commit().await?;
        Ok(())
    }

    // Operator-only schema migration. It runs transactionally once; online
    // requests never scan historical records or construct arbitrary SQL.
    async fn upgrade_tenant_columns(
        &self,
        tx: &mut Transaction<'_, Any>,
        config: &JournalConfig,
        schema: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_access_schema (id BIGINT PRIMARY KEY CHECK(id=1), version BIGINT NOT NULL)").execute(&mut **tx).await?;
        let version: Option<i64> =
            sqlx::query_scalar("SELECT version FROM journal_access_schema WHERE id=1")
                .fetch_optional(&mut **tx)
                .await?;
        if let Some(version) = version {
            ensure!(version == 1, "Unsupported tenant access schema");
            return Ok(());
        }
        let invalid = if self.postgres {
            "SELECT COUNT(*) FROM events WHERE kind='decision-record' AND ((body::jsonb->>'tenant_id') IS NULL OR (body::jsonb->>'tenant_id')<>$1)"
        } else {
            "SELECT COUNT(*) FROM events WHERE kind='decision-record' AND (json_extract(body,'$.tenant_id') IS NULL OR json_extract(body,'$.tenant_id')<>$1)"
        };
        let invalid: i64 = sqlx::query_scalar(invalid)
            .bind(&self.tenant)
            .fetch_one(&mut **tx)
            .await?;
        ensure!(
            invalid == 0,
            "Cannot migrate records belonging to another tenant"
        );
        // Backfill from the verified storage owner, never the current request.
        for table in ["events", "request_keys", "journal_usage"] {
            sqlx::query(&format!(
                "ALTER TABLE {table} ADD COLUMN tenant_id TEXT NOT NULL DEFAULT ''"
            ))
            .execute(&mut **tx)
            .await?;
            sqlx::query(&format!(
                "UPDATE {table} SET tenant_id=$1 WHERE tenant_id=''"
            ))
            .bind(&config.tenant_id)
            .execute(&mut **tx)
            .await?;
            if self.postgres {
                sqlx::query(&format!(
                    "ALTER TABLE {table} ALTER COLUMN tenant_id DROP DEFAULT"
                ))
                .execute(&mut **tx)
                .await?;
                sqlx::query(&format!("ALTER TABLE {table} ADD CONSTRAINT {table}_tenant_required CHECK (tenant_id<>'')")).execute(&mut **tx).await?;
            } else {
                for action in ["INSERT", "UPDATE"] {
                    sqlx::query(&format!("CREATE TRIGGER {table}_tenant_required_{action} BEFORE {action} ON {table} WHEN NEW.tenant_id='' BEGIN SELECT RAISE(ABORT,'tenant_id required'); END")).execute(&mut **tx).await?;
                }
            }
        }
        if self.postgres {
            sqlx::query("ALTER TABLE events ADD CONSTRAINT event_tenant_matches CHECK (kind<>'decision-record' OR (body::jsonb->>'tenant_id') IS NOT DISTINCT FROM tenant_id)").execute(&mut **tx).await?;
        } else {
            for action in ["INSERT", "UPDATE"] {
                sqlx::query(&format!("CREATE TRIGGER event_tenant_matches_{action} BEFORE {action} ON events WHEN NEW.kind='decision-record' AND json_extract(NEW.body,'$.tenant_id') IS NOT NEW.tenant_id BEGIN SELECT RAISE(ABORT,'event tenant mismatch'); END")).execute(&mut **tx).await?;
            }
        }
        sqlx::query("CREATE INDEX journal_tenant_pending ON events(tenant_id,seq) WHERE kind='decision-record' AND delivered=0").execute(&mut **tx).await?;
        sqlx::query("CREATE INDEX journal_tenant_requests ON request_keys(tenant_id,key)")
            .execute(&mut **tx)
            .await?;
        if let Some(schema) = schema {
            sqlx::query(&format!(r#"CREATE OR REPLACE FUNCTION "{schema}".maintain_journal_usage() RETURNS trigger LANGUAGE plpgsql AS $$
                BEGIN
                    IF TG_OP = 'INSERT' THEN
                        UPDATE "{schema}".journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1 AND tenant_id=NEW.tenant_id;
                    ELSIF TG_OP = 'DELETE' THEN
                        UPDATE "{schema}".journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1 AND tenant_id=OLD.tenant_id;
                    ELSE
                        UPDATE "{schema}".journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1 AND tenant_id=OLD.tenant_id;
                        UPDATE "{schema}".journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1 AND tenant_id=NEW.tenant_id;
                    END IF;
                    RETURN NULL;
                END $$"#)).execute(&mut **tx).await?;
            sqlx::query("DROP TRIGGER journal_usage_change ON events")
                .execute(&mut **tx)
                .await?;
            sqlx::query(&format!("CREATE TRIGGER journal_usage_change AFTER INSERT OR DELETE OR UPDATE OF bytes,tenant_id ON events FOR EACH ROW EXECUTE FUNCTION \"{schema}\".maintain_journal_usage()")).execute(&mut **tx).await?;
        } else {
            for name in ["insert", "delete", "update"] {
                sqlx::query(&format!("DROP TRIGGER journal_usage_{name}"))
                    .execute(&mut **tx)
                    .await?;
            }
            sqlx::query("CREATE TRIGGER journal_usage_insert AFTER INSERT ON events BEGIN UPDATE journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1 AND tenant_id=NEW.tenant_id; END").execute(&mut **tx).await?;
            sqlx::query("CREATE TRIGGER journal_usage_delete AFTER DELETE ON events BEGIN UPDATE journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1 AND tenant_id=OLD.tenant_id; END").execute(&mut **tx).await?;
            sqlx::query("CREATE TRIGGER journal_usage_update AFTER UPDATE OF bytes,tenant_id ON events BEGIN UPDATE journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1 AND tenant_id=OLD.tenant_id; UPDATE journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1 AND tenant_id=NEW.tenant_id; END").execute(&mut **tx).await?;
        }
        sqlx::query("INSERT INTO journal_access_schema VALUES(1,1)")
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn check_capacity(
        tx: &mut Transaction<'_, Any>,
        config: &JournalConfig,
    ) -> anyhow::Result<()> {
        let (records, bytes): (i64, i64) =
            sqlx::query_as("SELECT record_count,total_bytes FROM journal_usage WHERE id=1")
                .fetch_one(&mut **tx)
                .await?;
        ensure!(
            records <= i64::from(config.max_records) && bytes <= config.max_bytes as i64,
            "Journal capacity exceeded"
        );
        Ok(())
    }
    // A transactional, one-time upgrade of the existing events table. Historical
    // feedback stays untouched for external migration; it is never replayed.
    async fn initialize_sqlite(&self, config: &JournalConfig) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_meta (id INTEGER PRIMARY KEY CHECK(id=1), tenant TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("INSERT OR IGNORE INTO journal_meta VALUES(1,$1)")
            .bind(&config.tenant_id)
            .execute(&mut *tx)
            .await?;
        let tenant: String = sqlx::query_scalar("SELECT tenant FROM journal_meta WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
        ensure!(
            tenant == config.tenant_id,
            "Journal belongs to a different tenant"
        );
        sqlx::query("CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, digest TEXT NOT NULL UNIQUE, body TEXT NOT NULL, input TEXT, bytes INTEGER NOT NULL, delivered INTEGER NOT NULL DEFAULT 0, lease TEXT, retry_at INTEGER NOT NULL DEFAULT 0, attempts INTEGER NOT NULL DEFAULT 0)").execute(&mut *tx).await?;

        sqlx::query("CREATE TABLE IF NOT EXISTS journal_usage (id INTEGER PRIMARY KEY CHECK(id=1), record_count INTEGER NOT NULL, total_bytes INTEGER NOT NULL)")
            .execute(&mut *tx).await?;
        let initialized: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM journal_usage WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
        if initialized == 0 {
            let rows = sqlx::query(
                "SELECT body,digest FROM events WHERE kind='decision-record' ORDER BY seq",
            )
            .fetch_all(&mut *tx)
            .await?;
            for row in rows {
                let value: Value = serde_json::from_str(row.get("body"))?;
                anyhow::ensure!(value["tenant_id"] == config.tenant_id, "Tenant mismatch");
                let record = contract("decision-record", &value)?;
                validate_decision_record(&record)?;
                anyhow::ensure!(
                    record.sha256() == row.get::<String, _>("digest"),
                    "Journal content fingerprint mismatch"
                );
            }
            sqlx::query("CREATE UNIQUE INDEX journal_decision_identity ON events(json_extract(body,'$.tenant_id'), json_extract(body,'$.decision_id')) WHERE kind='decision-record'")
                .execute(&mut *tx).await?;
            sqlx::query("CREATE INDEX journal_pending_decisions ON events(seq) WHERE kind='decision-record' AND delivered=0")
                .execute(&mut *tx).await?;
            sqlx::query(
                "INSERT INTO journal_usage SELECT 1,COUNT(*),COALESCE(SUM(bytes),0) FROM events",
            )
            .execute(&mut *tx)
            .await?;
            // Triggers keep counters correct across connections, restarts and
            // future archive/delete operations, in the same transaction as rows.
            sqlx::query("CREATE TRIGGER journal_usage_insert AFTER INSERT ON events BEGIN UPDATE journal_usage SET record_count=record_count+1,total_bytes=total_bytes+NEW.bytes WHERE id=1; END")
                .execute(&mut *tx).await?;
            sqlx::query("CREATE TRIGGER journal_usage_delete AFTER DELETE ON events BEGIN UPDATE journal_usage SET record_count=record_count-1,total_bytes=total_bytes-OLD.bytes WHERE id=1; END")
                .execute(&mut *tx).await?;
            sqlx::query("CREATE TRIGGER journal_usage_update AFTER UPDATE OF bytes ON events BEGIN UPDATE journal_usage SET total_bytes=total_bytes+NEW.bytes-OLD.bytes WHERE id=1; END")
                .execute(&mut *tx).await?;
        }
        let usage = sqlx::query("SELECT record_count,total_bytes FROM journal_usage WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
        anyhow::ensure!(
            usage.get::<i64, _>("record_count") <= i64::from(config.max_records)
                && usage.get::<i64, _>("total_bytes") <= config.max_bytes as i64,
            "Journal capacity exceeded"
        );
        // Incremental upgrade; no scan or replay of historical decisions.
        let columns = sqlx::query("PRAGMA table_info(events)")
            .fetch_all(&mut *tx)
            .await?;
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "response")
        {
            sqlx::query("ALTER TABLE events ADD COLUMN response TEXT")
                .execute(&mut *tx)
                .await?;
            sqlx::query("ALTER TABLE events ADD COLUMN http_status INTEGER")
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("CREATE TABLE IF NOT EXISTS request_keys (key TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, owner TEXT NOT NULL, expires INTEGER NOT NULL, decision_id TEXT)").execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS pending_request_expiry ON request_keys(expires) WHERE decision_id IS NULL").execute(&mut *tx).await?;
        self.upgrade_tenant_columns(&mut tx, config, None).await?;
        tx.commit().await?;
        Ok(())
    }
}
