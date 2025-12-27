//! Tests for REST API components

#![cfg(test)]

use super::conversions::*;
use super::types::*;
use corint_core::Value;

#[test]
fn test_json_to_value_conversion() {
    let json = serde_json::json!({
        "string": "test",
        "number": 42,
        "bool": true,
        "null": null
    });

    let value = json_to_value(json);

    if let Value::Object(map) = value {
        assert!(matches!(map.get("string"), Some(Value::String(_))));
        assert!(matches!(map.get("number"), Some(Value::Number(_))));
        assert!(matches!(map.get("bool"), Some(Value::Bool(true))));
        assert!(matches!(map.get("null"), Some(Value::Null)));
    } else {
        panic!("Expected Object");
    }
}


#[test]
fn test_json_to_value_null() {
    let json = serde_json::Value::Null;
    let value = json_to_value(json);
    assert!(matches!(value, Value::Null));
}

#[test]
fn test_json_to_value_bool() {
    let json_true = serde_json::Value::Bool(true);
    let json_false = serde_json::Value::Bool(false);

    assert!(matches!(json_to_value(json_true), Value::Bool(true)));
    assert!(matches!(json_to_value(json_false), Value::Bool(false)));
}

#[test]
fn test_json_to_value_number_integer() {
    let json = serde_json::json!(42);
    let value = json_to_value(json);

    if let Value::Number(n) = value {
        assert_eq!(n, 42.0);
    } else {
        panic!("Expected Number");
    }
}

#[test]
fn test_json_to_value_number_float() {
    let json = serde_json::json!(3.5);
    let value = json_to_value(json);

    if let Value::Number(n) = value {
        assert!((n - 3.5).abs() < 0.001);
    } else {
        panic!("Expected Number");
    }
}

#[test]
fn test_json_to_value_string() {
    let json = serde_json::json!("hello");
    let value = json_to_value(json);

    assert_eq!(value, Value::String("hello".to_string()));
}

#[test]
fn test_json_to_value_array() {
    let json = serde_json::json!([1, 2, 3]);
    let value = json_to_value(json);

    if let Value::Array(arr) = value {
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], Value::Number(1.0));
        assert_eq!(arr[1], Value::Number(2.0));
        assert_eq!(arr[2], Value::Number(3.0));
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn test_json_to_value_nested_array() {
    let json = serde_json::json!([[1, 2], [3, 4]]);
    let value = json_to_value(json);

    if let Value::Array(outer) = value {
        assert_eq!(outer.len(), 2);
        if let Value::Array(inner) = &outer[0] {
            assert_eq!(inner.len(), 2);
            assert_eq!(inner[0], Value::Number(1.0));
        } else {
            panic!("Expected nested array");
        }
    } else {
        panic!("Expected Array");
    }
}

#[test]
fn test_json_to_value_object() {
    let json = serde_json::json!({"key": "value"});
    let value = json_to_value(json);

    if let Value::Object(map) = value {
        assert_eq!(map.get("key"), Some(&Value::String("value".to_string())));
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_json_to_value_nested_object() {
    let json = serde_json::json!({
        "user": {
            "name": "Alice",
            "age": 30
        }
    });
    let value = json_to_value(json);

    if let Value::Object(outer) = value {
        if let Some(Value::Object(inner)) = outer.get("user") {
            assert_eq!(inner.get("name"), Some(&Value::String("Alice".to_string())));
            assert_eq!(inner.get("age"), Some(&Value::Number(30.0)));
        } else {
            panic!("Expected nested object");
        }
    } else {
        panic!("Expected Object");
    }
}


#[test]
fn test_health_response_fields() {
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: "1.0.0".to_string(),
    };

    assert_eq!(response.status, "healthy");
    assert_eq!(response.version, "1.0.0");
}

#[test]
fn test_decide_response_payload_fields() {
    let response = DecideResponsePayload {
        request_id: "req_123".to_string(),
        status: 200,
        process_time_ms: 42,
        pipeline_id: "pipeline_001".to_string(),
        decision: DecisionPayload {
            result: "ALLOW".to_string(),
            actions: vec![],
            scores: ScoresPayload {
                canonical: 85,
                raw: 85,
                confidence: None,
            },
            evidence: EvidencePayload {
                triggered_rules: vec!["rule1".to_string(), "rule2".to_string()],
            },
            cognition: CognitionPayload {
                summary: "Low risk transaction".to_string(),
                reason_codes: vec!["LOW_RISK".to_string()],
            },
        },
        features: None,
        trace: None,
    };

    assert_eq!(response.request_id, "req_123");
    assert_eq!(response.status, 200);
    assert_eq!(response.pipeline_id, "pipeline_001");
    assert_eq!(response.decision.result, "ALLOW");
    assert_eq!(response.decision.scores.canonical, 85);
    assert_eq!(response.decision.evidence.triggered_rules.len(), 2);
    assert_eq!(response.decision.cognition.summary, "Low risk transaction");
    assert_eq!(response.process_time_ms, 42);
    assert!(response.trace.is_none());
}

#[test]
fn test_decide_request_payload_empty() {
    use std::collections::HashMap;

    let payload = DecideRequestPayload {
        event: HashMap::new(),
        user: None,
        options: None,
        features: None,
        api: None,
        service: None,
        llm: None,
        vars: None,
    };

    assert_eq!(payload.event.len(), 0);
    assert!(payload.options.is_none());
}

#[test]
fn test_request_options_defaults() {
    let options = RequestOptions::default();
    assert!(!options.return_features);
    assert!(!options.enable_trace);
    assert!(!options.async_mode);
}

#[test]
fn test_normalize_score() {
    // Negative scores become 0
    assert_eq!(normalize_score(-100), 0);
    assert_eq!(normalize_score(-1), 0);

    // Sigmoid normalization provides smooth S-curve
    // Center point (500) should map to ~500
    let center = normalize_score(500);
    assert!(center >= 495 && center <= 505, "Center: {}", center);

    // Low scores should be compressed
    let low = normalize_score(100);
    assert!(low > 0 && low < 200, "Low: {}", low);

    // High scores should be compressed
    let high = normalize_score(1000);
    assert!(high > 700 && high < 1000, "High: {}", high);

    // Very high scores should saturate near 1000
    let very_high = normalize_score(5000);
    assert!(very_high >= 900 && very_high <= 1000, "Very high: {}", very_high);

    // Scores should increase monotonically
    assert!(normalize_score(300) < normalize_score(500));
    assert!(normalize_score(500) < normalize_score(700));
}

#[test]
fn test_extract_reason_codes() {
    let codes = extract_reason_codes("Low risk transaction");
    assert!(codes.contains(&"LOW_RISK".to_string()));

    let codes2 = extract_reason_codes("Email not verified, high transaction amount");
    assert!(codes2.contains(&"EMAIL_NOT_VERIFIED".to_string()));
    assert!(codes2.contains(&"HIGH_TRANSACTION_AMOUNT".to_string()));
}

#[test]
fn test_json_to_value_mixed_types() {
    let json = serde_json::json!({
        "str": "text",
        "num": 123,
        "bool": true,
        "null": null,
        "arr": [1, 2, 3],
        "obj": {"nested": "value"}
    });

    let value = json_to_value(json);

    if let Value::Object(map) = value {
        assert!(matches!(map.get("str"), Some(Value::String(_))));
        assert!(matches!(map.get("num"), Some(Value::Number(_))));
        assert!(matches!(map.get("bool"), Some(Value::Bool(true))));
        assert!(matches!(map.get("null"), Some(Value::Null)));
        assert!(matches!(map.get("arr"), Some(Value::Array(_))));
        assert!(matches!(map.get("obj"), Some(Value::Object(_))));
    } else {
        panic!("Expected Object");
    }
}
