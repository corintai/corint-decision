//! Common test utilities for SDK integration tests

use corint_core::Value;
use corint_sdk::{DecisionEngineBuilder, DecisionRequest, DecisionResponse};
use std::collections::HashMap;

/// Test helper to create a DecisionEngine from inline YAML definitions
pub struct TestEngine {
    contents: Vec<String>,
}

impl TestEngine {
    /// Create a new test engine with multiple definitions
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
        }
    }

    /// Load a rule from YAML string
    pub fn with_rule(mut self, rule_yaml: &str) -> Self {
        self.contents.push(rule_yaml.trim().to_string());
        self
    }

    /// Load a ruleset from YAML string
    pub fn with_ruleset(mut self, ruleset_yaml: &str) -> Self {
        self.contents.push(ruleset_yaml.trim().to_string());
        self
    }

    /// Load a pipeline from YAML string
    pub fn with_pipeline(mut self, pipeline_yaml: &str) -> Self {
        self.contents.push(pipeline_yaml.trim().to_string());
        self
    }

    /// Build a combined YAML content
    fn build_combined_yaml(&self) -> String {
        // Use empty lines around --- separator to match SDK internal test format
        self.contents.join("\n\n---\n\n")
    }

    /// Execute a ruleset with event data (creates a wrapper pipeline)
    pub async fn execute_ruleset(
        &self,
        ruleset_id: &str,
        event: HashMap<String, Value>,
    ) -> DecisionResponse {
        // Create a wrapper pipeline that includes the ruleset
        // Using shorthand format that matches SDK internal tests
        let wrapper_pipeline = format!(
            r#"pipeline:
  id: test_wrapper_pipeline
  name: Test Wrapper Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: {}"#,
            ruleset_id
        );

        // The combined content must have the pipeline at the start
        // Use empty lines around --- separator to match SDK internal test format
        let combined = if self.contents.is_empty() {
            wrapper_pipeline
        } else {
            format!("{}\n\n---\n\n{}", wrapper_pipeline, self.build_combined_yaml())
        };

        // Write to temp file - the SDK loads from files correctly
        // Use thread ID and counter for unique filename to avoid conflicts in parallel tests
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_path = format!(
            "/tmp/test_ruleset_{}_{:?}_{}.yaml",
            std::process::id(),
            std::thread::current().id(),
            unique_id
        );
        std::fs::write(&temp_path, &combined).expect("Failed to write temp file");

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(&temp_path)
            .build()
            .await
            .expect("Failed to build engine");

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        // Add event.type: test to trigger the wrapper pipeline
        let mut event_with_type = event;
        event_with_type.insert("type".to_string(), Value::String("test".to_string()));

        let request = DecisionRequest::new(event_with_type).with_trace();
        engine.decide(request).await.expect("Execution failed")
    }

    /// Execute a pipeline with event data
    pub async fn execute_pipeline(
        &self,
        _pipeline_id: &str,
        event: HashMap<String, Value>,
    ) -> DecisionResponse {
        let combined = self.build_combined_yaml();

        // Write to temp file
        let temp_path = format!("/tmp/test_pipeline_{}.yaml", std::process::id());
        std::fs::write(&temp_path, &combined).expect("Failed to write temp file");

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(&temp_path)
            .build()
            .await
            .expect("Failed to build engine");

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        // Add event.type: test to trigger the pipeline
        let mut event_with_type = event;
        event_with_type.insert("type".to_string(), Value::String("test".to_string()));

        let request = DecisionRequest::new(event_with_type).with_trace();
        engine.decide(request).await.expect("Execution failed")
    }
}

/// Helper to create event data from key-value pairs
#[macro_export]
macro_rules! event {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(
            map.insert($key.to_string(), $value.into());
        )*
        map
    }};
}

/// Helper trait for converting values to corint_core::Value
pub trait IntoValue {
    fn into_value(self) -> Value;
}

impl IntoValue for i32 {
    fn into_value(self) -> Value {
        Value::Number(self as f64)
    }
}

impl IntoValue for i64 {
    fn into_value(self) -> Value {
        Value::Number(self as f64)
    }
}

impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::Number(self)
    }
}

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::String(self.to_string())
    }
}

impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::String(self)
    }
}

impl<T: IntoValue> IntoValue for Vec<T> {
    fn into_value(self) -> Value {
        Value::Array(self.into_iter().map(|v| v.into_value()).collect())
    }
}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}

/// Assertion helpers for DecisionResponse
pub trait ResponseAssertions {
    fn assert_action(&self, expected: corint_core::ast::Action);
    fn assert_score(&self, expected: i32);
    fn assert_score_range(&self, min: i32, max: i32);
    fn assert_triggered_rules(&self, expected: &[&str]);
    fn assert_triggered_rules_count(&self, count: usize);
}

impl ResponseAssertions for DecisionResponse {
    fn assert_action(&self, expected: corint_core::ast::Action) {
        let actual = self.result.action.clone();
        assert_eq!(
            actual,
            Some(expected.clone()),
            "Expected action {:?}, got {:?}",
            expected,
            actual
        );
    }

    fn assert_score(&self, expected: i32) {
        assert_eq!(
            self.result.score, expected,
            "Expected score {}, got {}",
            expected, self.result.score
        );
    }

    fn assert_score_range(&self, min: i32, max: i32) {
        assert!(
            self.result.score >= min && self.result.score <= max,
            "Expected score in range [{}, {}], got {}",
            min,
            max,
            self.result.score
        );
    }

    fn assert_triggered_rules(&self, expected: &[&str]) {
        let expected_set: std::collections::HashSet<_> =
            expected.iter().map(|s| s.to_string()).collect();
        let actual_set: std::collections::HashSet<_> =
            self.result.triggered_rules.iter().cloned().collect();
        assert_eq!(
            expected_set, actual_set,
            "Expected triggered rules {:?}, got {:?}",
            expected, self.result.triggered_rules
        );
    }

    fn assert_triggered_rules_count(&self, count: usize) {
        assert_eq!(
            self.result.triggered_rules.len(),
            count,
            "Expected {} triggered rules, got {}",
            count,
            self.result.triggered_rules.len()
        );
    }
}
