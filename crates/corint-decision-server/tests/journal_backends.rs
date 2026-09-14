//! Identical protocol contracts run against SQLite and a real PostgreSQL database.
use corint_decision_server::journal::{Journal, JournalBackend, JournalConfig, RequestStart};
use corint_decision_toolchain::phase0::Ingest;
use serde_json::{json, Value};
use std::path::Path;

fn config(postgres: bool, max_records: u32) -> JournalConfig {
    let backend = if postgres {
        assert!(
            std::env::var("CORINT_TEST_POSTGRES_URL").is_ok(),
            "Use tests/scripts/run_journal_postgres_tests.py"
        );
        JournalBackend::Postgres {
            url_env: "CORINT_TEST_POSTGRES_URL".into(),
            schema: format!("journal_test_{}", uuid::Uuid::new_v4().simple()),
        }
    } else {
        JournalBackend::Sqlite {}
    };
    JournalConfig {
        backend,
        path: if postgres {
            Default::default()
        } else {
            "journal.sqlite".into()
        },
        best_effort: false,
        export_replay: true,
        tenant_id: "fixture-tenant".into(),
        max_records,
        max_bytes: 1_000_000,
        consumer_token_env: String::new(),
    }
}
fn decision() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/conformance/contracts/phase0/decision-record.json"
    ))
    .unwrap()
}
async fn cleanup(config: &JournalConfig) {
    if let JournalBackend::Postgres { url_env, schema } = &config.backend {
        let pool = sqlx::PgPool::connect(&std::env::var(url_env).unwrap())
            .await
            .unwrap();
        sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
}
async fn protocol(postgres: bool) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(postgres, 1);
    // First startup itself is concurrent: migrations must run atomically once.
    let (a, b) = tokio::join!(
        Journal::open(dir.path(), &cfg),
        Journal::open(dir.path(), &cfg)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    let RequestStart::Reserved(old) = a
        .begin_request(Some("payment-1"), "input-hash", 1000)
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(b
        .begin_request(Some("payment-1"), "changed", 1001)
        .await
        .unwrap_err()
        .to_string()
        .contains("E_IDEMPOTENCY_CONFLICT"));
    assert!(b
        .begin_request(Some("payment-1"), "input-hash", 1001)
        .await
        .unwrap_err()
        .to_string()
        .contains("E_REQUEST_IN_PROGRESS"));
    assert!(b
        .begin_request(Some("another"), "input-hash", 1001)
        .await
        .is_err());
    let RequestStart::Reserved(new) = b
        .begin_request(Some("payment-1"), "input-hash", 121001)
        .await
        .unwrap()
    else {
        panic!()
    };
    let record = decision();
    let input = json!({"amount": 100});
    let response = json!({"record":record,"persistence":"durable"});
    assert!(a
        .append_response(
            "decision-record",
            &record,
            Some(&input),
            Some((&old, 200, &response))
        )
        .await
        .is_err());
    a.abandon(&old).await; // An expired owner must not delete the new reservation.
    b.append_response(
        "decision-record",
        &record,
        Some(&input),
        Some((&new, 200, &response)),
    )
    .await
    .unwrap();
    assert_eq!(
        a.append("decision-record", &record, Some(&input))
            .await
            .unwrap(),
        Ingest::Duplicate
    );
    assert!(a
        .append("decision-record", &record, Some(&json!({"amount": 200})))
        .await
        .is_err());
    let mut conflict = record.clone();
    conflict["result"] = "pass".into();
    assert!(a.append("decision-record", &conflict, None).await.is_err());
    assert!(a.append("outcome-event", &record, None).await.is_err());
    let status = a.status().await.unwrap();
    assert_eq!(
        status["backend"],
        if postgres { "postgres" } else { "sqlite" }
    );
    assert_eq!(status["stored_records"], 1);
    assert_eq!(status["accepting"], false);
    let first = a.claim(121002).await.unwrap();
    assert_eq!(first["events"][0]["input_evidence"], input);
    assert_eq!(first["events"][0]["response"], response);
    assert_eq!(b.claim(121003).await.unwrap()["events"], json!([]));
    drop(a);
    drop(b);
    let restarted = Journal::open(dir.path(), &cfg).await.unwrap();
    let RequestStart::Replay(code, frozen) = restarted
        .begin_request(Some("payment-1"), "input-hash", 181003)
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!((code, frozen), (200, response));
    let retry = restarted.claim(181003).await.unwrap();
    assert_eq!(retry["events"][0]["attempt"], 2);
    let digest = retry["events"][0]["idempotency_key"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(restarted
        .acknowledge(first["lease"].as_str().unwrap(), &[digest.clone()], 181004)
        .await
        .is_err());
    // Partial acknowledgement must roll back all updates.
    assert!(restarted
        .acknowledge(
            retry["lease"].as_str().unwrap(),
            &[digest.clone(), "unknown".into()],
            181004
        )
        .await
        .is_err());
    let again = restarted.claim(301004).await.unwrap();
    assert_eq!(again["events"][0]["attempt"], 3);
    restarted
        .acknowledge(again["lease"].as_str().unwrap(), &[digest.clone()], 301005)
        .await
        .unwrap();
    restarted
        .acknowledge(again["lease"].as_str().unwrap(), &[digest], 301006)
        .await
        .unwrap();
    assert_eq!(
        restarted.claim(1_000_000).await.unwrap()["events"],
        json!([])
    );
    assert!(!dir.path().join("journal.sqlite").exists() || !postgres);
    let mut wrong_tenant = cfg.clone();
    wrong_tenant.tenant_id = "other".into();
    assert!(Journal::open(dir.path(), &wrong_tenant).await.is_err());
    drop(restarted);
    cleanup(&cfg).await;
}

async fn concurrency(postgres: bool) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(postgres, 1);
    let a = Journal::open(dir.path(), &cfg).await.unwrap();
    let b = Journal::open(dir.path(), &cfg).await.unwrap();
    let (ra, rb) = tokio::join!(
        a.begin_request(Some("same"), "same", 0),
        b.begin_request(Some("same"), "same", 0)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
    let RequestStart::Reserved(owner) = ra.or(rb).unwrap() else {
        panic!()
    };
    a.abandon(&owner).await;
    let (ra, rb) = tokio::join!(
        a.begin_request(Some("one"), "same", 0),
        b.begin_request(Some("two"), "same", 0)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
    let RequestStart::Reserved(owner) = ra.or(rb).unwrap() else {
        panic!()
    };
    b.abandon(&owner).await;
    let record = decision();
    let mut second = record.clone();
    second["decision_id"] = "second".into();
    let (ra, rb) = tokio::join!(
        a.append("decision-record", &record, None),
        b.append("decision-record", &second, None)
    );
    assert_ne!(ra.is_ok(), rb.is_ok());
    assert_eq!(a.status().await.unwrap()["stored_records"], 1);
    let (ra, rb) = tokio::join!(a.claim(1000), b.claim(1000));
    let count = ra.unwrap()["events"].as_array().unwrap().len()
        + rb.unwrap()["events"].as_array().unwrap().len();
    assert_eq!(count, 1);
    drop(a);
    drop(b);
    cleanup(&cfg).await;
}

async fn byte_capacity_and_private_export(postgres: bool) {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(postgres, 10);
    cfg.max_bytes = 1024;
    cfg.export_replay = false;
    let a = Journal::open(dir.path(), &cfg).await.unwrap();
    let RequestStart::Reserved(owner) = a
        .begin_request(Some("too-large"), "input", 0)
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(a
        .append_response(
            "decision-record",
            &decision(),
            None,
            Some((&owner, 200, &json!({"large":"x".repeat(2048)})))
        )
        .await
        .is_err());
    assert_eq!(a.status().await.unwrap()["stored_records"], 0);
    a.abandon(&owner).await;
    drop(a);
    cfg.max_bytes = 1_000_000;
    let a = Journal::open(dir.path(), &cfg).await.unwrap();
    a.append(
        "decision-record",
        &decision(),
        Some(&json!({"private":true})),
    )
    .await
    .unwrap();
    let claimed = a.claim(0).await.unwrap();
    assert!(claimed["events"][0].get("input_evidence").is_none());
    assert!(claimed["events"][0].get("response").is_none());
    drop(a);
    cleanup(&cfg).await;
}

#[tokio::test]
async fn sqlite_shared_protocol() {
    protocol(false).await;
}
#[tokio::test]
async fn sqlite_shared_concurrency() {
    concurrency(false).await;
}
#[tokio::test]
async fn sqlite_shared_capacity_and_privacy() {
    byte_capacity_and_private_export(false).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run_journal_postgres_tests.py"]
async fn postgres_shared_protocol() {
    protocol(true).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run_journal_postgres_tests.py"]
async fn postgres_shared_concurrency() {
    concurrency(true).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run_journal_postgres_tests.py"]
async fn postgres_shared_capacity_and_privacy() {
    byte_capacity_and_private_export(true).await;
}

#[tokio::test]
async fn backend_config_rejects_ambiguous_or_invalid_targets() {
    let dir = tempfile::tempdir().unwrap();
    let base = json!({"tenant_id":"test","max_records":10,"max_bytes":10000});
    let cfg: JournalConfig = serde_json::from_value(base.clone()).unwrap();
    assert!(Journal::open(dir.path(), &cfg).await.is_err());
    for schema in [
        "public",
        "pg_catalog",
        "bad; DROP TABLE events",
        "",
        "UPPER",
    ] {
        let mut invalid = base.clone();
        invalid["backend"] =
            json!({"type":"postgres","url_env":"NO_SUCH_TEST_DATABASE", "schema":schema});
        let cfg: JournalConfig = serde_json::from_value(invalid).unwrap();
        assert!(Journal::open(Path::new("/must-not-create-files"), &cfg)
            .await
            .is_err());
    }
    let mut both = base.clone();
    both["path"] = "must-not-create.sqlite".into();
    both["backend"] =
        json!({"type":"postgres","url_env":"NO_SUCH_TEST_DATABASE","schema":"journal_test"});
    assert!(
        Journal::open(dir.path(), &serde_json::from_value(both).unwrap())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let mut unknown = base;
    unknown["backend"] = json!({"type":"sqlite","url_env":"INVALID"});
    assert!(serde_json::from_value::<JournalConfig>(unknown).is_err());
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL; run_journal_postgres_tests.py"]
async fn postgres_shared_rejects_unrelated_schema_without_mutating_it() {
    let cfg = config(true, 10);
    let JournalBackend::Postgres { url_env, schema } = &cfg.backend else {
        unreachable!()
    };
    let pool = sqlx::PgPool::connect(&std::env::var(url_env).unwrap())
        .await
        .unwrap();
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&format!("CREATE TABLE \"{schema}\".events (body TEXT)"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&format!(
        "INSERT INTO \"{schema}\".events VALUES('existing business data')"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    assert!(Journal::open(dir.path(), &cfg).await.is_err());
    let value: String = sqlx::query_scalar(&format!("SELECT body FROM \"{schema}\".events"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, "existing business data");
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=$1")
            .bind(schema)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    cleanup(&cfg).await;
    pool.close().await;
}
