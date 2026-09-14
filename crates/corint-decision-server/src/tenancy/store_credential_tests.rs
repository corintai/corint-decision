use super::super::auth::{Change, Registry};
use super::*;
use sha2::{Digest, Sha256};
#[tokio::test]
async fn credential_revision_conflict_preserves_committed_state_and_audit() {
    let dir = tempfile::tempdir().unwrap();
    let config = StoreConfig::Sqlite {
        path: "control.db".into(),
    };
    let a = Store::open(dir.path(), &config, &BTreeSet::new())
        .await
        .unwrap();
    let b = Store::open(dir.path(), &config, &BTreeSet::new())
        .await
        .unwrap();
    let digest: [u8; 32] =
        Sha256::digest(b"synthetic-platform-credential-at-least-32-chars").into();
    let initial: Registry = serde_json::from_value(json!({"entries":{"admin":{"principal":{"id":"admin","platform_admin":true},"digest":digest}}})).unwrap();
    a.bootstrap_credentials(&initial).await.unwrap();
    let (revision, mut first) = a.credentials().await.unwrap().unwrap();
    let (_, mut second) = b.credentials().await.unwrap().unwrap();
    first
        .change(Change::Rotate { id: "admin".into() }, &BTreeSet::new())
        .unwrap();
    second
        .change(Change::Rotate { id: "admin".into() }, &BTreeSet::new())
        .unwrap();
    assert!(a
        .save_credentials(revision, &first, "admin", "rotate", "admin")
        .await
        .unwrap());
    assert!(!b
        .save_credentials(revision, &second, "admin", "rotate", "admin")
        .await
        .unwrap());
    // An old seed cannot replace an existing database registry either.
    b.bootstrap_credentials(&initial).await.unwrap();
    let (current, actual) = b.credentials().await.unwrap().unwrap();
    assert_eq!(current, revision + 1);
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(first).unwrap()
    );
    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenant_credential_audit")
        .fetch_one(&b.pool)
        .await
        .unwrap();
    assert_eq!(audit, 1);
}
