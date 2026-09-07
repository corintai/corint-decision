use async_trait::async_trait;
use corint_decision_compiler::codegen::PipelineCompiler;
use corint_decision_dsl_parser::PipelineParser;
use corint_decision_model::{ir::Program, Value};
use corint_decision_runtime::{
    HttpServiceClient, HttpServiceConfig, PipelineExecutor, RuntimeError, ServiceClient,
    ServiceRequest, ServiceResponse,
};
use std::{collections::HashMap, sync::Arc};

async fn read_http_headers(socket: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let count = socket.read(&mut buffer).await.unwrap();
        assert!(count > 0, "connection closed before request headers");
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
        assert!(request.len() < 65536, "request headers too large");
    }
    String::from_utf8(request).unwrap()
}

fn compile(fields: &str) -> Program {
    PipelineCompiler::compile(&PipelineParser::parse(&format!("pipeline:\n  id: p\n  name: P\n  entry: lookup\n  steps:\n    - step:\n        id: lookup\n        name: Lookup\n        type: service\n        service: risk\n        operation: score\n{fields}")).unwrap()).unwrap()
}

fn fixture(id: &str) -> &'static str {
    match id {
        "service-http" => include_str!("../../../tests/conformance/service/customer-risk.yaml"),
        "service-parameters" => include_str!("../../../tests/conformance/service/pipeline.yaml"),
        _ => panic!("unknown test fixture: {id}"),
    }
}
struct Echo;
#[async_trait]
impl ServiceClient for Echo {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse, RuntimeError> {
        assert_eq!(request.operation, "score");
        Ok(ServiceResponse::new(Value::Object(request.params)))
    }
    fn name(&self) -> &str {
        "echo"
    }
}
struct Failure;
#[async_trait]
impl ServiceClient for Failure {
    async fn call(&self, _: ServiceRequest) -> Result<ServiceResponse, RuntimeError> {
        Err(RuntimeError::ServiceCallFailed("unavailable".into()))
    }
    fn name(&self) -> &str {
        "failure"
    }
}
struct Slow;
#[async_trait]
impl ServiceClient for Slow {
    async fn call(&self, _: ServiceRequest) -> Result<ServiceResponse, RuntimeError> {
        std::future::pending().await
    }
    fn name(&self) -> &str {
        "slow"
    }
}

#[tokio::test]
async fn service_parameters_and_step_results_are_preserved() {
    let yaml = "pipeline:\n  id: p\n  name: P\n  entry: first\n  steps:\n    - step:\n        id: first\n        name: First\n        type: service\n        service: risk\n        operation: score\n        params:\n          value: '${event.amount + 1}'\n          url: https://example.com\n          tags: [one, two]\n        next: second\n    - step:\n        id: second\n        name: Second\n        type: service\n        service: risk\n        operation: score\n        params:\n          value: '${service.first.value + 1}'\n";
    let program = PipelineCompiler::compile(&PipelineParser::parse(yaml).unwrap()).unwrap();
    let executor = PipelineExecutor::new_offline()
        .with_service("risk", Arc::new(Echo))
        .unwrap();
    let result = executor
        .execute(
            &program,
            HashMap::from([("amount".into(), Value::Number(40.0))]),
        )
        .await
        .unwrap();
    let Value::Object(service) = &result.context["service"] else {
        panic!()
    };
    let Value::Object(first) = &service["first"] else {
        panic!()
    };
    let Value::Object(second) = &service["second"] else {
        panic!()
    };
    assert_eq!(first["value"], Value::Number(41.0));
    assert_eq!(second["value"], Value::Number(42.0));
    assert_eq!(first["url"], Value::String("https://example.com".into()));
    assert_eq!(
        first["tags"],
        Value::Array(vec![
            Value::String("one".into()),
            Value::String("two".into())
        ])
    );
    assert!(!result.context.contains_key("api"));
}

#[tokio::test(start_paused = true)]
async fn service_errors_missing_bindings_and_deadlines_fail_execution() {
    let program = compile("        timeout_ms: 5\n");
    assert!(PipelineExecutor::new_offline()
        .execute(&program, HashMap::new())
        .await
        .is_err());
    let failure = PipelineExecutor::new_offline()
        .with_service("risk", Arc::new(Failure))
        .unwrap();
    assert!(failure
        .execute(&program, HashMap::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("unavailable"));
    let slow = PipelineExecutor::new_offline()
        .with_service("risk", Arc::new(Slow))
        .unwrap();
    assert!(slow
        .execute(&program, HashMap::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("timed out"));
}

#[test]
fn removed_api_syntax_and_ambiguous_service_options_are_rejected() {
    for fields in [
        "type: api\n        api: risk",
        "type: service\n        service: risk\n        endpoint: score",
        "type: service\n        service: risk\n        operation: score\n        timeout: 10",
        "type: service\n        service: risk\n        operation: score\n        on_error: ignore",
    ] {
        let yaml = format!("pipeline:\n  id: p\n  name: P\n  entry: s\n  steps:\n    - step:\n        id: s\n        name: S\n        {fields}\n");
        assert!(PipelineParser::parse(&yaml).is_err(), "{fields}");
    }
    for extra in [
        "timeout_ms: 0",
        "timeout_ms: -1",
        "params: []",
        "params: {1: value}",
    ] {
        let yaml = format!("pipeline:\n  id: p\n  name: P\n  entry: s\n  steps:\n    - step:\n        id: s\n        name: S\n        type: service\n        service: risk\n        operation: score\n        {extra}\n");
        assert!(PipelineParser::parse(&yaml).is_err(), "{extra}");
    }
}

#[tokio::test]
async fn http_connector_executes_the_same_service_instruction() {
    use tokio::{io::AsyncWriteExt, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(read_http_headers(&mut socket)
            .await
            .starts_with("GET /customers/customer%2F42?amount=41&channel=web "));
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"risk_score\":42}").await.unwrap();
    });
    let mut binding: HttpServiceConfig = serde_yaml::from_str(fixture("service-http")).unwrap();
    // Keep the published definition intact except for the local test transport.
    binding.base_url = format!("http://{address}");
    let program =
        PipelineCompiler::compile(&PipelineParser::parse(fixture("service-parameters")).unwrap())
            .unwrap();
    let mut client = HttpServiceClient::new();
    client.register_service(binding.clone()).unwrap();
    assert!(client.register_service(binding).is_err());
    let result = PipelineExecutor::new()
        .with_http_service_client(Arc::new(client))
        .execute(
            &program,
            HashMap::from([
                ("customer_id".into(), Value::String("customer/42".into())),
                ("amount".into(), Value::Number(40.0)),
            ]),
        )
        .await
        .unwrap();
    server.await.unwrap();
    let Value::Object(service) = &result.context["service"] else {
        panic!()
    };
    let Value::Object(response) = &service["lookup"] else {
        panic!()
    };
    assert_eq!(response["score"], Value::Number(42.0));
    assert!(!response.contains_key("risk_score"));
}

#[test]
fn service_documentation_and_binding_schema_agree() {
    PipelineCompiler::compile(&PipelineParser::parse(fixture("service-parameters")).unwrap())
        .unwrap();
    let binding: HttpServiceConfig = serde_yaml::from_str(fixture("service-http")).unwrap();
    HttpServiceClient::new().register_service(binding).unwrap();
    let old = "name: risk\nbase_url: http://localhost\nendpoints: {}\n";
    assert!(serde_yaml::from_str::<HttpServiceConfig>(old).is_err());
    assert!(
        serde_json::from_value::<corint_decision_engine::DecisionRequest>(
            serde_json::json!({"event_data": {}, "api": null})
        )
        .is_err()
    );
}

#[tokio::test]
async fn sdk_builder_registers_named_services() {
    let policy = "pipeline:\n  id: p\n  name: P\n  entry: lookup\n  steps:\n    - step:\n        id: lookup\n        name: Lookup\n        type: service\n        service: risk\n        operation: score\n        params:\n          value: event.amount\n  decision:\n    - default: true\n      result: approve\n";
    let engine = corint_decision_engine::DecisionEngineBuilder::new()
        .with_service("risk", Arc::new(Echo))
        .unwrap()
        .add_rule_content("p", policy)
        .build()
        .await
        .unwrap();
    let result = engine
        .decide(corint_decision_engine::DecisionRequest::new(HashMap::from(
            [("amount".into(), Value::Number(42.0))],
        )))
        .await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn http_errors_are_not_implicit_null_results() {
    use tokio::{io::AsyncWriteExt, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert!(read_http_headers(&mut socket)
            .await
            .starts_with("GET /score "));
        socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.unwrap();
    });
    let config: HttpServiceConfig = serde_yaml::from_str(&format!("name: risk\nbase_url: http://{address}\noperations:\n  score:\n    method: GET\n    path: /score\n")).unwrap();
    let mut client = HttpServiceClient::new();
    client.register_service(config).unwrap();
    let result = PipelineExecutor::new()
        .with_http_service_client(Arc::new(client))
        .execute(&compile(""), HashMap::new())
        .await;
    server.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("503"));
}

#[tokio::test]
async fn http_fallback_distinguishes_json_errors_from_response_transport_errors() {
    use tokio::{io::AsyncWriteExt, net::TcpListener};
    for (label, status, body, incomplete, hold_open) in [
        ("mapped", "200 OK", r#"{"risk_score":42}"#, false, false),
        ("status", "503 Unavailable", "{}", false, false),
        ("json", "200 OK", "invalid JSON", false, false),
        ("truncated", "200 OK", "{", true, false),
        ("timeout", "200 OK", "{", true, true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert!(read_http_headers(&mut socket)
                .await
                .starts_with("GET /score "));
            let length = body.len() + if incomplete { 100 } else { 0 };
            let response = format!("HTTP/1.1 {status}\r\nContent-Length: {length}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}");
            socket.write_all(response.as_bytes()).await.unwrap();
            if hold_open {
                std::future::pending::<()>().await;
            }
        });
        let config: HttpServiceConfig = serde_json::from_value(serde_json::json!({
            "name":"risk", "base_url":format!("http://{address}"),
            "timeout_ms": if hold_open { 200 } else { 2000 },
            "operations":{"score":{"method":"GET","path":"/score",
                "response":{"mapping":{"score":"risk_score"},"fallback":{"available":false}}}}
        }))
        .unwrap();
        let mut client = HttpServiceClient::new();
        client.register_service(config).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            PipelineExecutor::new_offline()
                .with_http_service_client(Arc::new(client))
                .execute(&compile(""), HashMap::new()),
        )
        .await
        .expect("bounded HTTP test");
        if hold_open {
            server.abort();
            assert!(server.await.unwrap_err().is_cancelled());
        } else {
            server.await.unwrap();
        }
        if incomplete {
            assert!(
                result.is_err(),
                "{label} must not become a fallback success"
            );
        } else {
            let result = result.unwrap();
            let Value::Object(services) = &result.context["service"] else {
                panic!("service outputs")
            };
            let expected = if label == "mapped" {
                Value::Object(HashMap::from([("score".into(), Value::Number(42.0))]))
            } else {
                Value::Object(HashMap::from([("available".into(), Value::Bool(false))]))
            };
            assert_eq!(services["lookup"], expected, "{label}");
        }
    }
}
