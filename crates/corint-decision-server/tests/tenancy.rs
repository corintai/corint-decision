use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_decision_compiler::core::CoreSource;
use corint_decision_engine::decision_host::canonical_sha256;
use corint_decision_server::tenancy::{self, Scope};
use corint_decision_toolchain::transfer::SourceBundle;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use tempfile::TempDir;
use tower::ServiceExt;
#[path = "../../../tests/support/core_repository.rs"]
mod repository_fixture;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance")
            .join(name),
    )
    .unwrap()
}
fn bundle(change: bool) -> SourceBundle {
    let source = |name: &str| CoreSource {
        path: name.into(),
        yaml: fixture(&format!("cdl_core/{name}")),
    };
    let mut sources: Vec<_> = [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
    ]
    .iter()
    .map(|n| source(n))
    .collect();
    if change {
        sources[0].yaml = sources[0]
            .yaml
            .replace("> 1000", "> 1000 && event.amount < 2000");
    }
    SourceBundle::new(source("input-schema.yaml"), sources).unwrap()
}
fn write(path: &Path, v: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(v).unwrap()).unwrap();
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn scope(tenant: &str, environment: &str) -> Value {
    json!({"tenant_id":tenant,"environment":environment,"deployment":"risk"})
}
fn path(tenant: &str, environment: &str, route: &str) -> String {
    format!("/v1/tenants/{tenant}/environments/{environment}/deployments/risk/{route}")
}
struct Setup {
    dir: TempDir,
    config: Value,
    credentials: Value,
    tokens: BTreeMap<String, String>,
    envs: Vec<String>,
}
impl Drop for Setup {
    fn drop(&mut self) {
        for name in &self.envs {
            std::env::remove_var(name);
        }
    }
}
impl Setup {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let limits =
            json!({"max_inflight":16,"requests_per_second":1000,"burst":1000,"max_connections":64});
        let context = fixture("contracts/business-context.yaml");
        let target = fixture("contracts/target-capabilities.json");
        let cases = fixture("cdl_core/behavior.yaml");
        let mut deployments = vec![];
        for (tenant, env) in [("local", "prod"), ("acme", "prod"), ("local", "dev")] {
            let scope = scope(tenant, env);
            let subdir = format!("{tenant}_{env}");
            let root = dir.path().join(&subdir);
            std::fs::create_dir(&root).unwrap();
            for (name, text) in [
                ("context.yaml", &context),
                ("target.json", &target),
                ("cases.yaml", &cases),
            ] {
                std::fs::write(root.join(name), text).unwrap();
            }
            repository_fixture::publish(&root, &bundle(false), "initial");
            let approvals: Vec<_> = [false,true].iter().map(|change| json!({"policy_sha256":repository_fixture::identity(&bundle(*change)),"context_sha256":hash(&context),"target_sha256":hash(&target),"cases_sha256":hash(&cases),
                "tenant_scope":scope,"resource_scope_sha256":canonical_sha256(&json!({"scope":scope,"resources":[]}))})).collect();
            write(
                &root.join("core.json"),
                &json!({"config_version":"3","listen":"127.0.0.1:0","context":"context.yaml","target":"target.json","cases":"cases.yaml","repository":"repository",
                "decision_token_env":"","publisher_token_env":"","approvals":approvals,"journal":{"path":"journal.db","tenant_id":tenant,"max_records":1000,"max_bytes":100_000_000,"export_replay":true}}),
            );
            deployments.push(json!({"scope":scope,"root":subdir,"core_config":"core.json","limits":limits,"timeout_ms":30000,"idle_seconds":1,"resources":[]}));
        }
        let mut envs = vec![];
        let mut tokens = BTreeMap::new();
        let mut principals = vec![];
        let mut add = |id: &str, grant: Value, parent: Option<&str>, admin: bool, expired: bool| {
            let env = format!("CORINT_TENANT_TEST_{}", uuid::Uuid::new_v4().simple());
            let token = format!("test-only-{id}-{}", uuid::Uuid::new_v4());
            std::env::set_var(&env, &token);
            envs.push(env.clone());
            tokens.insert(id.into(), token);
            let mut p = json!({"id":id,"token_env":env,"platform_admin":admin,"grants":grant});
            if let Some(parent) = parent {
                p["delegated_by"] = parent.into();
                p["expires_at_ms"] = (chrono::Utc::now().timestamp_millis() + 600000).into();
            }
            if expired {
                p["expires_at_ms"] = 1.into();
            }
            principals.push(p);
        };
        let grant = |s: Value, permissions: Value| json!([{"scope":s,"permissions":permissions}]);
        add("admin", json!([]), None, true, false);
        for (id, tenant, env) in [
            ("local", "local", "prod"),
            ("acme", "acme", "prod"),
            ("dev", "local", "dev"),
        ] {
            add(
                id,
                grant(
                    scope(tenant, env),
                    json!(["decide", "inspect", "publish", "consume", "export", "manage"]),
                ),
                None,
                false,
                false,
            );
        }
        add(
            "consumer",
            grant(scope("local", "prod"), json!(["consume"])),
            None,
            false,
            false,
        );
        add(
            "agent",
            grant(scope("local", "prod"), json!(["decide"])),
            Some("local"),
            false,
            false,
        );
        add(
            "expired",
            grant(scope("local", "prod"), json!(["decide"])),
            Some("local"),
            false,
            true,
        );
        let credentials = json!({"format_version":"1","principals":principals});
        write(&dir.path().join("credentials.json"), &credentials);
        let config = json!({"format_version":"1","listen":"127.0.0.1:0","credentials":"credentials.json","control_store":{"type":"sqlite","path":"control.db"},
            "platform_limits":limits,"tenant_limits":{"local":limits,"acme":limits},"max_loaded":4,"max_preparations":2,"deployments":deployments,"decision_bindings":{"local":scope("local","prod"),"acme":scope("acme","prod")}});
        Self {
            dir,
            config,
            credentials,
            tokens,
            envs,
        }
    }
    async fn app(&self) -> Router {
        tenancy::create_router(
            serde_json::from_value(self.config.clone()).unwrap(),
            self.dir.path(),
        )
        .await
        .unwrap()
    }
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)})),
    )
}
fn event(amount: i32) -> Value {
    json!({"event":{"amount":amount},"business_event_id":"same-event","idempotency_key":"same-key"})
}

#[tokio::test]
async fn scopes_authenticate_before_body_and_idempotency_is_separate() {
    let s = Setup::new();
    let app = s.app().await;
    let decide = path("local", "prod", "v1/core/decide");
    for (token, route, expected) in [
        (&s.tokens["acme"], decide.clone(), StatusCode::FORBIDDEN),
        (&s.tokens["admin"], decide.clone(), StatusCode::FORBIDDEN),
        (
            &s.tokens["local"],
            path("local", "dev", "v1/core/decide"),
            StatusCode::FORBIDDEN,
        ),
        (
            &s.tokens["expired"],
            decide.clone(),
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        assert_eq!(
            call(&app, "POST", &route, token, json!("invalid body"))
                .await
                .0,
            expected
        );
    }
    let request = Request::builder()
        .method("POST")
        .uri(&decide)
        .header("authorization", format!("Bearer {}", s.tokens["local"]))
        .header("authorization", format!("Bearer {}", s.tokens["local"]))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let mut ids = std::collections::BTreeSet::new();
    for (id, tenant, env) in [
        ("local", "local", "prod"),
        ("acme", "acme", "prod"),
        ("dev", "local", "dev"),
    ] {
        let route = path(tenant, env, "v1/core/decide");
        let (status, result) = call(&app, "POST", &route, &s.tokens[id], event(1001)).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["record"]["tenant_context"]["tenant_id"], tenant);
        assert_eq!(result["record"]["tenant_context"]["environment"], env);
        assert!(ids.insert(result["record"]["decision_id"].as_str().unwrap().to_owned()));
        assert_eq!(
            call(&app, "POST", &route, &s.tokens[id], event(1001))
                .await
                .1,
            result
        );
        assert_eq!(
            call(&app, "POST", &route, &s.tokens[id], event(999))
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    let mut spoof = event(1001);
    spoof["event"]["tenant_id"] = "acme".into();
    assert_eq!(
        call(&app, "POST", &decide, &s.tokens["local"], spoof)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/core/decide",
            &s.tokens["local"],
            event(1)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn delegation_rotation_and_private_exports_are_scoped() {
    let s = Setup::new();
    let app = s.app().await;
    let (status, result) = call(
        &app,
        "POST",
        &path("local", "prod", "v1/core/decide"),
        &s.tokens["agent"],
        event(1001),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["record"]["tenant_context"]["principal_id"], "local");
    assert_eq!(result["record"]["tenant_context"]["actor_id"], "agent");
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/repo/reload"),
            &s.tokens["agent"],
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, batch) = call(
        &app,
        "POST",
        &path("local", "prod", "v1/core/outbox/claim"),
        &s.tokens["consumer"],
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    assert_eq!(batch["events"].as_array().unwrap().len(), 1);
    assert!(batch["events"][0].get("input_evidence").is_none());
    assert!(batch["events"][0].get("response").is_none());
    let ack =
        json!({"lease":batch["lease"],"idempotency_keys":[batch["events"][0]["idempotency_key"]]});
    assert_eq!(
        call(
            &app,
            "POST",
            &path("acme", "prod", "v1/core/outbox/ack"),
            &s.tokens["acme"],
            ack.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/outbox/ack"),
            &s.tokens["consumer"],
            ack
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut next = event(900);
    next["idempotency_key"] = "next".into();
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/decide"),
            &s.tokens["local"],
            next
        )
        .await
        .0,
        StatusCode::OK
    );
    let batch = call(
        &app,
        "POST",
        &path("local", "prod", "v1/core/outbox/claim"),
        &s.tokens["local"],
        json!({}),
    )
    .await
    .1;
    assert!(batch["events"][0].get("input_evidence").is_some());
    assert!(batch["events"][0].get("response").is_some());
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/tenancy/credentials/reload",
            &s.tokens["local"],
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/tenancy/credentials",
            &s.tokens["admin"],
            json!({"action":"revoke","id":"agent"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/decide"),
            &s.tokens["agent"],
            event(1001)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    // The bootstrap file is no longer authoritative once the database is initialized.
    write(
        &s.dir.path().join("credentials.json"),
        &json!({"format_version":"1","principals":[]}),
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/tenancy/credentials/reload",
            &s.tokens["admin"],
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/v1/tenancy/deployments",
            &s.tokens["local"],
            json!(null)
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn pause_restart_reload_and_audit_do_not_change_another_tenant() {
    let s = Setup::new();
    let app = s.app().await;
    let a = path("local", "prod", "v1/core/target");
    let b = path("acme", "prod", "v1/core/target");
    let old_a = call(&app, "GET", &a, &s.tokens["local"], json!(null))
        .await
        .1;
    let old_b = call(&app, "GET", &b, &s.tokens["acme"], json!(null))
        .await
        .1;
    repository_fixture::publish(&s.dir.path().join("local_prod"), &bundle(true), "next");
    let (status, updated) = call(
        &app,
        "POST",
        &path("local", "prod", "v1/core/repo/reload"),
        &s.tokens["local"],
        json!({"expected_revision":old_a["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_ne!(updated["revision"], old_a["revision"]);
    assert_eq!(
        call(&app, "GET", &b, &s.tokens["acme"], json!(null))
            .await
            .1,
        old_b
    );
    let state_path = path("local", "prod", "runtime");
    assert_eq!(
        call(
            &app,
            "POST",
            &state_path,
            &s.tokens["local"],
            json!({"expected_revision":0,"paused":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &state_path,
            &s.tokens["local"],
            json!({"expected_revision":0,"paused":false})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/decide"),
            &s.tokens["local"],
            event(2001)
        )
        .await
        .1["error"],
        "E_TENANT_PAUSED"
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("acme", "prod", "v1/core/decide"),
            &s.tokens["acme"],
            event(2001)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "runtime/unload"),
            &s.tokens["local"],
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    let app2 = s.app().await;
    assert_eq!(
        call(&app2, "GET", &state_path, &s.tokens["local"], json!(null))
            .await
            .1["state"]["paused"],
        true
    );
    assert_eq!(
        call(
            &app2,
            "POST",
            &state_path,
            &s.tokens["local"],
            json!({"expected_revision":1,"paused":false})
        )
        .await
        .0,
        StatusCode::OK
    );
    let audit = call(
        &app2,
        "GET",
        &path("local", "prod", "runtime/audit"),
        &s.tokens["local"],
        json!(null),
    )
    .await
    .1;
    assert_eq!(audit["events"].as_array().unwrap().len(), 2);
    assert_eq!(audit["events"][0]["actor"]["actor_id"], "local");
    assert!(call(
        &app2,
        "GET",
        &path("acme", "prod", "runtime/audit"),
        &s.tokens["acme"],
        json!(null)
    )
    .await
    .1["events"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn configuration_rejects_shared_roots_cross_scope_approvals_and_delegation_escalation() {
    let mut s = Setup::new();
    let original = s.config.clone();
    s.config["deployments"][1]["root"] = s.config["deployments"][0]["root"].clone();
    assert!(tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path()
    )
    .await
    .is_err());
    s.config = original.clone();
    s.config["control_store"]["path"] = json!("local_prod/control.db");
    assert!(tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path()
    )
    .await
    .is_err());
    s.config = original;
    let file = s.dir.path().join("local_prod/core.json");
    let original: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let mut wrong = original.clone();
    wrong["approvals"][0]["tenant_scope"] = scope("acme", "prod");
    write(&file, &wrong);
    assert!(tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path()
    )
    .await
    .is_err());
    write(&file, &original);
    for p in s.credentials["principals"].as_array_mut().unwrap() {
        if p["id"] == "agent" {
            p["grants"][0]["scope"] = scope("acme", "prod");
        }
    }
    write(&s.dir.path().join("credentials.json"), &s.credentials);
    assert!(tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path()
    )
    .await
    .is_err());
}

#[test]
fn local_is_default_only_for_single_tenant_configuration() {
    let journal: corint_decision_server::journal::JournalConfig =
        serde_json::from_value(json!({"path":"journal.db","max_records":10,"max_bytes":1024}))
            .unwrap();
    assert_eq!(journal.tenant_id, "local");
    let journal: corint_decision_server::journal::JournalConfig = serde_json::from_value(
        json!({"path":"journal.db","tenant_id":"custom","max_records":10,"max_bytes":1024}),
    )
    .unwrap();
    assert_eq!(journal.tenant_id, "custom");
    assert!(
        serde_json::from_value::<Scope>(json!({"environment":"prod","deployment":"risk"})).is_err()
    );
}

#[tokio::test]
async fn shared_repository_selects_full_scope_with_identical_policy_ids() {
    let s = Setup::new();
    let db = s.dir.path().join("publications.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query(include_str!(
        "../../../docs/contracts/tenant-publication.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    for (tenant, env, changed) in [
        ("local", "prod", false),
        ("acme", "prod", true),
        ("local", "dev", true),
    ] {
        let root = s.dir.path().join(format!("{tenant}_{env}"));
        repository_fixture::publish(&root, &bundle(changed), "scoped-repository");
        let snapshot =
            corint_decision_toolchain::repository::load(&root.join("repository")).unwrap();
        let document =
            serde_json::to_string(&corint_decision_toolchain::repository::PublishedSources {
                manifest: std::fs::read_to_string(root.join("repository/published.json")).unwrap(),
                sources: snapshot.closure.originals().to_vec(),
            })
            .unwrap();
        sqlx::query("INSERT INTO corint_tenant_publication VALUES($1,$2,'risk',$3)")
            .bind(tenant)
            .bind(env)
            .bind(document)
            .execute(&pool)
            .await
            .unwrap();
        let file = root.join("core.json");
        let mut config: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        config["repository"] = json!("");
        config["repository_backend"] = json!({"type":"sqlite","path":db});
        write(&file, &config);
    }
    let app = s.app().await;
    for (actor, tenant, env, score) in [
        ("local", "local", "prod", 60),
        ("acme", "acme", "prod", 0),
        ("dev", "local", "dev", 0),
    ] {
        let (status, value) = call(
            &app,
            "POST",
            &path(tenant, env, "decide"),
            &s.tokens[actor],
            event(2100),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["decision"]["result"]["score"], score, "{value}");
    }
    pool.close().await;
}

#[tokio::test]
async fn an_open_single_tenant_journal_cannot_be_adopted_even_when_empty() {
    let s = Setup::new();
    let root = s.dir.path().join("local_prod");
    let core: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("core.json")).unwrap()).unwrap();
    let config = serde_json::from_value(core["journal"].clone()).unwrap();
    let journal = corint_decision_server::journal::Journal::open(&root, &config)
        .await
        .unwrap();
    let app = s.app().await;
    let (status, _) = call(
        &app,
        "POST",
        &path("local", "prod", "decide"),
        &s.tokens["local"],
        event(1001),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(journal.status().await.is_ok());
    let (status, _) = call(
        &app,
        "POST",
        &path("acme", "prod", "decide"),
        &s.tokens["acme"],
        event(1001),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn shared_sql_features_force_tenant_and_environment_and_reject_resource_drift() {
    use corint_decision_engine::{
        decision_host::FeatureHostConfig,
        feature_pipeline::{FeatureInput, FeaturePlan},
    };
    let mut s = Setup::new();
    let db = s.dir.path().join("shared-features.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE events(tenant_id TEXT, environment TEXT, user_id TEXT, amount REAL, occurred_at TEXT)").execute(&pool).await.unwrap();
    let now = chrono::Utc::now().timestamp();
    let timestamp = chrono::DateTime::from_timestamp(now - 2, 0)
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    for (tenant, env, amount) in [
        ("local", "prod", 1100),
        ("acme", "prod", 100),
        ("local", "dev", 500),
    ] {
        sqlx::query("INSERT INTO events VALUES(?,?,?,?,?)")
            .bind(tenant)
            .bind(env)
            .bind("same-user")
            .bind(amount)
            .bind(&timestamp)
            .execute(&pool)
            .await
            .unwrap();
    }
    let datasource = json!({"name":"events","type":"sql","provider":"sqlite","connection_string":db,"database":"test","pool_size":1,"timeout_ms":1000,"query_cache_ttl_secs":0});
    for (index, tenant, env, expected, score) in [
        (0, "local", "prod", 1100.0, 60),
        (1, "acme", "prod", 100.0, 0),
        (2, "local", "dev", 500.0, 0),
    ] {
        let root = s.dir.path().join(format!("{tenant}_{env}"));
        let mut core: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("core.json")).unwrap())
                .unwrap();
        let original = bundle(false);
        let mut schema: Value = serde_yaml::from_str(&original.input_schema.yaml).unwrap();
        schema["fields"]["user_id"] =
            json!({"name":"user_id","field_type":"string","required":false});
        let sources = SourceBundle::new(
            CoreSource {
                path: original.input_schema.path,
                yaml: schema.to_string(),
            },
            original.sources,
        )
        .unwrap();
        repository_fixture::publish(&root, &sources, "features");
        let mut context: Value =
            serde_yaml::from_str(&std::fs::read_to_string(root.join("context.yaml")).unwrap())
                .unwrap();
        context["input_schema"] = schema;
        context["fields"]["user_id"] = json!({"description":"Synthetic user","unit":"identifier","entity":"transaction","time_basis":"Request time"});
        let context = context.to_string();
        std::fs::write(root.join("context.yaml"), &context).unwrap();
        let mut target: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("target.json")).unwrap())
                .unwrap();
        target["context"]["sha256"] = hash(&context).into();
        let target = target.to_string();
        std::fs::write(root.join("target.json"), &target).unwrap();
        let plan=FeaturePlan { format_version:"1".into(),revision:"v1".into(),datasource_revisions:BTreeMap::from([("events".into(),"db-v1".into())]),timeout_ms:1000,
            outputs:vec![FeatureInput {available_at_field:None,freshness:None,field:"amount".into(),definition:serde_yaml::from_str("name: volume\ntype: aggregation\nmethod: sum\ndatasource: events\nentity: events\ndimension: user_id\ndimension_value: '${event.user_id}'\nfield: amount\nwindow: 1h\ntimestamp_field: occurred_at\n").unwrap()}] };
        let features: FeatureHostConfig=serde_json::from_value(json!({"plan":plan,"datasources":{"events":{"revision":"db-v1","config":datasource}},"activation_cases":[{"event":{"user_id":"same-user"},"as_of":now,"expected_values":{"amount":expected},"expected_score":score}]})).unwrap();
        write(&root.join("features.json"), &json!(features));
        core["feature_pipeline"] = "features.json".into();
        let resources = json!([{"datasource":"events","revision":"db-v1","config_sha256":canonical_sha256(&features.datasources["events"].config),"entity":"events","tenant_column":"tenant_id","environment_column":"environment"}]);
        s.config["deployments"][index]["resources"] = resources.clone();
        let approval = json!({"policy_sha256":repository_fixture::identity(&sources),"context_sha256":hash(&context),"target_sha256":hash(&target),"cases_sha256":hash(&fixture("cdl_core/behavior.yaml")),
            "feature_binding_sha256":features.binding_sha256(),"tenant_scope":scope(tenant,env),"resource_scope_sha256":canonical_sha256(&json!({"scope":scope(tenant,env),"resources":resources}))});
        core["approvals"] = json!([approval]);
        write(&root.join("core.json"), &core);
    }
    let app = s.app().await;
    for (id, tenant, env, expected, score) in [
        ("local", "local", "prod", 1100.0, 60),
        ("acme", "acme", "prod", 100.0, 0),
        ("dev", "local", "dev", 500.0, 0),
    ] {
        let (status,result)=call(&app,"POST",&path(tenant,env,"v1/core/decide"),&s.tokens[id],json!({"event":{"user_id":"same-user"},"business_event_id":"same-event","idempotency_key":"same-key"})).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["feature_evidence"]["values"]["amount"], expected);
        assert_eq!(result["decision"]["result"]["score"], score);
    }
    let before = call(
        &app,
        "GET",
        &path("local", "prod", "v1/core/target"),
        &s.tokens["local"],
        json!(null),
    )
    .await
    .1;
    let file = s.dir.path().join("local_prod/features.json");
    let mut changed: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    changed["datasources"]["events"]["config"]["connection_string"] =
        "another-tenant.sqlite".into();
    write(&file, &changed);
    let (status, _) = call(
        &app,
        "POST",
        &path("local", "prod", "v1/core/repo/reload"),
        &s.tokens["local"],
        json!({"expected_revision":before["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    pool.close().await;
}

#[tokio::test]
async fn exhausted_tenant_budget_does_not_block_other_tenants_or_pause() {
    let mut s = Setup::new();
    s.config["tenant_limits"]["local"]["burst"] = 1.into();
    s.config["tenant_limits"]["local"]["requests_per_second"] = 1.into();
    let app = s.app().await;
    // Warm the policy through the independent management admission pool.
    assert_eq!(
        call(
            &app,
            "GET",
            &path("local", "prod", "v1/core/target"),
            &s.tokens["local"],
            json!(null)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "v1/core/decide"),
            &s.tokens["local"],
            event(1001)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "dev", "v1/core/decide"),
            &s.tokens["dev"],
            event(1001)
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("local", "prod", "runtime"),
            &s.tokens["local"],
            json!({"expected_revision":0,"paused":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path("acme", "prod", "v1/core/decide"),
            &s.tokens["acme"],
            event(1001)
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run tests/scripts/run_tenant_postgres_tests.py"]
async fn postgres_tenant_instances_share_idempotency_and_control_without_cross_scope_access() {
    let mut s = Setup::new();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let control_schema = format!("tenant_control_{suffix}");
    s.config["control_store"] =
        json!({"type":"postgres","url_env":"CORINT_TEST_POSTGRES_URL","schema":control_schema});
    let mut schemas = vec![control_schema];
    for (i, deployment) in s.config["deployments"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let file = s
            .dir
            .path()
            .join(deployment["root"].as_str().unwrap())
            .join("core.json");
        let mut core: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let schema = format!("tenant_journal_{suffix}_{i}");
        schemas.push(schema.clone());
        core["journal"]["path"] = "".into();
        core["journal"]["backend"] =
            json!({"type":"postgres","url_env":"CORINT_TEST_POSTGRES_URL","schema":schema});
        write(&file, &core);
    }
    let (a, b) = tokio::join!(s.app(), s.app());
    let route = path("local", "prod", "v1/core/decide");
    let first = call(&a, "POST", &route, &s.tokens["local"], event(1001)).await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    let mut public_request = event(1001);
    public_request["tenant_id"] = "local".into();
    assert_eq!(
        call(&b, "POST", "/v1/decide", &s.tokens["local"], public_request).await,
        first
    );
    assert_eq!(
        call(&b, "POST", &route, &s.tokens["local"], event(1001)).await,
        first
    );
    let other = call(
        &b,
        "POST",
        &path("local", "dev", "v1/core/decide"),
        &s.tokens["dev"],
        event(999),
    )
    .await;
    assert_eq!(other.0, StatusCode::OK, "{}", other.1);
    assert_ne!(
        other.1["record"]["decision_id"],
        first.1["record"]["decision_id"]
    );
    assert_eq!(
        call(
            &a,
            "POST",
            &path("local", "prod", "runtime"),
            &s.tokens["local"],
            json!({"expected_revision":0,"paused":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut next = event(1001);
    next["idempotency_key"] = "new".into();
    assert_eq!(
        call(&b, "POST", &route, &s.tokens["local"], next).await.1["error"],
        "E_TENANT_PAUSED"
    );
    let file = s.dir.path().join("local_dev/core.json");
    let mut core: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let prod: Value = serde_json::from_str(
        &std::fs::read_to_string(s.dir.path().join("local_prod/core.json")).unwrap(),
    )
    .unwrap();
    core["journal"]["backend"] = prod["journal"]["backend"].clone();
    write(&file, &core);
    s.config["deployments"] = json!([s.config["deployments"][2]]);
    s.config["decision_bindings"] = json!({"local":scope("local","dev")});
    s.credentials["principals"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] == "admin" || p["id"] == "dev");
    write(&s.dir.path().join("credentials.json"), &s.credentials);
    let wrong = s.app().await;
    assert_eq!(
        call(
            &wrong,
            "POST",
            &path("local", "dev", "v1/core/decide"),
            &s.tokens["dev"],
            event(99)
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop((a, b, wrong));
    let pool = sqlx::PgPool::connect(&std::env::var("CORINT_TEST_POSTGRES_URL").unwrap())
        .await
        .unwrap();
    for schema in schemas {
        sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
}

#[tokio::test]
async fn timeout_keeps_admission_until_the_decision_finishes_durably() {
    use std::time::Duration;
    let mut s = Setup::new();
    s.config["deployments"][0]["timeout_ms"] = 1000.into();
    s.config["deployments"][0]["limits"]["max_inflight"] = 1.into();
    let app = s.app().await;
    let warm = call(
        &app,
        "GET",
        &path("local", "prod", "v1/core/target"),
        &s.tokens["local"],
        json!(null),
    )
    .await
    .0;
    assert!(matches!(warm, StatusCode::OK | StatusCode::GATEWAY_TIMEOUT));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if call(
                &app,
                "GET",
                &path("local", "prod", "runtime"),
                &s.tokens["local"],
                json!(null),
            )
            .await
            .1["loaded"]
                == true
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(s.dir.path().join("local_prod/journal.db")),
    )
    .await
    .unwrap();
    let lock = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let route = path("local", "prod", "v1/core/decide");
    assert_eq!(
        call(&app, "POST", &route, &s.tokens["local"], event(1001))
            .await
            .0,
        StatusCode::GATEWAY_TIMEOUT
    );
    assert_eq!(
        call(&app, "POST", &route, &s.tokens["local"], event(1001))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    lock.rollback().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let result = call(&app, "POST", &route, &s.tokens["local"], event(1001)).await;
            if result.0 == StatusCode::OK {
                break result.1;
            }
            assert_eq!(result.0, StatusCode::TOO_MANY_REQUESTS, "{}", result.1);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result["persistence"], "durable");
    assert_eq!(
        call(
            &app,
            "GET",
            &path("local", "prod", "v1/core/persistence"),
            &s.tokens["local"],
            json!(null)
        )
        .await
        .1["stored_records"],
        1
    );
    pool.close().await;
}

#[tokio::test]
async fn idle_eviction_releases_capacity_and_preserves_logical_revision() {
    let mut s = Setup::new();
    s.config["max_loaded"] = 1.into();
    let app = s.app().await;
    let a = path("local", "prod", "v1/core/target");
    let first = call(&app, "GET", &a, &s.tokens["local"], json!(null)).await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        call(
            &app,
            "GET",
            &path("acme", "prod", "v1/core/target"),
            &s.tokens["acme"],
            json!(null)
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let again = call(&app, "GET", &a, &s.tokens["local"], json!(null)).await;
    assert_eq!(again, first);
}

#[tokio::test]
async fn public_decide_uses_body_tenant_and_operator_binding_with_scoped_idempotency() {
    let s = Setup::new();
    let app = s.app().await;
    let mut request = event(1001);
    request["tenant_id"] = "local".into();
    for id in ["acme", "dev", "admin"] {
        assert_eq!(
            call(&app, "POST", "/v1/decide", &s.tokens[id], request.clone())
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let first = call(
        &app,
        "POST",
        "/v1/decide",
        &s.tokens["local"],
        request.clone(),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    assert_eq!(first.1["record"]["tenant_context"]["environment"], "prod");
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/decide",
            &s.tokens["local"],
            request.clone()
        )
        .await,
        first
    );
    request["tenant_id"] = "acme".into();
    let second = call(
        &app,
        "POST",
        "/v1/decide",
        &s.tokens["acme"],
        request.clone(),
    )
    .await;
    assert_eq!(second.0, StatusCode::OK, "{}", second.1);
    assert_eq!(second.1["record"]["tenant_id"], "acme");
    assert_ne!(
        second.1["record"]["decision_id"],
        first.1["record"]["decision_id"]
    );
    request["tenant_id"] = "missing-tenant".into();
    assert_eq!(
        call(&app, "POST", "/v1/decide", &s.tokens["local"], request)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn public_decide_authenticates_before_parsing_and_rejects_scope_overrides() {
    let s = Setup::new();
    let app = s.app().await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/decide",
            "invalid",
            json!("not an object")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for payload in [
        event(1),
        json!({"tenant_id":"","event":{}}),
        json!({"tenant_id":123,"event":{}}),
        json!({"tenant_id":"local","environment":"dev","event":{}}),
        json!({"tenant_id":"local","deployment":"other","event":{}}),
        json!({"tenant_id":"local","business_event_id":"spoof","event":{"amount":1,"tenant_id":"acme"}}),
    ] {
        assert_eq!(
            call(&app, "POST", "/v1/decide", &s.tokens["local"], payload)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("authorization", format!("Bearer {}", s.tokens["local"]))
                .body(Body::from(
                    r#"{"tenant_id":"local","tenant_id":"acme","event":{}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("authorization", format!("Bearer {}", s.tokens["local"]))
                .header("authorization", format!("Bearer {}", s.tokens["acme"]))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("authorization", format!("Bearer {}", s.tokens["local"]))
                .body(Body::from(vec![b' '; 8 * 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/decide?tenant_id=acme",
            &s.tokens["local"],
            json!({"tenant_id":"local","event":{}})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn public_decide_requires_unambiguous_operator_routing() {
    let mut s = Setup::new();
    s.config
        .as_object_mut()
        .unwrap()
        .remove("decision_bindings");
    let error = tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path(),
    )
    .await
    .err()
    .expect("ambiguous tenant");
    assert!(error.to_string().contains("multiple runtimes"));
    s.config["decision_bindings"] = json!({"local":scope("acme","prod")});
    assert!(tenancy::create_router(
        serde_json::from_value(s.config.clone()).unwrap(),
        s.dir.path()
    )
    .await
    .is_err());
    // A domain serving one runtime per tenant needs no extra routing configuration.
    s.config
        .as_object_mut()
        .unwrap()
        .remove("decision_bindings");
    s.config["deployments"].as_array_mut().unwrap().pop();
    s.credentials["principals"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] != "dev");
    write(&s.dir.path().join("credentials.json"), &s.credentials);
    let app = s.app().await;
    let mut request = event(1001);
    request["tenant_id"] = "local".into();
    assert_eq!(
        call(&app, "POST", "/v1/decide", &s.tokens["local"], request)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn public_decide_routes_event_type_through_registry() {
    let s = Setup::new();
    let root = s.dir.path().join("local_prod");
    let mut source = bundle(false);
    let mut schema: Value = serde_yaml::from_str(&source.input_schema.yaml).unwrap();
    schema["fields"]["type"] = json!({"name":"type","field_type":"string","required":true});
    source.input_schema.yaml = schema.to_string();
    source.sources.iter_mut().find(|s| s.path=="registry.yaml").unwrap().yaml = "version: \"0.1\"\nregistry:\n  - pipeline: payment\n    when: event.type == \"payment\"\n  - pipeline: login\n    when: event.type == \"login\"\n".into();
    let pipeline = source
        .sources
        .iter()
        .find(|s| s.path == "pipeline.yaml")
        .unwrap()
        .yaml
        .replace("id: payment", "id: login");
    source.sources.push(CoreSource {
        path: "login.yaml".into(),
        yaml: pipeline,
    });
    let source = SourceBundle::new(source.input_schema, source.sources).unwrap();
    repository_fixture::publish(&root, &source, "typed-events");
    let mut context: Value =
        serde_yaml::from_str(&fixture("contracts/business-context.yaml")).unwrap();
    context["input_schema"] = schema;
    context["fields"]["type"] = json!({"description":"Synthetic event type","unit":"category","entity":"transaction","time_basis":"Request time"});
    let context = context.to_string();
    std::fs::write(root.join("context.yaml"), &context).unwrap();
    let mut target: Value =
        serde_json::from_str(&fixture("contracts/target-capabilities.json")).unwrap();
    target["context"]["sha256"] = hash(&context).into();
    let target = target.to_string();
    std::fs::write(root.join("target.json"), &target).unwrap();
    let mut cases: Value = serde_yaml::from_str(&fixture("cdl_core/behavior.yaml")).unwrap();
    for case in cases["cases"].as_array_mut().unwrap() {
        case["input"]["event"]["type"] = "payment".into();
    }
    let cases = cases.to_string();
    std::fs::write(root.join("cases.yaml"), &cases).unwrap();
    let mut core: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("core.json")).unwrap()).unwrap();
    core["approvals"] = json!([{"policy_sha256":repository_fixture::identity(&source),"context_sha256":hash(&context),"target_sha256":hash(&target),"cases_sha256":hash(&cases),"tenant_scope":scope("local","prod"),"resource_scope_sha256":canonical_sha256(&json!({"scope":scope("local","prod"),"resources":[]}))}]);
    write(&root.join("core.json"), &core);
    let app = s.app().await;
    for kind in ["payment", "login"] {
        let request = json!({"tenant_id":"local","event":{"type":kind,"amount":1001},"business_event_id":kind});
        let (status, response) =
            call(&app, "POST", "/v1/decide", &s.tokens["local"], request).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["record"]["runtime"]["pipeline_id"], kind);
    }
}

async fn credential_lifecycle(s: &mut Setup) {
    let a = s.app().await;
    let b = s.app().await;
    let endpoint = "/v1/tenancy/credentials";
    let create = json!({"action":"create","id":"new_client","grants":[{"scope":scope("local","prod"),"permissions":["decide","inspect"]}]});
    assert_eq!(
        call(&a, "POST", endpoint, &s.tokens["local"], create.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let result = call(&a, "POST", endpoint, &s.tokens["admin"], create.clone()).await;
    assert_eq!(result.0, StatusCode::OK, "{}", result.1);
    let first_token = result.1["token"].as_str().unwrap().to_owned();
    assert_eq!(first_token.len(), 64);
    let mut request = event(1001);
    request["tenant_id"] = "local".into();
    assert_eq!(
        call(&a, "POST", "/v1/decide", &first_token, request.clone())
            .await
            .0,
        StatusCode::OK
    );
    request["tenant_id"] = "acme".into();
    assert_eq!(
        call(&a, "POST", "/v1/decide", &first_token, request)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&a, "POST", endpoint, &s.tokens["admin"], create)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(call(&a,"POST",endpoint,&s.tokens["admin"],json!({"action":"create","id":"bad_scope","grants":[{"scope":scope("missing","prod"),"permissions":["decide"]}]})).await.0,StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        call(
            &a,
            "POST",
            endpoint,
            &s.tokens["admin"],
            json!({"action":"revoke","id":"admin"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(call(&a,"POST",endpoint,&s.tokens["admin"],json!({"action":"create","id":"escalated_agent","delegated_by":"consumer","expires_at_ms":chrono::Utc::now().timestamp_millis()+60000,"grants":[{"scope":scope("local","prod"),"permissions":["publish"]}]})).await.0,StatusCode::UNPROCESSABLE_ENTITY);
    let rotated = call(
        &a,
        "POST",
        endpoint,
        &s.tokens["admin"],
        json!({"action":"rotate","id":"new_client"}),
    )
    .await;
    assert_eq!(rotated.0, StatusCode::OK);
    let token = rotated.1["token"].as_str().unwrap().to_owned();
    assert_ne!(first_token, token);
    let inventory = "/v1/tenancy/deployments";
    assert_eq!(
        call(&a, "GET", inventory, &first_token, json!(null))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // Remove all bootstrap inputs: subsequent starts must load only the database.
    std::fs::remove_file(s.dir.path().join("credentials.json")).unwrap();
    for env in &s.envs {
        std::env::remove_var(env);
    }
    s.config.as_object_mut().unwrap().remove("credentials");
    let restarted = s.app().await;
    assert_eq!(
        call(&restarted, "GET", inventory, &token, json!(null))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&restarted, "GET", inventory, &first_token, json!(null))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // Force another live node to observe rotation without waiting for its minute tick.
    assert_eq!(
        call(
            &b,
            "POST",
            "/v1/tenancy/credentials/reload",
            &s.tokens["admin"],
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&b, "GET", inventory, &token, json!(null)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &a,
            "POST",
            endpoint,
            &s.tokens["admin"],
            json!({"action":"revoke","id":"new_client"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&a, "GET", inventory, &token, json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &b,
            "POST",
            "/v1/tenancy/credentials/reload",
            &s.tokens["admin"],
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&b, "GET", inventory, &token, json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &a,
            "POST",
            endpoint,
            &s.tokens["admin"],
            json!({"action":"revoke","id":"local"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&a, "GET", inventory, &s.tokens["agent"], json!(null))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &a,
            "POST",
            endpoint,
            &s.tokens["admin"],
            json!({"action":"rotate","id":"local"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let restarted_again = s.app().await;
    assert_eq!(
        call(&restarted_again, "GET", inventory, &token, json!(null))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn credentials_persist_hashes_and_refresh_across_sqlite_hosts() {
    let mut s = Setup::new();
    credential_lifecycle(&mut s).await;
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        s.dir.path().join("control.db").display()
    ))
    .await
    .unwrap();
    let document: String = sqlx::query_scalar("SELECT document FROM tenant_credentials WHERE id=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!document.contains("token_env"));
    for token in s.tokens.values() {
        assert!(!document.contains(token));
    }
    let audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenant_credential_audit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit, 4);
}

#[tokio::test]
#[ignore = "requires CORINT_TEST_POSTGRES_URL"]
async fn postgres_credentials_persist_and_refresh_across_hosts() {
    let mut s = Setup::new();
    let schema = format!("credentials_{}", uuid::Uuid::new_v4().simple());
    s.config["control_store"] =
        json!({"type":"postgres","url_env":"CORINT_TEST_POSTGRES_URL","schema":schema});
    credential_lifecycle(&mut s).await;
    let pool = sqlx::PgPool::connect(&std::env::var("CORINT_TEST_POSTGRES_URL").unwrap())
        .await
        .unwrap();
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&pool)
        .await
        .unwrap();
}
