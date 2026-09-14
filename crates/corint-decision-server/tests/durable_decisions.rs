use corint_decision_server::journal::{Journal, JournalConfig};
use corint_decision_toolchain::phase0::Ingest;
use serde_json::{json, Value};
use std::path::Path;
fn fixture(kind: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tests/conformance/contracts/phase0/{kind}.json"
        )))
        .unwrap(),
    )
    .unwrap()
}
fn config() -> JournalConfig {
    JournalConfig {
        path: "journal.sqlite".into(),
        tenant_id: "fixture-tenant".into(),
        max_records: 100,
        max_bytes: 1_000_000,
        consumer_token_env: "UNUSED".into(),
    }
}
#[tokio::test]
async fn restart_recovers_decision_evidence_and_unacknowledged_delivery() {
    let dir = tempfile::tempdir().unwrap();
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    let decision = fixture("decision-record");
    let input = json!({"amount":100});
    j.append("decision-record", &decision, Some(&input))
        .await
        .unwrap();
    for kind in ["outcome-event", "action-receipt"] {
        assert!(j.append(kind, &fixture(kind), None).await.is_err());
    }
    let first = j.claim(1000).await.unwrap();
    assert_eq!(first["events"].as_array().unwrap().len(), 1);
    assert!(j.claim(1001).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
    drop(j);
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    assert_eq!(
        j.append("decision-record", &decision, Some(&input))
            .await
            .unwrap(),
        Ingest::Duplicate
    );
    let retry = j.claim(61_001).await.unwrap();
    let keys = vec![retry["events"][0]["idempotency_key"]
        .as_str()
        .unwrap()
        .to_owned()];
    assert_eq!(
        first["events"][0]["idempotency_key"],
        retry["events"][0]["idempotency_key"]
    );
    assert_eq!(retry["events"][0]["attempt"], 2);
    assert!(j
        .acknowledge(first["lease"].as_str().unwrap(), &keys, 61_002)
        .await
        .is_err());
    j.acknowledge(retry["lease"].as_str().unwrap(), &keys, 61_002)
        .await
        .unwrap();
    j.acknowledge(retry["lease"].as_str().unwrap(), &keys, 61_003)
        .await
        .unwrap();
    assert!(j.claim(10_000_000).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
    let pool = pool(dir.path()).await;
    let stored: String = sqlx::query_scalar("SELECT input FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), input);
}

async fn pool(root: &Path) -> sqlx::SqlitePool {
    sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.join("journal.sqlite"))
            .create_if_missing(true),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn capacity_tenant_and_conflicts_fail_without_partial_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_records = 1;
    let j = Journal::open(dir.path(), &config).await.unwrap();
    let decision = fixture("decision-record");
    let input = json!({"amount":100});
    j.append("decision-record", &decision, Some(&input))
        .await
        .unwrap();
    assert_eq!(
        j.append("decision-record", &decision, None).await.unwrap(),
        Ingest::Duplicate
    );
    let mut conflict = decision.clone();
    conflict["result"] = json!("pass");
    assert!(j.append("decision-record", &conflict, None).await.is_err());
    assert!(j
        .append("decision-record", &decision, Some(&json!({"amount":200})))
        .await
        .is_err());
    conflict["decision_id"] = json!("new-decision");
    assert!(j.append("decision-record", &conflict, None).await.is_err());
    conflict["tenant_id"] = json!("other");
    assert!(j.append("decision-record", &conflict, None).await.is_err());
    let pool = pool(dir.path()).await;
    let row: (String, String) = sqlx::query_as("SELECT body,input FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&row.0).unwrap(), decision);
    assert_eq!(serde_json::from_str::<Value>(&row.1).unwrap(), input);
    let usage: (i64, i64) = sqlx::query_as("SELECT record_count,total_bytes FROM journal_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        usage,
        (
            1,
            (decision.to_string().len() + input.to_string().len()) as i64
        )
    );
    config.tenant_id = "other".into();
    assert!(Journal::open(dir.path(), &config).await.is_err());
}

#[tokio::test]
async fn concurrent_connections_cannot_overwrite_a_decision_or_overbook_capacity() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_records = 1;
    let a = Journal::open(dir.path(), &config).await.unwrap();
    let b = Journal::open(dir.path(), &config).await.unwrap();
    let first = fixture("decision-record");
    let mut second = first.clone();
    second["result"] = json!("pass");
    let (ra, rb) = tokio::join!(
        a.append("decision-record", &first, None),
        b.append("decision-record", &second, None)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
    second["decision_id"] = json!("another-decision");
    assert!(b.append("decision-record", &second, None).await.is_err());
    let pool = pool(dir.path()).await;
    let count: i64 = sqlx::query_scalar("SELECT record_count FROM journal_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn invalid_decision_semantics_are_rejected_without_feedback_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    let mut decision = fixture("decision-record");
    decision["result"] = json!("error");
    assert!(j.append("decision-record", &decision, None).await.is_err());
    let mut decision = fixture("decision-record");
    let action = decision["actions"][0].clone();
    decision["actions"].as_array_mut().unwrap().push(action);
    assert!(j.append("decision-record", &decision, None).await.is_err());
    assert!(j.claim(0).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn old_journal_is_upgraded_once_without_replaying_or_exporting_feedback() {
    use corint_decision_server::journal::contract;
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(dir.path()).await;
    sqlx::query("CREATE TABLE events (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, digest TEXT NOT NULL UNIQUE, body TEXT NOT NULL, input TEXT, bytes INTEGER NOT NULL, delivered INTEGER NOT NULL DEFAULT 0, lease TEXT, retry_at INTEGER NOT NULL DEFAULT 0, attempts INTEGER NOT NULL DEFAULT 0)").execute(&pool).await.unwrap();
    let decision = fixture("decision-record");
    // Deliberately lacks a valid feedback chain: online startup must not need it.
    let mut outcome = fixture("outcome-event");
    outcome["decision_id"] = json!("unknown-external-decision");
    for (kind, value) in [("decision-record", &decision), ("outcome-event", &outcome)] {
        sqlx::query("INSERT INTO events(kind,digest,body,input,bytes) VALUES(?,?,?,?,?)")
            .bind(kind)
            .bind(contract(kind, value).unwrap().sha256())
            .bind(value.to_string())
            .bind("{}")
            .bind((value.to_string().len() + 2) as i64)
            .execute(&pool)
            .await
            .unwrap();
    }
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    assert_eq!(
        j.claim(0).await.unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2); // Existing feedback is preserved, never deleted.
    drop(j);
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    let mut next = decision.clone();
    next["decision_id"] = json!("after-restart");
    j.append("decision-record", &next, None).await.unwrap();
    let usage: i64 = sqlx::query_scalar("SELECT record_count FROM journal_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(usage, 3);
}

#[tokio::test]
async fn append_uses_indexed_identity_and_counters_without_revalidating_history() {
    use sqlx::Row;
    let dir = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_records = 20_000;
    config.max_bytes = 100_000_000;
    let j = Journal::open(dir.path(), &config).await.unwrap();
    let pool = pool(dir.path()).await;
    // Populate historical rows with deliberately stale digests. An online append
    // must not revalidate unrelated historical content; a whole-ledger replay fails.
    sqlx::query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO events(kind,digest,body,bytes) SELECT 'decision-record',printf('historical-%d',x),json_set(?,'$.decision_id',printf('historical-%d',x)),2000 FROM n")
        .bind(fixture("decision-record").to_string()).execute(&pool).await.unwrap();
    let record = fixture("decision-record");
    assert_eq!(
        j.append("decision-record", &record, None).await.unwrap(),
        Ingest::Inserted
    );
    let usage: i64 = sqlx::query_scalar("SELECT record_count FROM journal_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(usage, 10_001);
    let plan = sqlx::query("EXPLAIN QUERY PLAN SELECT digest,input FROM events WHERE kind='decision-record' AND json_extract(body,'$.tenant_id')=? AND json_extract(body,'$.decision_id')=?")
        .bind("fixture-tenant").bind("decision-1").fetch_all(&pool).await.unwrap();
    assert!(plan.iter().any(|row| row
        .get::<String, _>("detail")
        .contains("journal_decision_identity")));
}

#[tokio::test]
async fn storage_failure_rolls_back_record_input_and_usage() {
    let dir = tempfile::tempdir().unwrap();
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    let pool = pool(dir.path()).await;
    sqlx::query("CREATE TRIGGER fail_write AFTER INSERT ON events BEGIN SELECT RAISE(ABORT,'simulated storage failure'); END")
        .execute(&pool).await.unwrap();
    assert!(j
        .append(
            "decision-record",
            &fixture("decision-record"),
            Some(&json!({"amount":100}))
        )
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let usage: (i64, i64) = sqlx::query_as("SELECT record_count,total_bytes FROM journal_usage")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(usage, (0, 0));
}

#[tokio::test]
async fn failed_legacy_upgrade_keeps_all_rows_and_rolls_back_indexes() {
    use corint_decision_server::journal::contract;
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(dir.path()).await;
    sqlx::query("CREATE TABLE events (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, digest TEXT NOT NULL UNIQUE, body TEXT NOT NULL, input TEXT, bytes INTEGER NOT NULL, delivered INTEGER NOT NULL DEFAULT 0, lease TEXT, retry_at INTEGER NOT NULL DEFAULT 0, attempts INTEGER NOT NULL DEFAULT 0)")
        .execute(&pool).await.unwrap();
    let first = fixture("decision-record");
    let mut second = first.clone();
    second["result"] = json!("pass");
    for value in [&first, &second] {
        sqlx::query("INSERT INTO events(kind,digest,body,bytes) VALUES('decision-record',?,?,?)")
            .bind(contract("decision-record", value).unwrap().sha256())
            .bind(value.to_string())
            .bind(value.to_string().len() as i64)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert!(Journal::open(dir.path(), &config()).await.is_err());
    let rows: Vec<String> = sqlx::query_scalar("SELECT body FROM events ORDER BY seq")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows, vec![first.to_string(), second.to_string()]);
    let upgrades: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name IN ('journal_usage','journal_decision_identity')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(upgrades, 0);
}
