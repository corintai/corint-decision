//! Durable online decision records with an optional, at-least-once export outbox.
//! This stores evidence, never policy source or the active policy selection.
use corint_decision_compiler::core::CoreSource;
use corint_decision_engine::background::{BackgroundWrites, PersistenceStatus};
use corint_decision_toolchain::phase0::{validate_decision_record, Contract, Ingest};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
mod storage;
use std::path::{Path, PathBuf};
use storage::Storage;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalConfig {
    /// Legacy/default SQLite file path; forbidden for PostgreSQL.
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub backend: JournalBackend,
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
/// Backend selection is independent of policy repositories and feature sources.
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JournalBackend {
    Sqlite {},
    #[serde(alias = "postgresql")]
    Postgres {
        url_env: String,
        schema: String,
    },
}
impl Default for JournalBackend {
    fn default() -> Self {
        Self::Sqlite {}
    }
}
#[derive(Clone)]
pub struct Journal {
    pub best_effort: bool,
    export_replay: bool,
    background: BackgroundWrites,
    storage: Storage,
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
        let storage = Storage::open(root, config).await?;
        Ok(Self {
            best_effort: config.best_effort,
            export_replay: config.export_replay,
            background: BackgroundWrites::default(),
            storage,
            tenant_id: config.tenant_id.clone(),
            max_records: config.max_records,
            max_bytes: config.max_bytes,
            admission: std::sync::Arc::new(tokio::sync::Semaphore::new(64)),
        })
    }

    /// Queue one frozen decision and input without waiting for database I/O.
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
            status["backend"] = self.storage.name().into();
            return Ok(status);
        }
        let row: (i64,i64,i64) = sqlx::query_as("SELECT record_count,total_bytes,(SELECT COUNT(*) FROM request_keys WHERE decision_id IS NULL AND expires>$1) FROM journal_usage WHERE id=1")
            .bind(chrono::Utc::now().timestamp_millis()).fetch_one(&self.storage.pool).await?;
        Ok(
            json!({"mode":"reliable","backend":self.storage.name(),"accepting":row.0+row.2 < i64::from(self.max_records) && row.1 < self.max_bytes as i64 && row.2 < 64,
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
        let mut tx = self.storage.begin().await?;
        if let Some((reservation, _, _)) = response {
            let owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_keys WHERE key=$1 AND owner=$2 AND decision_id IS NULL")
                .bind(&reservation.key).bind(&reservation.owner).fetch_one(&mut *tx).await?;
            anyhow::ensure!(owned == 1, "Request reservation was superseded");
        }
        let existing = sqlx::query(self.storage.decision_lookup())
            .bind(&self.tenant_id)
            .bind(
                value["decision_id"]
                    .as_str()
                    .expect("validated decision ID"),
            )
            .fetch_optional(&mut *tx)
            .await?;
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
        sqlx::query("INSERT INTO events(kind,digest,body,input,bytes) VALUES($1,$2,$3,$4,$5)")
            .bind(kind)
            .bind(digest)
            .bind(body)
            .bind(input)
            .bind(bytes as i64)
            .execute(&mut *tx)
            .await?;
        if let Some((reservation, status, _)) = response {
            sqlx::query("UPDATE events SET response=$1,http_status=$2 WHERE digest=$3")
                .bind(response_body)
                .bind(i64::from(status))
                .bind(candidate.sha256())
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE request_keys SET decision_id=$1,expires=0 WHERE key=$2 AND owner=$3",
            )
            .bind(value["decision_id"].as_str())
            .bind(&reservation.key)
            .bind(&reservation.owner)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(Ingest::Inserted)
    }
    /// Reserve a bounded execution slot in the same durability domain as
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
        let mut tx = self.storage.begin().await?;
        if let Some(row) = sqlx::query(
            "SELECT fingerprint,owner,decision_id,expires FROM request_keys WHERE key=$1",
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
                let result: (i64, String) = sqlx::query_as(self.storage.response_lookup())
                    .bind(&self.tenant_id)
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(RequestStart::Replay(
                    result.0 as u16,
                    serde_json::from_str(&result.1)?,
                ));
            }
            anyhow::ensure!(row.get::<i64, _>("expires") <= now, "E_REQUEST_IN_PROGRESS");
        }
        sqlx::query("DELETE FROM request_keys WHERE decision_id IS NULL AND expires<=$1")
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
        sqlx::query("INSERT INTO request_keys(key,fingerprint,owner,expires) VALUES($1,$2,$3,$4)")
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
        // Serialize with reservation/commit, including across PostgreSQL hosts.
        // If storage is unavailable the bounded reservation expires naturally.
        let result: anyhow::Result<()> = async {
            let mut tx = self.storage.begin().await?;
            sqlx::query(
                "DELETE FROM request_keys WHERE key=$1 AND owner=$2 AND decision_id IS NULL",
            )
            .bind(&reservation.key)
            .bind(&reservation.owner)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        let _ = result;
    }

    /// A crashed or disconnected consumer leaves a lease that becomes eligible
    /// again. Stable digest is the consumer's idempotency key across retries.
    pub async fn claim(&self, now_ms: i64) -> anyhow::Result<Value> {
        let _permit = self
            .admission
            .try_acquire()
            .map_err(|_| anyhow::anyhow!("Journal admission full"))?;
        let mut tx = self.storage.begin().await?;
        let rows = sqlx::query("SELECT seq,digest,body,input,response,bytes,attempts FROM events WHERE kind='decision-record' AND delivered=0 AND retry_at<=$1 ORDER BY seq LIMIT 100")
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
            sqlx::query("UPDATE events SET lease=$1,retry_at=$2,attempts=attempts+1 WHERE seq=$3")
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
        let mut tx = self.storage.begin().await?;
        for digest in digests {
            let n = sqlx::query(
                "UPDATE events SET delivered=1 WHERE digest=$1 AND lease=$2 AND retry_at>$3 ",
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
