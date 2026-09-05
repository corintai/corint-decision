//! Bounded durable event journal and leased, at-least-once outbox.
//! This stores evidence, never policy source or the active policy selection.
use corint_decision_compiler::core::CoreSource;
use corint_decision_toolchain::phase0::{Contract, FeedbackLedger, Ingest};
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
    pub tenant_id: String,
    pub max_records: u32,
    pub max_bytes: u64,
    /// Separate feedback/outbox role, loaded only from operator environment.
    pub consumer_token_env: String,
}
#[derive(Clone)]
pub struct Journal {
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
fn apply(ledger: &mut FeedbackLedger, kind: &str, value: &Value) -> anyhow::Result<Ingest> {
    let c = contract(kind, value)?;
    Ok(match kind {
        "decision-record" => ledger.record_decision(c)?,
        "outcome-event" => ledger.ingest_outcome(c)?,
        "action-receipt" => ledger.ingest_receipt(c)?,
        _ => anyhow::bail!("Unsupported event kind"),
    })
}
impl Journal {
    pub async fn open(root: &Path, config: &JournalConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=100_000).contains(&config.max_records)
                && (1024..=1024 * 1024 * 1024).contains(&config.max_bytes),
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
        // Serialize validation and append in a transaction; a second process must
        // take the same write lock before reconstructing correlation history.
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
            pool,
            tenant_id: tenant,
            max_records: config.max_records,
            max_bytes: config.max_bytes,
            admission: std::sync::Arc::new(tokio::sync::Semaphore::new(64)),
        };
        // Refuse corrupt or semantically invalid persisted histories on restart.
        let mut tx = journal.pool.begin().await?;
        let rows = sqlx::query("SELECT kind,body,digest,bytes FROM events ORDER BY seq LIMIT ?")
            .bind(i64::from(config.max_records) + 1)
            .fetch_all(&mut *tx)
            .await?;
        anyhow::ensure!(
            rows.len() <= config.max_records as usize,
            "Journal capacity exceeded"
        );
        let mut ledger = FeedbackLedger::default();
        let mut bytes = 0u64;
        for row in rows {
            let value: Value = serde_json::from_str(row.get("body"))?;
            anyhow::ensure!(
                contract(row.get("kind"), &value)?.sha256() == row.get::<String, _>("digest"),
                "Journal content fingerprint mismatch"
            );
            bytes += row.get::<i64, _>("bytes") as u64;
            apply(&mut ledger, row.get("kind"), &value)?;
        }
        anyhow::ensure!(bytes <= journal.max_bytes, "Journal byte capacity exceeded");
        tx.commit().await?;
        Ok(journal)
    }
    /// Validation, history update, input evidence and outbox insertion are atomic.
    /// Full or unavailable storage fails the request; no successful audit is lost.
    pub async fn append(
        &self,
        kind: &str,
        value: &Value,
        input: Option<&Value>,
    ) -> anyhow::Result<Ingest> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        anyhow::ensure!(value["tenant_id"] == self.tenant_id, "Tenant mismatch");
        let body = value.to_string();
        let input = input.map(Value::to_string);
        let bytes = body.len() + input.as_ref().map_or(0, String::len);
        anyhow::ensure!(bytes <= 8 * 1024 * 1024, "Event too large");
        let candidate = contract(kind, value)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query("SELECT kind,body,digest,bytes FROM events ORDER BY seq LIMIT ?")
            .bind(i64::from(self.max_records) + 1)
            .fetch_all(&mut *tx)
            .await?;
        let count = rows.len();
        anyhow::ensure!(
            count <= self.max_records as usize,
            "Journal capacity exceeded"
        );
        let mut total = 0u64;
        let mut ledger = FeedbackLedger::default();
        for row in rows {
            total += row.get::<i64, _>("bytes") as u64;
            let value: Value = serde_json::from_str(row.get("body"))?;
            anyhow::ensure!(
                contract(row.get("kind"), &value)?.sha256() == row.get::<String, _>("digest"),
                "Journal content fingerprint mismatch"
            );
            apply(&mut ledger, row.get("kind"), &value)?;
        }
        let result = apply(&mut ledger, kind, value)?;
        if result == Ingest::Duplicate {
            tx.commit().await?;
            return Ok(result);
        }
        anyhow::ensure!(
            count < self.max_records as usize && total + bytes as u64 <= self.max_bytes,
            "Journal capacity exhausted"
        );
        sqlx::query("INSERT INTO events(kind,digest,body,input,bytes) VALUES(?,?,?,?,?)")
            .bind(kind)
            .bind(candidate.sha256())
            .bind(body)
            .bind(input)
            .bind(bytes as i64)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    /// A crashed or disconnected consumer leaves a lease that becomes eligible
    /// again. Stable digest is the consumer's idempotency key across retries.
    pub async fn claim(&self, now_ms: i64) -> anyhow::Result<Value> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query("SELECT seq,digest,body,attempts FROM (SELECT seq,digest,body,attempts,SUM(bytes) OVER (ORDER BY seq) AS batch_bytes FROM events WHERE delivered=0 AND retry_at<=?) WHERE batch_bytes<=8388608 ORDER BY seq LIMIT 100").bind(now_ms).fetch_all(&mut *tx).await?;
        let lease = uuid::Uuid::new_v4().to_string();
        let mut events = Vec::new();
        for row in rows {
            let attempts: i64 = row.get("attempts");
            // 1, 2, 4 ... 60 minutes. Retries remain eligible after restarts.
            let delay = 60_000i64 * (1i64 << attempts.min(6)).min(60);
            sqlx::query("UPDATE events SET lease=?,retry_at=?,attempts=attempts+1 WHERE seq=?")
                .bind(&lease)
                .bind(now_ms + delay)
                .bind(row.get::<i64, _>("seq"))
                .execute(&mut *tx)
                .await?;
            events.push(json!({"idempotency_key":row.get::<String,_>("digest"),"attempt":attempts+1,"lease_expires_at_ms":now_ms+delay,"event":serde_json::from_str::<Value>(row.get("body"))?}));
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
    pub async fn outcome_as_of(
        &self,
        decision: &str,
        label: &str,
        time: u64,
    ) -> anyhow::Result<Option<Value>> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        let rows = sqlx::query("SELECT kind,body,digest,bytes FROM events ORDER BY seq LIMIT ?")
            .bind(i64::from(self.max_records) + 1)
            .fetch_all(&self.pool)
            .await?;
        let mut ledger = FeedbackLedger::default();
        for row in rows {
            apply(
                &mut ledger,
                row.get("kind"),
                &serde_json::from_str::<Value>(row.get("body"))?,
            )?;
        }
        Ok(ledger
            .outcome_as_of(&self.tenant_id, decision, label, time)
            .map(|c| c.value().clone()))
    }
}
