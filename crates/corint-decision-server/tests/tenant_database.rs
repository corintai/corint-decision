use corint_decision_server::{
    journal::{Journal, JournalConfig, RequestStart},
    tenancy::{
        store::{Store, StoreConfig},
        Scope,
    },
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn scope(tenant: &str) -> Scope {
    Scope {
        tenant_id: tenant.into(),
        environment: "prod".into(),
        deployment: "risk".into(),
    }
}
fn actor(scope: &Scope) -> Value {
    json!({"tenant_id":scope.tenant_id,"environment":scope.environment,"deployment":scope.deployment,"principal_id":"operator","actor_id":"agent","request_id":"request"})
}

#[tokio::test]
async fn control_migration_preserves_history_and_forces_owner_on_reads_and_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE tenant_runtimes(scope_key TEXT PRIMARY KEY, paused BIGINT NOT NULL, revision BIGINT NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tenant_management_audit(scope_key TEXT NOT NULL, revision BIGINT NOT NULL, event TEXT NOT NULL, PRIMARY KEY(scope_key,revision))").execute(&pool).await.unwrap();
    let a = scope("a");
    let b = scope("b");
    sqlx::query("INSERT INTO tenant_runtimes VALUES($1,0,1)")
        .bind(a.key())
        .execute(&pool)
        .await
        .unwrap();
    let old = json!({"scope":a,"revision":1,"paused":false,"actor":actor(&a)});
    sqlx::query("INSERT INTO tenant_management_audit VALUES($1,1,$2)")
        .bind(a.key())
        .bind(old.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let config = StoreConfig::Sqlite { path };
    let known = BTreeSet::from([a.clone(), b.clone()]);
    let store = Store::open(dir.path(), &config, &known).await.unwrap();
    assert_eq!(store.audit(&a).await.unwrap(), vec![old.clone()]);
    assert!(store.audit(&b).await.unwrap().is_empty());
    assert!(store.set_paused(&a, 1, true, &actor(&b)).await.is_err());
    assert!(!store.state(&a).await.unwrap().paused);
    assert!(store.set_paused(&a, 1, true, &actor(&a)).await.unwrap());
    assert_eq!(store.state(&a).await.unwrap().revision, 2);
    assert!(!store.state(&b).await.unwrap().paused);
    assert_eq!(store.audit(&a).await.unwrap().len(), 2);
    assert!(store.audit(&b).await.unwrap().is_empty());
    assert!(store.state(&scope("unknown")).await.is_err());
    assert!(store.audit(&scope("")).await.is_err());
    // Missing identity and changing the owner independently of scope both fail.
    assert!(
        sqlx::query("INSERT INTO tenant_runtimes(scope_key,paused,revision) VALUES($1,0,0)")
            .bind(scope("missing").key())
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE tenant_runtimes SET tenant_id='b' WHERE scope_key=$1")
            .bind(a.key())
            .execute(&pool)
            .await
            .is_err()
    );
    drop(store);
    let store = Store::open(dir.path(), &config, &known).await.unwrap();
    assert_eq!(store.state(&a).await.unwrap().revision, 2);
    assert_eq!(store.audit(&a).await.unwrap()[1], old);
    let owners: Vec<String> =
        sqlx::query_scalar("SELECT tenant_id FROM tenant_runtimes ORDER BY tenant_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(owners, vec!["a", "b"]);
    pool.close().await;
}

#[tokio::test]
async fn malformed_legacy_scope_rolls_back_the_entire_control_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE tenant_runtimes(scope_key TEXT PRIMARY KEY, paused BIGINT NOT NULL, revision BIGINT NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tenant_runtimes VALUES('{}',1,7)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(Store::open(
        dir.path(),
        &StoreConfig::Sqlite { path },
        &BTreeSet::from([scope("local")])
    )
    .await
    .is_err());
    let columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('tenant_runtimes') WHERE name='tenant_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(columns, 0);
    let state: (i64, i64) = sqlx::query_as("SELECT paused,revision FROM tenant_runtimes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, (1, 7));
    pool.close().await;
}

#[tokio::test]
async fn reservations_and_records_cannot_be_transferred_to_another_tenant() {
    let dir = tempfile::tempdir().unwrap();
    let config = |tenant| {
        serde_json::from_value::<JournalConfig>(json!({"tenant_id":tenant,"path":format!("{tenant}.sqlite"),"max_records":10,"max_bytes":100000})).unwrap()
    };
    let a = Journal::open(dir.path(), &config("a")).await.unwrap();
    let b = Journal::open(dir.path(), &config("b")).await.unwrap();
    let RequestStart::Reserved(reservation) = a
        .begin_request(Some("same"), "input", chrono::Utc::now().timestamp_millis())
        .await
        .unwrap()
    else {
        panic!()
    };
    let mut record: Value = serde_json::from_str(include_str!(
        "../../../tests/conformance/contracts/phase0/decision-record.json"
    ))
    .unwrap();
    record["tenant_id"] = "b".into();
    assert!(b
        .append_response(
            "decision-record",
            &record,
            None,
            Some((&reservation, 200, &json!({})))
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("Reservation tenant scope mismatch"));
    b.abandon(&reservation).await;
    assert_eq!(a.status().await.unwrap()["inflight_requests"], 1);
    assert_eq!(b.status().await.unwrap()["inflight_requests"], 0);
    assert!(a.append("decision-record", &record, None).await.is_err());
    a.abandon(&reservation).await;
    assert_eq!(a.status().await.unwrap()["inflight_requests"], 0);
}
