//! Durable online decision records with an optional, at-least-once export outbox.
//! This stores evidence, never policy source or the active policy selection.
use corint_decision_compiler::core::CoreSource;
use corint_decision_engine::background::{BackgroundWrites, PersistenceStatus};
use corint_decision_toolchain::phase0::{validate_decision_record, Contract, Ingest};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Row, SqlitePool,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalConfig {
    pub path: PathBuf,
    /// Explicit opt-in to volatile queuing; cannot provide request idempotency.
    #[serde(default)]
    pub best_effort: bool,
    /// Explicitly allow the export role to receive private replay inputs/results.
    #[serde(default)]
    pub export_replay: bool,
    pub tenant_id: String,
    pub max_records: u32,
    pub max_bytes: u64,
    /// Optional decision-export role. Empty disables outbox HTTP routes.
    #[serde(default)]
    pub consumer_token_env: String,
}
#[derive(Clone)]
pub struct Journal {
    pub best_effort: bool,
    export_replay: bool,
    background: BackgroundWrites,
    pool: SqlitePool,
    pub tenant_id: String,
    max_records: u32,
    max_bytes: u64,
    admission: std::sync::Arc<tokio::sync::Semaphore>,
}
pub fn contract(kind: &str, value: &Value) -> anyhow::Result<Contract> {
    Ok(Contract::load(
        kind,
        &CoreSource {
            path: "event".into(),
            yaml: value.to_string(),
        },
    )?)
}
impl Journal {
    pub async fn open(root: &Path, config: &JournalConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            config.max_records > 0 && (1024..=i64::MAX as u64).contains(&config.max_bytes),
            "Invalid journal capacity"
        );
        anyhow::ensure!(
            !config.tenant_id.is_empty()
                && config.tenant_id.len() <= 128
                && config
                    .tenant_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
            "Invalid journal tenant"
        );
        let path = root.join(&config.path);
        anyhow::ensure!(path != Path::new(":memory:"), "Journal must be persistent");
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            anyhow::ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "Journal must be a regular file"
            );
        }
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(std::time::Duration::from_secs(5));
        // Serialize local inserts and capacity accounting. Separate processes use
        // the same database write lock, without reconstructing feedback history.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_meta (id INTEGER PRIMARY KEY CHECK(id=1), tenant TEXT NOT NULL)").execute(&pool).await?;
        sqlx::query("INSERT OR IGNORE INTO journal_meta VALUES(1,?)")
            .bind(&config.tenant_id)
            .execute(&pool)
            .await?;
        let tenant: String = sqlx::query_scalar("SELECT tenant FROM journal_meta WHERE id=1")
            .fetch_one(&pool)
            .await?;
        anyhow::ensure!(
            tenant == config.tenant_id,
            "Journal belongs to a different tenant"
        );
        sqlx::query("CREATE TABLE IF NOT EXISTS events (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, digest TEXT NOT NULL UNIQUE, body TEXT NOT NULL, input TEXT, bytes INTEGER NOT NULL, delivered INTEGER NOT NULL DEFAULT 0, lease TEXT, retry_at INTEGER NOT NULL DEFAULT 0, attempts INTEGER NOT NULL DEFAULT 0)").execute(&pool).await?;
        let journal = Self {
            best_effort: config.best_effort,
            export_replay: config.export_replay,
            background: BackgroundWrites::default(),
            pool,
            tenant_id: tenant,
            max_records: config.max_records,
            max_bytes: config.max_bytes,
            admission: std::sync::Arc::new(tokio::sync::Semaphore::new(64)),
        };
        journal.initialize_storage().await?;
        Ok(journal)
    }

    // A transactional, one-time upgrade of the existing events table. Historical
    // feedback stays untouched for external migration; it is never replayed.
    async fn initialize_storage(&self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS journal_usage (id INTEGER PRIMARY KEY CHECK(id=1), record_count INTEGER NOT NULL, total_bytes INTEGER NOT NULL)")
            .execute(&mut *tx).await?;
        let initialized: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM journal_usage WHERE id=1)")
                .fetch_one(&mut *tx)
                .await?;
        if !initialized {
            let rows = sqlx::query(
                "SELECT body,digest FROM events WHERE kind='decision-record' ORDER BY seq",
            )
            .fetch_all(&mut *tx)
            .await?;
            for row in rows {
                let value: Value = serde_json::from_str(row.get("body"))?;
                anyhow::ensure!(value["tenant_id"] == self.tenant_id, "Tenant mismatch");
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
            usage.get::<i64, _>("record_count") <= i64::from(self.max_records)
                && usage.get::<i64, _>("total_bytes") <= self.max_bytes as i64,
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
        tx.commit().await?;
        Ok(())
    }

    /// Queue one frozen decision and input without waiting for SQLite I/O.
    pub fn enqueue(&self, value: Value, input: Value) -> anyhow::Result<()> {
        anyhow::ensure!(value["tenant_id"] == self.tenant_id, "Tenant mismatch");
        let record = contract("decision-record", &value)?;
        validate_decision_record(&record)?;
        let bytes = value.to_string().len() + input.to_string().len();
        anyhow::ensure!(bytes <= 8 * 1024 * 1024, "Event too large");
        let id = value["decision_id"]
            .as_str()
            .expect("validated decision ID")
            .to_owned();
        let journal = self.clone();
        self.background
            .submit(id, bytes, move || {
                let journal = journal.clone();
                let value = value.clone();
                let input = input.clone();
                async move {
                    journal
                        .append("decision-record", &value, Some(&input))
                        .await
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            })
            .map_err(anyhow::Error::msg)
    }
    pub fn persistence_status(&self) -> PersistenceStatus {
        self.background.status()
    }

    pub async fn status(&self) -> anyhow::Result<Value> {
        if self.best_effort {
            let mut status = serde_json::to_value(self.background.status())?;
            status["mode"] = "best_effort".into();
            return Ok(status);
        }
        let row: (i64,i64,i64) = sqlx::query_as("SELECT record_count,total_bytes,(SELECT COUNT(*) FROM request_keys WHERE decision_id IS NULL AND expires>?) FROM journal_usage WHERE id=1")
            .bind(chrono::Utc::now().timestamp_millis()).fetch_one(&self.pool).await?;
        Ok(
            json!({"mode":"reliable","accepting":row.0+row.2 < i64::from(self.max_records) && row.1 < self.max_bytes as i64 && row.2 < 64,
            "stored_records":row.0,"stored_bytes":row.1,"inflight_requests":row.2,"max_records":self.max_records,"max_bytes":self.max_bytes}),
        )
    }

    /// Validate only this decision, then atomically save its evidence and export
    /// state. Same ID/content is idempotent; conflicting content never overwrites.
    /// Feedback ingestion and cross-event state belong to the external Agent.
    pub async fn append(
        &self,
        kind: &str,
        value: &Value,
        input: Option<&Value>,
    ) -> anyhow::Result<Ingest> {
        self.append_response(kind, value, input, None).await
    }

    pub async fn append_response(
        &self,
        kind: &str,
        value: &Value,
        input: Option<&Value>,
        response: Option<(&RequestReservation, u16, &Value)>,
    ) -> anyhow::Result<Ingest> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        anyhow::ensure!(
            kind == "decision-record",
            "Journal accepts only decision records; feedback belongs to the external Agent"
        );
        anyhow::ensure!(value["tenant_id"] == self.tenant_id, "Tenant mismatch");
        let body = value.to_string();
        let input = input.map(Value::to_string);
        let response_body = response.as_ref().map(|(_, _, body)| body.to_string());
        let bytes = body.len()
            + input.as_ref().map_or(0, String::len)
            + response_body.as_ref().map_or(0, |b| b.len() + 1024);
        anyhow::ensure!(bytes <= 8 * 1024 * 1024, "Event too large");
        let candidate = contract(kind, value)?;
        validate_decision_record(&candidate)?;
        let digest = candidate.sha256();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some((reservation, _, _)) = response {
            let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM request_keys WHERE key=? AND owner=? AND decision_id IS NULL)")
                .bind(&reservation.key).bind(&reservation.owner).fetch_one(&mut *tx).await?;
            anyhow::ensure!(owned, "Request reservation was superseded");
        }
        let existing = sqlx::query("SELECT digest,input FROM events WHERE kind='decision-record' AND json_extract(body,'$.tenant_id')=? AND json_extract(body,'$.decision_id')=?")
            .bind(&self.tenant_id)
            .bind(value["decision_id"].as_str().expect("validated decision ID"))
            .fetch_optional(&mut *tx).await?;
        if let Some(existing) = existing {
            anyhow::ensure!(
                existing.get::<String, _>("digest") == digest,
                "Decision ID reused with different content"
            );
            if let Some(input) = &input {
                anyhow::ensure!(
                    existing.get::<Option<String>, _>("input").as_ref() == Some(input),
                    "Decision ID reused with different input evidence"
                );
            }
            tx.commit().await?;
            return Ok(Ingest::Duplicate);
        }
        let usage = sqlx::query("SELECT record_count,total_bytes FROM journal_usage WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
        let total = usage.get::<i64, _>("total_bytes");
        anyhow::ensure!(
            usage.get::<i64, _>("record_count") < i64::from(self.max_records)
                && total >= 0
                && bytes as u64 <= self.max_bytes
                && total as u64 <= self.max_bytes - bytes as u64,
            "Journal capacity exhausted"
        );
        sqlx::query("INSERT INTO events(kind,digest,body,input,bytes) VALUES(?,?,?,?,?)")
            .bind(kind)
            .bind(digest)
            .bind(body)
            .bind(input)
            .bind(bytes as i64)
            .execute(&mut *tx)
            .await?;
        if let Some((reservation, status, _)) = response {
            sqlx::query("UPDATE events SET response=?,http_status=? WHERE digest=?")
                .bind(response_body)
                .bind(i64::from(status))
                .bind(candidate.sha256())
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE request_keys SET decision_id=?,expires=0 WHERE key=? AND owner=?")
                .bind(value["decision_id"].as_str())
                .bind(&reservation.key)
                .bind(&reservation.owner)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Ingest::Inserted)
    }
    /// Reserve a bounded execution slot in the same local durability domain as
    /// the result. Expiry fences abandoned owners; only the current owner can commit.
    pub async fn begin_request(
        &self,
        key: Option<&str>,
        fingerprint: &str,
        now: i64,
    ) -> anyhow::Result<RequestStart> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        anyhow::ensure!(
            key.is_none_or(|k| !k.is_empty() && k.len() <= 128) && fingerprint.len() <= 128,
            "Invalid request identity"
        );
        anyhow::ensure!(
            !self.best_effort,
            "Idempotency requires reliable persistence"
        );
        let key = key
            .map(|k| format!("client:{k}"))
            .unwrap_or_else(|| format!("auto:{}", uuid::Uuid::new_v4()));
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(row) = sqlx::query(
            "SELECT fingerprint,owner,decision_id,expires FROM request_keys WHERE key=?",
        )
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?
        {
            anyhow::ensure!(
                row.get::<String, _>("fingerprint") == fingerprint,
                "E_IDEMPOTENCY_CONFLICT"
            );
            if let Some(id) = row.get::<Option<String>, _>("decision_id") {
                let result: (i64, String) = sqlx::query_as("SELECT http_status,response FROM events WHERE kind='decision-record' AND json_extract(body,'$.tenant_id')=? AND json_extract(body,'$.decision_id')=?")
                    .bind(&self.tenant_id).bind(id).fetch_one(&mut *tx).await?;
                tx.commit().await?;
                return Ok(RequestStart::Replay(
                    result.0 as u16,
                    serde_json::from_str(&result.1)?,
                ));
            }
            anyhow::ensure!(row.get::<i64, _>("expires") <= now, "E_REQUEST_IN_PROGRESS");
        }
        sqlx::query("DELETE FROM request_keys WHERE decision_id IS NULL AND expires<=?")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        let pending: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM request_keys WHERE decision_id IS NULL")
                .fetch_one(&mut *tx)
                .await?;
        let usage: (i64, i64) =
            sqlx::query_as("SELECT record_count,total_bytes FROM journal_usage WHERE id=1")
                .fetch_one(&mut *tx)
                .await?;
        anyhow::ensure!(
            pending < 64
                && usage.0 + pending < i64::from(self.max_records)
                && usage.1 < self.max_bytes as i64,
            "E_JOURNAL_CAPACITY"
        );
        let owner = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO request_keys(key,fingerprint,owner,expires) VALUES(?,?,?,?)")
            .bind(&key)
            .bind(fingerprint)
            .bind(&owner)
            .bind(now + 120_000)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(RequestStart::Reserved(RequestReservation { key, owner }))
    }
    pub async fn abandon(&self, reservation: &RequestReservation) {
        let _ =
            sqlx::query("DELETE FROM request_keys WHERE key=? AND owner=? AND decision_id IS NULL")
                .bind(&reservation.key)
                .bind(&reservation.owner)
                .execute(&self.pool)
                .await;
    }

    /// A crashed or disconnected consumer leaves a lease that becomes eligible
    /// again. Stable digest is the consumer's idempotency key across retries.
    pub async fn claim(&self, now_ms: i64) -> anyhow::Result<Value> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query("SELECT seq,digest,body,input,response,bytes,attempts FROM events WHERE kind='decision-record' AND delivered=0 AND retry_at<=? ORDER BY seq LIMIT 100")
            .bind(now_ms).fetch_all(&mut *tx).await?;
        let lease = uuid::Uuid::new_v4().to_string();
        let mut events = Vec::new();
        let mut batch_bytes = 0i64;
        for row in rows {
            batch_bytes += row.get::<i64, _>("bytes");
            if batch_bytes > 8 * 1024 * 1024 {
                break;
            }
            let attempts: i64 = row.get("attempts");
            // 1, 2, 4 ... 60 minutes. Retries remain eligible after restarts.
            let delay = 60_000i64 * (1i64 << attempts.min(6)).min(60);
            sqlx::query("UPDATE events SET lease=?,retry_at=?,attempts=attempts+1 WHERE seq=?")
                .bind(&lease)
                .bind(now_ms + delay)
                .bind(row.get::<i64, _>("seq"))
                .execute(&mut *tx)
                .await?;
            let mut item = json!({"idempotency_key":row.get::<String,_>("digest"),"attempt":attempts+1,"lease_expires_at_ms":now_ms+delay,"event":serde_json::from_str::<Value>(row.get("body"))?});
            if self.export_replay {
                item["input_evidence"] = row
                    .get::<Option<String>, _>("input")
                    .map(|v| serde_json::from_str::<Value>(&v))
                    .transpose()?
                    .unwrap_or(Value::Null);
                item["response"] = row
                    .get::<Option<String>, _>("response")
                    .map(|v| serde_json::from_str::<Value>(&v))
                    .transpose()?
                    .unwrap_or(Value::Null);
            }
            events.push(item);
        }
        tx.commit().await?;
        Ok(json!({"lease":lease,"events":events}))
    }
    pub async fn acknowledge(
        &self,
        lease: &str,
        digests: &[String],
        now_ms: i64,
    ) -> anyhow::Result<()> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        anyhow::ensure!(
            !digests.is_empty() && digests.len() <= 100,
            "Invalid acknowledgement size"
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for digest in digests {
            let n = sqlx::query(
                "UPDATE events SET delivered=1 WHERE digest=? AND lease=? AND retry_at>? ",
            )
            .bind(digest)
            .bind(lease)
            .bind(now_ms)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            anyhow::ensure!(n == 1, "Unknown, expired or replaced lease");
        }
        tx.commit().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct RequestReservation {
    key: String,
    owner: String,
}
#[derive(Debug)]
pub enum RequestStart {
    Reserved(RequestReservation),
    Replay(u16, Value),
}
