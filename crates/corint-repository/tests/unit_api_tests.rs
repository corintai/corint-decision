//! Unit tests for ApiRepository implementation
//!
//! These tests use mockito to mock HTTP API responses.

#![cfg(feature = "api")]

use corint_repository::{ApiRepository, Repository};
use mockito::{Mock, Server};

/// Helper to create a mock manifest response
fn create_mock_manifest(server: &mut Server) -> Mock {
    let base_url = server.url();
    let manifest_json = format!(
        r#"{{
        "registry": "{base_url}/registry.yaml",
        "pipelines": [
            {{
                "id": "test_pipeline",
                "url": "{base_url}/pipelines/test_pipeline.yaml",
                "description": "Test pipeline"
            }}
        ],
        "rulesets": [
            {{
                "id": "test_ruleset",
                "url": "{base_url}/rulesets/test_ruleset.yaml",
                "description": "Test ruleset"
            }}
        ],
        "rules": [
            {{
                "id": "test_rule",
                "url": "{base_url}/rules/test_rule.yaml",
                "description": "Test rule"
            }}
        ],
        "templates": [
            {{
                "id": "test_template",
                "url": "{base_url}/templates/test_template.yaml",
                "description": "Test template"
            }}
        ]
    }}"#
    );

    server
        .mock("GET", "/manifest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(manifest_json)
        .create()
}

#[tokio::test]
async fn test_api_repository_creation() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create API repository");

    // Just verify it was created successfully
    drop(repo);
}

#[tokio::test]
async fn test_api_repository_with_api_key() {
    let mut server = Server::new_async().await;

    let _m = server
        .mock("GET", "/manifest")
        .match_header("Authorization", "Bearer test_key_123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"pipelines": [], "rulesets": [], "rules": [], "templates": []}"#)
        .create();

    let repo = ApiRepository::new(&server.url(), Some("test_key_123"))
        .await
        .expect("Failed to create API repository with API key");

    drop(repo);
}

#[tokio::test]
async fn test_api_repository_manifest_fetch_failure() {
    let mut server = Server::new_async().await;

    let _m = server.mock("GET", "/manifest").with_status(404).create();

    let result = ApiRepository::new(&server.url(), None::<String>).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_load_rule_success() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let rule_yaml = r#"id: test_rule
name: Test Rule
when:
  conditions:
    - !Binary
      left: !FieldAccess
        - amount
      op: Gt
      right: !Literal
        Number: 100.0
score: 50
"#;

    let _rule_mock = server
        .mock("GET", "/rules/test_rule.yaml")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(rule_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let (rule, content) = repo
        .load_rule("test_rule")
        .await
        .expect("Failed to load rule");

    assert_eq!(rule.id, "test_rule");
    assert_eq!(rule.name, "Test Rule");
    assert!(content.contains("test_rule"));
}

#[tokio::test]
async fn test_load_rule_not_found_in_manifest() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let result = repo.load_rule("nonexistent_rule").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_load_rule_http_error() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let _rule_mock = server
        .mock("GET", "/rules/test_rule.yaml")
        .with_status(500)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let result = repo.load_rule("test_rule").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_load_ruleset_success() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let ruleset_yaml = r#"id: test_ruleset
name: Test Ruleset
rules:
  - test_rule
decision_logic:
  - condition: !Binary
      left: !FieldAccess
        - total_score
      op: Ge
      right: !Literal
        Number: 100.0
    action:
      type: deny
  - default: true
    action:
      type: approve
"#;

    let _ruleset_mock = server
        .mock("GET", "/rulesets/test_ruleset.yaml")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(ruleset_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let (ruleset, content) = repo
        .load_ruleset("test_ruleset")
        .await
        .expect("Failed to load ruleset");

    assert_eq!(ruleset.id, "test_ruleset");
    assert!(content.contains("test_ruleset"));
}

#[tokio::test]
async fn test_load_template_success() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let template_yaml = r#"id: test_template
name: Test Template
params:
  threshold: integer
decision_logic:
  - condition: !Binary
      left: !FieldAccess
        - total_score
      op: Ge
      right: !FieldAccess
        - params
        - threshold
    action:
      type: deny
  - default: true
    action:
      type: approve
"#;

    let _template_mock = server
        .mock("GET", "/templates/test_template.yaml")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(template_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let (template, content) = repo
        .load_template("test_template")
        .await
        .expect("Failed to load template");

    assert_eq!(template.id, "test_template");
    assert!(content.contains("test_template"));
}

#[tokio::test]
async fn test_load_pipeline_success() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let pipeline_yaml = r#"id: test_pipeline
name: Test Pipeline
steps:
  - type: include
    ruleset: test_ruleset
"#;

    let _pipeline_mock = server
        .mock("GET", "/pipelines/test_pipeline.yaml")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(pipeline_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let (pipeline, content) = repo
        .load_pipeline("test_pipeline")
        .await
        .expect("Failed to load pipeline");

    assert_eq!(pipeline.id, Some("test_pipeline".to_string()));
    assert!(content.contains("test_pipeline"));
}

#[tokio::test]
async fn test_exists_true() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let exists = repo
        .exists("test_rule")
        .await
        .expect("Failed to check existence");

    assert!(exists);
}

#[tokio::test]
async fn test_exists_false() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let exists = repo
        .exists("nonexistent")
        .await
        .expect("Failed to check existence");

    assert!(!exists);
}

#[tokio::test]
async fn test_list_rules() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let rules = repo.list_rules().await.expect("Failed to list rules");

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0], "test_rule");
}

#[tokio::test]
async fn test_list_rulesets() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let rulesets = repo.list_rulesets().await.expect("Failed to list rulesets");

    assert_eq!(rulesets.len(), 1);
    assert_eq!(rulesets[0], "test_ruleset");
}

#[tokio::test]
async fn test_list_templates() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let templates = repo
        .list_templates()
        .await
        .expect("Failed to list templates");

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0], "test_template");
}

#[tokio::test]
async fn test_list_pipelines() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let pipelines = repo
        .list_pipelines()
        .await
        .expect("Failed to list pipelines");

    assert_eq!(pipelines.len(), 1);
    assert_eq!(pipelines[0], "test_pipeline");
}

#[tokio::test]
async fn test_load_registry() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let registry_yaml = r#"version: "0.1"

registry:
  pipelines:
    - event_type: transaction
      pipeline: test_pipeline
"#;

    let _registry_mock = server
        .mock("GET", "/registry")
        .with_status(200)
        .with_header("content-type", "text/yaml")
        .with_body(registry_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let content = repo.load_registry().await.expect("Failed to load registry");

    assert!(content.contains("registry"));
    assert!(content.contains("test_pipeline"));
}

#[tokio::test]
async fn test_load_registry_not_found() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let _registry_mock = server.mock("GET", "/registry").with_status(404).create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let result = repo.load_registry().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_api_with_authentication_header() {
    let mut server = Server::new_async().await;
    let base_url = server.url();

    let _manifest_mock = server
        .mock("GET", "/manifest")
        .match_header("Authorization", "Bearer secret_key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(format!(
            r#"{{
            "rules": [{{
                "id": "secure_rule",
                "url": "{base_url}/rules/secure_rule.yaml"
            }}],
            "rulesets": [],
            "templates": [],
            "pipelines": []
        }}"#
        ))
        .create();

    let rule_yaml = r#"id: secure_rule
name: Secure Rule
when:
  conditions:
    - !Literal
      Bool: true
score: 10
"#;

    let _rule_mock = server
        .mock("GET", "/rules/secure_rule.yaml")
        .match_header("Authorization", "Bearer secret_key")
        .with_status(200)
        .with_body(rule_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), Some("secret_key"))
        .await
        .expect("Failed to create repository");

    let (rule, _) = repo
        .load_rule("secure_rule")
        .await
        .expect("Failed to load secure rule");

    assert_eq!(rule.id, "secure_rule");
}

#[tokio::test]
async fn test_parse_error_handling() {
    let mut server = Server::new_async().await;
    let _m = create_mock_manifest(&mut server);

    let invalid_yaml = "invalid: yaml: syntax: :::";

    let _rule_mock = server
        .mock("GET", "/rules/test_rule.yaml")
        .with_status(200)
        .with_body(invalid_yaml)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let result = repo.load_rule("test_rule").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_manifest() {
    let mut server = Server::new_async().await;

    let _m = server
        .mock("GET", "/manifest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"rules": [], "rulesets": [], "templates": [], "pipelines": []}"#)
        .create();

    let repo = ApiRepository::new(&server.url(), None::<String>)
        .await
        .expect("Failed to create repository");

    let rules = repo.list_rules().await.expect("Failed to list rules");
    assert_eq!(rules.len(), 0);

    let exists = repo
        .exists("any_rule")
        .await
        .expect("Failed to check exists");
    assert!(!exists);
}
