//! Environment Variables Loader
//!
//! Loads and parses environment variables for runtime configuration including:
//! - CORINT_* prefixed configuration variables
//! - FEATURE_* prefixed feature flags
//! - Default configuration values

use corint_core::Value;
use std::collections::HashMap;

/// Load environment variables (env namespace)
///
/// Loads configuration from environment variables:
/// 1. CORINT_* variables (e.g., CORINT_MAX_SCORE -> max_score)
/// 2. FEATURE_* variables (e.g., FEATURE_ENABLE_LLM -> feature_flags.enable_llm)
/// 3. Feature flags
pub(super) fn load_environment_vars() -> HashMap<String, Value> {
    let mut env = HashMap::new();

    // Load CORINT_* environment variables
    for (key, value) in std::env::vars() {
        if key.starts_with("CORINT_") {
            // Remove CORINT_ prefix and convert to lowercase
            let config_key = key
                .strip_prefix("CORINT_")
                .unwrap()
                .to_lowercase();

            // Try to parse as different types
            env.insert(
                config_key,
                parse_env_value(&value),
            );
        }
    }

    // Add common configuration with defaults
    if !env.contains_key("max_score") {
        env.insert("max_score".to_string(), Value::Number(100.0));
    }

    if !env.contains_key("default_action") {
        env.insert("default_action".to_string(), Value::String("approve".to_string()));
    }

    // Feature flags namespace
    let mut feature_flags = HashMap::new();

    // Check for feature flag environment variables
    for (key, value) in std::env::vars() {
        if key.starts_with("FEATURE_") {
            let flag_name = key
                .strip_prefix("FEATURE_")
                .unwrap()
                .to_lowercase();

            feature_flags.insert(
                flag_name,
                parse_bool_value(&value),
            );
        }
    }

    // Add default feature flags if not set
    if !feature_flags.contains_key("enable_llm") {
        feature_flags.insert("enable_llm".to_string(), Value::Bool(false));
    }
    if !feature_flags.contains_key("enable_cache") {
        feature_flags.insert("enable_cache".to_string(), Value::Bool(true));
    }

    env.insert("feature_flags".to_string(), Value::Object(feature_flags));

    env
}

/// Parse environment variable value to appropriate type
fn parse_env_value(value: &str) -> Value {
    // Try to parse as number
    if let Ok(num) = value.parse::<f64>() {
        return Value::Number(num);
    }

    // Try to parse as boolean
    match value.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => return Value::Bool(true),
        "false" | "no" | "0" | "off" => return Value::Bool(false),
        _ => {}
    }

    // Try to parse as JSON (for objects/arrays)
    if let Ok(json_value) = serde_json::from_str(value) {
        return json_value;
    }

    // Default to string
    Value::String(value.to_string())
}

/// Parse boolean value from string
fn parse_bool_value(value: &str) -> Value {
    match value.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Value::Bool(true),
        "false" | "no" | "0" | "off" => Value::Bool(false),
        _ => Value::Bool(false),
    }
}
