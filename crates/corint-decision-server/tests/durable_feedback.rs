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
async fn restart_recovers_feedback_corrections_receipts_and_unacknowledged_delivery() {
    let dir = tempfile::tempdir().unwrap();
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    let decision = fixture("decision-record");
    let event = fixture("outcome-event");
    let receipt = fixture("action-receipt");
    assert!(j.append("outcome-event", &event, None).await.is_err());
    j.append("decision-record", &decision, Some(&json!({"amount":100})))
        .await
        .unwrap();
    j.append("outcome-event", &event, None).await.unwrap();
    j.append("action-receipt", &receipt, None).await.unwrap();
    let first = j.claim(1000).await.unwrap();
    assert_eq!(first["events"].as_array().unwrap().len(), 3);
    assert!(j.claim(1001).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
    drop(j);
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    for (kind, event) in [
        ("decision-record", &decision),
        ("outcome-event", &event),
        ("action-receipt", &receipt),
    ] {
        assert_eq!(
            j.append(kind, event, None).await.unwrap(),
            Ingest::Duplicate
        );
    }
    let mut corrected = event.clone();
    corrected["id"] = json!("correction");
    corrected["supersedes"] = event["id"].clone();
    corrected["label_version"] = json!(2);
    corrected["label"] = json!("negative");
    corrected["available_at_ms"] = json!(400);
    j.append("outcome-event", &corrected, None).await.unwrap();
    assert_eq!(
        j.outcome_as_of("decision-1", "fraud", 399)
            .await
            .unwrap()
            .unwrap()["label"],
        "positive"
    );
    assert_eq!(
        j.outcome_as_of("decision-1", "fraud", 400)
            .await
            .unwrap()
            .unwrap()["label"],
        "negative"
    );
    let retry = j.claim(61_001).await.unwrap();
    let keys: Vec<String> = retry["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["idempotency_key"].as_str().unwrap().into())
        .collect();
    assert_eq!(keys.len(), 4);
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
    // Duplicate acknowledgement under a still-valid lease is idempotent.
    j.acknowledge(retry["lease"].as_str().unwrap(), &keys, 61_003)
        .await
        .unwrap();
    assert!(j.claim(10_000_000).await.unwrap()["events"]
        .as_array()
        .unwrap()
        .is_empty());
    drop(j);
    let j = Journal::open(dir.path(), &config()).await.unwrap();
    assert_eq!(
        j.outcome_as_of("decision-1", "fraud", 400)
            .await
            .unwrap()
            .unwrap()["id"],
        "correction"
    );
}
#[tokio::test]
async fn capacity_tenant_and_conflicts_fail_without_partial_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config();
    config.max_records = 1;
    let j = Journal::open(dir.path(), &config).await.unwrap();
    let mut decision = fixture("decision-record");
    decision["result"] = json!("hold");
    j.append("decision-record", &decision, None).await.unwrap();
    assert_eq!(
        j.append("decision-record", &decision, None).await.unwrap(),
        Ingest::Duplicate
    );
    let mut conflict = decision.clone();
    conflict["result"] = json!("pass");
    assert!(j.append("decision-record", &conflict, None).await.is_err());
    assert!(j
        .append("outcome-event", &fixture("outcome-event"), None)
        .await
        .is_err());
    conflict["tenant_id"] = json!("other");
    assert!(j.append("decision-record", &conflict, None).await.is_err());
    assert_eq!(
        j.claim(0).await.unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    config.tenant_id = "other".into();
    assert!(Journal::open(dir.path(), &config).await.is_err());
}
#[tokio::test]
async fn concurrent_conflicting_labels_have_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let a = Journal::open(dir.path(), &config()).await.unwrap();
    let b = Journal::open(dir.path(), &config()).await.unwrap();
    a.append("decision-record", &fixture("decision-record"), None)
        .await
        .unwrap();
    let first = fixture("outcome-event");
    let mut second = first.clone();
    second["id"] = json!("other");
    second["label"] = json!("negative");
    let (ra, rb) = tokio::join!(
        a.append("outcome-event", &first, None),
        b.append("outcome-event", &second, None)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
}
