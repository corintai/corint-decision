//! Type conversion utilities
//!
//! Functions to convert between corint_core::Value and serde_json::Value,
//! and other conversion utilities.

use corint_core::Value;
use corint_sdk::ScoreNormalizer;

/// Normalize raw score to canonical 0-1000 range using sigmoid/logistic function
///
/// Uses the SDK's ScoreNormalizer with default parameters for industry-standard
/// score normalization that provides smooth S-curve mapping.
pub(super) fn normalize_score(raw: i32) -> i32 {
    // Use sigmoid normalization for smooth, production-grade score mapping
    ScoreNormalizer::default().normalize(raw)
}

/// Extract reason codes from explanation string
pub(super) fn extract_reason_codes(explanation: &str) -> Vec<String> {
    // Simple extraction: look for common patterns
    let mut codes = Vec::new();

    // Extract codes from explanation (this is a basic implementation)
    // In a real system, these would come from the decision result
    if explanation.to_lowercase().contains("email") && explanation.to_lowercase().contains("not verified") {
        codes.push("EMAIL_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("phone") && explanation.to_lowercase().contains("not verified") {
        codes.push("PHONE_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("new account") || explanation.to_lowercase().contains("account_age") {
        codes.push("NEW_ACCOUNT".to_string());
    }
    if explanation.to_lowercase().contains("high") && explanation.to_lowercase().contains("amount") {
        codes.push("HIGH_TRANSACTION_AMOUNT".to_string());
    }
    if explanation.to_lowercase().contains("low risk") {
        codes.push("LOW_RISK".to_string());
    }

    codes
}

/// Convert corint_core::Value to serde_json::Value
pub(super) fn value_to_json(v: Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(b),
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::Value::String(s),
        Value::Array(arr) => serde_json::Value::Array(arr.into_iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .into_iter()
                .map(|(k, v)| (k, value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Convert serde_json::Value to corint_core::Value
pub(super) fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i as f64)
            } else if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let map = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}
