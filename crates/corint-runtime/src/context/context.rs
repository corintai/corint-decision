//! Execution context implementation
//!
//! Contains the ExecutionContext struct and its implementation for managing state
//! during program execution with a flattened namespace architecture.

use crate::error::{Result, RuntimeError};
use crate::result::{DecisionResult, ExecutionResult};
use corint_core::ast::Signal;
use corint_core::Value;
use std::collections::HashMap;

/// Input structure for creating ExecutionContext with multi-namespace support
#[derive(Debug, Clone, Default)]
pub struct ContextInput {
    /// User request raw data (required)
    pub event: HashMap<String, Value>,
    /// Complex feature computation results (optional)
    pub features: Option<HashMap<String, Value>>,
    /// External API call results (optional)
    pub api: Option<HashMap<String, Value>>,
    /// Internal service call results (optional)
    pub service: Option<HashMap<String, Value>>,
    /// LLM analysis results (optional)
    pub llm: Option<HashMap<String, Value>>,
    /// Simple variables and intermediate calculations (optional)
    pub vars: Option<HashMap<String, Value>>,
}

impl ContextInput {
    /// Create a new ContextInput with only event data
    pub fn new(event: HashMap<String, Value>) -> Self {
        Self {
            event,
            features: None,
            api: None,
            service: None,
            llm: None,
            vars: None,
        }
    }

    /// Builder method to add features
    pub fn with_features(mut self, features: HashMap<String, Value>) -> Self {
        self.features = Some(features);
        self
    }

    /// Builder method to add API results
    pub fn with_api(mut self, api: HashMap<String, Value>) -> Self {
        self.api = Some(api);
        self
    }

    /// Builder method to add service results
    pub fn with_service(mut self, service: HashMap<String, Value>) -> Self {
        self.service = Some(service);
        self
    }

    /// Builder method to add LLM results
    pub fn with_llm(mut self, llm: HashMap<String, Value>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Builder method to add variables
    pub fn with_vars(mut self, vars: HashMap<String, Value>) -> Self {
        self.vars = Some(vars);
        self
    }
}

/// Execution context for running IR programs with flattened namespace architecture
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Value stack for intermediate calculations
    pub stack: Vec<Value>,

    // ========== 8 Namespaces (Flattened Architecture) ==========

    /// User request raw data (read-only)
    pub event: HashMap<String, Value>,

    /// Complex feature computation results (writable)
    pub features: HashMap<String, Value>,

    /// External API call results (writable)
    pub api: HashMap<String, Value>,

    /// Internal service call results (writable)
    pub service: HashMap<String, Value>,

    /// LLM analysis results (writable)
    pub llm: HashMap<String, Value>,

    /// Simple variables and intermediate calculations (writable)
    pub vars: HashMap<String, Value>,

    /// System injected metadata (read-only)
    pub sys: HashMap<String, Value>,

    /// Environment configuration (read-only)
    pub env: HashMap<String, Value>,

    /// Execution result (accumulated state)
    pub result: ExecutionResult,
}

impl ExecutionContext {
    /// Create a new execution context with multi-namespace input
    pub fn new(input: ContextInput) -> Result<Self> {
        // Validate event data doesn't contain reserved fields
        crate::validation::validate_event_data(&input.event)?;

        Ok(Self {
            stack: Vec::new(),
            event: input.event,
            features: input.features.unwrap_or_default(),
            api: input.api.unwrap_or_default(),
            service: input.service.unwrap_or_default(),
            llm: input.llm.unwrap_or_default(),
            vars: input.vars.unwrap_or_default(),
            sys: super::system_vars::build_system_vars(),
            env: super::env_vars::load_environment_vars(),
            result: ExecutionResult::new(),
        })
    }

    /// Create a new execution context from event data only (convenience method)
    pub fn from_event(event_data: HashMap<String, Value>) -> Result<Self> {
        Self::new(ContextInput::new(event_data))
    }

    /// Create a new execution context with multi-namespace input and existing result state
    pub fn with_result(input: ContextInput, result: ExecutionResult) -> Result<Self> {
        // Validate event data doesn't contain reserved fields
        crate::validation::validate_event_data(&input.event)?;

        Ok(Self {
            stack: Vec::new(),
            event: input.event,
            features: input.features.unwrap_or_default(),
            api: input.api.unwrap_or_default(),
            service: input.service.unwrap_or_default(),
            llm: input.llm.unwrap_or_default(),
            vars: input.vars.unwrap_or_default(),
            sys: super::system_vars::build_system_vars(),
            env: super::env_vars::load_environment_vars(),
            result,
        })
    }

    // ========== Data Storage Methods ==========

    /// Store feature computation result
    pub fn store_feature(&mut self, name: &str, value: Value) {
        self.features.insert(name.to_string(), value);
    }

    /// Store API call result
    pub fn store_api_result(&mut self, api_name: &str, result: Value) {
        self.api.insert(api_name.to_string(), result);
    }

    /// Store service call result
    pub fn store_service_result(&mut self, service_name: &str, result: Value) {
        self.service.insert(service_name.to_string(), result);
    }

    /// Store LLM analysis result
    pub fn store_llm_result(&mut self, step_id: &str, analysis: Value) {
        self.llm.insert(step_id.to_string(), analysis);
    }

    /// Store variable
    pub fn store_var(&mut self, name: &str, value: Value) {
        self.vars.insert(name.to_string(), value);
    }

    // ========== Field Lookup (supports all 8 namespaces) ==========

    /// Load a field value from any namespace
    ///
    /// Supports dot notation like:
    /// - event.user.id
    /// - features.user_transaction_count_7d
    /// - api.device_fingerprint.risk_score
    /// - service.user_profile.vip_level
    /// - llm.fraud_analysis.reason
    /// - vars.high_risk_threshold
    /// - sys.timestamp
    /// - env.feature_flags.new_model
    ///
    /// Returns Value::Null if field is not found (graceful handling)
    pub fn load_field(&self, path: &[String]) -> Result<Value> {
        if path.is_empty() {
            return Err(RuntimeError::FieldNotFound("empty path".to_string()));
        }

        let namespace = &path[0];
        let remaining_path = &path[1..];

        // Route to appropriate namespace
        let namespace_data = match namespace.as_str() {
            "event" => Some(&self.event),
            "features" => Some(&self.features),
            "api" => Some(&self.api),
            "service" => Some(&self.service),
            "llm" => Some(&self.llm),
            "vars" => Some(&self.vars),
            "sys" => Some(&self.sys),
            "env" => Some(&self.env),
            _ => None,
        };

        // If it's a namespace access, try the namespace
        if let Some(data) = namespace_data {
            // If only namespace name (no remaining path), return entire namespace
            if remaining_path.is_empty() {
                return Ok(Value::Object(data.clone()));
            }

            // Otherwise, navigate through the namespace
            return super::field_lookup::get_nested_value(data, remaining_path);
        }

        // Fallback for backward compatibility: try event namespace
        let mut current = self.event.get(&path[0]);

        // If not found in event, try variables (stored context)
        if current.is_none() {
            current = self.result.variables.get(&path[0]);
        }

        // If still not found and it's a single-path special field, use computed value
        if current.is_none() && path.len() == 1 {
            match path[0].as_str() {
                "total_score" => {
                    return Ok(Value::Number(self.result.score as f64));
                }
                "triggered_rules" => {
                    return Ok(Value::Array(
                        self.result
                            .triggered_rules
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect(),
                    ));
                }
                "triggered_count" => {
                    return Ok(Value::Number(self.result.triggered_rules.len() as f64));
                }
                _ => {}
            }
        }

        // If field not found, return Null instead of error
        // This allows rules to gracefully handle missing fields
        let Some(mut current) = current else {
            tracing::debug!("Field not found: {}, returning Null", path[0]);
            return Ok(Value::Null);
        };

        for segment in &path[1..] {
            match current {
                Value::Object(map) => {
                    // If nested field not found, return Null
                    let Some(next) = map.get(segment) else {
                        tracing::debug!("Nested field not found: {}, returning Null", segment);
                        return Ok(Value::Null);
                    };
                    current = next;
                }
                _ => {
                    // If trying to access field on non-object, return Null
                    tracing::debug!(
                        "Cannot access field '{}' on non-object, returning Null",
                        segment
                    );
                    return Ok(Value::Null);
                }
            }
        }

        Ok(current.clone())
    }

    // ========== Stack Operations ==========

    /// Push a value onto the stack
    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    /// Pop a value from the stack
    pub fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    /// Peek at the top value without popping
    pub fn peek(&self) -> Result<&Value> {
        self.stack.last().ok_or(RuntimeError::StackUnderflow)
    }

    /// Duplicate the top stack value
    pub fn dup(&mut self) -> Result<()> {
        let value = self.peek()?.clone();
        self.push(value);
        Ok(())
    }

    /// Swap the top two stack values
    pub fn swap(&mut self) -> Result<()> {
        if self.stack.len() < 2 {
            return Err(RuntimeError::StackUnderflow);
        }
        let len = self.stack.len();
        self.stack.swap(len - 1, len - 2);
        Ok(())
    }

    // ========== Variable Operations (backward compatibility) ==========

    /// Store a variable in the result
    pub fn store_variable(&mut self, name: String, value: Value) {
        self.result.store_variable(name, value);
    }

    /// Load a variable from the result
    pub fn load_variable(&self, name: &str) -> Result<Value> {
        self.result
            .load_variable(name)
            .cloned()
            .ok_or_else(|| RuntimeError::FieldNotFound(name.to_string()))
    }

    // ========== Score and Rule Operations ==========

    /// Add to the score
    pub fn add_score(&mut self, value: i32) {
        self.result.add_score(value);
    }

    /// Set the score
    pub fn set_score(&mut self, value: i32) {
        self.result.set_score(value);
    }

    /// Mark a rule as triggered
    pub fn mark_rule_triggered(&mut self, rule_id: String) {
        self.result.mark_rule_triggered(rule_id);
    }

    /// Set the signal
    pub fn set_signal(&mut self, signal: Signal) {
        self.result.signal = Some(signal);
    }

    /// Set the explicit reason/explanation
    pub fn set_reason(&mut self, reason: String) {
        self.result.explicit_explanation = Some(reason);
    }

    /// Set user-defined actions
    pub fn set_actions(&mut self, actions: Vec<String>) {
        self.result.actions = actions;
    }

    /// Add user-defined actions
    pub fn add_actions(&mut self, actions: Vec<String>) {
        self.result.actions.extend(actions);
    }

    // ========== Result Conversion ==========

    /// Convert context into a DecisionResult
    pub fn into_decision_result(self) -> DecisionResult {
        // Use explicit explanation if set, otherwise build one
        let explanation = match self.result.explicit_explanation {
            Some(exp) => exp,
            None => Self::build_explanation(&self.result, &self.event),
        };

        // Merge all writable namespaces into context
        let mut context = HashMap::new();

        // Add features
        for (k, v) in self.features {
            context.insert(k, v);
        }

        // Add api results
        for (k, v) in self.api {
            context.insert(k, v);
        }

        // Add service results
        for (k, v) in self.service {
            context.insert(k, v);
        }

        // Add llm results
        for (k, v) in self.llm {
            context.insert(k, v);
        }

        // Add vars
        for (k, v) in self.vars {
            context.insert(k, v);
        }

        // Add result variables for backward compatibility
        for (k, v) in self.result.variables {
            context.insert(k, v);
        }

        DecisionResult {
            signal: self.result.signal,
            actions: self.result.actions,
            score: self.result.score,
            triggered_rules: self.result.triggered_rules,
            explanation,
            context,
        }
    }

    /// Build explanation from execution result and event data
    /// Focus on WHY the decision was made, not repeating data that's already in response fields
    fn build_explanation(result: &ExecutionResult, _event_data: &HashMap<String, Value>) -> String {
        // Build a human-readable explanation focused on the reasoning

        // Case 1: No rules triggered - explain based on signal
        if result.triggered_rules.is_empty() {
            return match result.signal {
                Some(Signal::Approve) => "No risk indicators detected".to_string(),
                Some(Signal::Review) => "Sent to review based on policy thresholds".to_string(),
                Some(Signal::Decline) => "Blocked by policy rules".to_string(),
                Some(Signal::Hold) => "Additional verification required by policy".to_string(),
                Some(Signal::Pass) => "No decision made, deferred to next stage".to_string(),
                None => "No decision signal set".to_string(),
            };
        }

        // Case 2: Rules triggered - explain based on risk level
        let rule_count = result.triggered_rules.len();
        let score = result.score;

        // Build reason based on severity
        let mut reason = if rule_count == 1 {
            format!("Risk indicator detected: {}", result.triggered_rules[0])
        } else {
            format!("{} risk indicators detected", rule_count)
        };

        // Add context about score level if significant
        if score >= 80 {
            reason.push_str(" (high risk)");
        } else if score >= 50 {
            reason.push_str(" (medium risk)");
        } else if score > 0 {
            reason.push_str(" (low risk)");
        }

        reason
    }

    // ========== Utility Methods ==========

    /// Get the current stack depth
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// Clear the stack
    pub fn clear_stack(&mut self) {
        self.stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> HashMap<String, Value> {
        let mut event = HashMap::new();
        event.insert("user_id".to_string(), Value::String("123".to_string()));
        event.insert("amount".to_string(), Value::Number(1000.0));

        // Nested object
        let mut user = HashMap::new();
        user.insert("age".to_string(), Value::Number(25.0));
        user.insert("country".to_string(), Value::String("US".to_string()));
        event.insert("user".to_string(), Value::Object(user));

        event
    }

    #[test]
    fn test_stack_operations() {
        let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Push and pop
        ctx.push(Value::Number(42.0));
        ctx.push(Value::String("test".to_string()));

        assert_eq!(ctx.stack_depth(), 2);

        let val = ctx.pop().unwrap();
        assert_eq!(val, Value::String("test".to_string()));

        let val = ctx.pop().unwrap();
        assert_eq!(val, Value::Number(42.0));

        assert_eq!(ctx.stack_depth(), 0);
    }

    #[test]
    fn test_namespace_storage() {
        let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Store in different namespaces
        ctx.store_feature("user_count", Value::Number(15.0));
        ctx.store_api_result("device_fp", Value::Number(0.75));
        ctx.store_service_result("user_profile", Value::String("vip".to_string()));
        ctx.store_llm_result("fraud_check", Value::Bool(true));
        ctx.store_var("threshold", Value::Number(80.0));

        // Verify stored
        assert_eq!(ctx.features.len(), 1);
        assert_eq!(ctx.api.len(), 1);
        assert_eq!(ctx.service.len(), 1);
        assert_eq!(ctx.llm.len(), 1);
        assert_eq!(ctx.vars.len(), 1);
    }

    #[test]
    fn test_namespace_field_lookup() {
        let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Store test data
        ctx.store_feature("user_count", Value::Number(15.0));
        ctx.store_var("threshold", Value::Number(80.0));

        // Load from namespace
        let value = ctx
            .load_field(&[String::from("features"), String::from("user_count")])
            .unwrap();
        assert_eq!(value, Value::Number(15.0));

        let value = ctx
            .load_field(&[String::from("vars"), String::from("threshold")])
            .unwrap();
        assert_eq!(value, Value::Number(80.0));
    }

    #[test]
    fn test_sys_namespace() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Check core sys variables exist
        assert!(ctx.sys.contains_key("request_id"));

        // Time-related fields
        assert!(ctx.sys.contains_key("timestamp"));
        assert!(ctx.sys.contains_key("timestamp_ms"));
        assert!(ctx.sys.contains_key("timestamp_sec"));

        // Date components
        assert!(ctx.sys.contains_key("date"));
        assert!(ctx.sys.contains_key("year"));
        assert!(ctx.sys.contains_key("month"));
        assert!(ctx.sys.contains_key("day"));
        assert!(ctx.sys.contains_key("month_name"));
        assert!(ctx.sys.contains_key("quarter"));

        // Time components
        assert!(ctx.sys.contains_key("time"));
        assert!(ctx.sys.contains_key("hour"));
        assert!(ctx.sys.contains_key("minute"));
        assert!(ctx.sys.contains_key("second"));

        // Time periods
        assert!(ctx.sys.contains_key("time_of_day"));
        assert!(ctx.sys.contains_key("is_business_hours"));

        // Day of week
        assert!(ctx.sys.contains_key("day_of_week"));
        assert!(ctx.sys.contains_key("day_of_week_num"));
        assert!(ctx.sys.contains_key("is_weekend"));
        assert!(ctx.sys.contains_key("is_weekday"));
        assert!(ctx.sys.contains_key("day_of_year"));

        // Environment
        assert!(ctx.sys.contains_key("environment"));
        assert!(ctx.sys.contains_key("corint_version"));

        // Verify types
        assert!(matches!(ctx.sys.get("hour").unwrap(), Value::Number(_)));
        assert!(matches!(ctx.sys.get("is_weekend").unwrap(), Value::Bool(_)));
        assert!(matches!(ctx.sys.get("day_of_week").unwrap(), Value::String(_)));
    }

    #[test]
    fn test_load_field_event_namespace() {
        let event = create_test_event();
        let ctx = ExecutionContext::from_event(event).unwrap();

        // Load from event namespace
        let value = ctx
            .load_field(&[String::from("event"), String::from("user_id")])
            .unwrap();
        assert_eq!(value, Value::String("123".to_string()));

        // Load nested field
        let value = ctx
            .load_field(&[
                String::from("event"),
                String::from("user"),
                String::from("age"),
            ])
            .unwrap();
        assert_eq!(value, Value::Number(25.0));
    }

    #[test]
    fn test_load_field_not_found() {
        let event = create_test_event();
        let ctx = ExecutionContext::from_event(event).unwrap();

        // Field not found should return Null (graceful handling)
        let result = ctx.load_field(&[String::from("nonexistent")]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Null);
    }

    #[test]
    fn test_backward_compatibility() {
        let event = create_test_event();
        let ctx = ExecutionContext::from_event(event).unwrap();

        // Old style access (without namespace prefix) should still work
        let value = ctx.load_field(&[String::from("user_id")]).unwrap();
        assert_eq!(value, Value::String("123".to_string()));
    }

    #[test]
    fn test_validation_rejects_reserved_field() {
        let mut event = HashMap::new();
        event.insert("total_score".to_string(), Value::Number(100.0));

        let result = ExecutionContext::from_event(event);
        assert!(result.is_err());

        if let Err(RuntimeError::ReservedField { field, .. }) = result {
            assert_eq!(field, "total_score");
        } else {
            panic!("Expected ReservedField error");
        }
    }

    #[test]
    fn test_validation_rejects_reserved_prefix() {
        let mut event = HashMap::new();
        event.insert("sys_custom_field".to_string(), Value::String("test".to_string()));

        let result = ExecutionContext::from_event(event);
        assert!(result.is_err());

        if let Err(RuntimeError::ReservedField { field, .. }) = result {
            assert_eq!(field, "sys_custom_field");
        } else {
            panic!("Expected ReservedField error");
        }
    }

    #[test]
    fn test_validation_accepts_valid_event() {
        let mut event = HashMap::new();
        event.insert("user_id".to_string(), Value::String("123".to_string()));
        event.insert("amount".to_string(), Value::Number(1000.0));
        event.insert("custom_data".to_string(), Value::String("test".to_string()));

        let result = ExecutionContext::from_event(event);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_with_result() {
        let mut event = HashMap::new();
        event.insert("total_score".to_string(), Value::Number(100.0));

        let input = ContextInput::new(event);
        let exec_result = ExecutionResult::new();
        let result = ExecutionContext::with_result(input, exec_result);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_nested_reserved_field() {
        let mut event = HashMap::new();
        let mut nested = HashMap::new();
        nested.insert("total_score".to_string(), Value::Number(100.0));
        event.insert("data".to_string(), Value::Object(nested));

        let result = ExecutionContext::from_event(event);
        assert!(result.is_err());
    }

    #[test]
    fn test_env_namespace_defaults() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Check default configuration values exist
        assert!(ctx.env.contains_key("max_score"));
        assert!(ctx.env.contains_key("default_action"));
        assert!(ctx.env.contains_key("feature_flags"));

        // Verify default values
        assert_eq!(ctx.env.get("max_score").unwrap(), &Value::Number(100.0));
        assert_eq!(
            ctx.env.get("default_action").unwrap(),
            &Value::String("approve".to_string())
        );
    }

    #[test]
    fn test_env_namespace_feature_flags() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Get feature_flags object
        if let Some(Value::Object(feature_flags)) = ctx.env.get("feature_flags") {
            // Check default feature flags exist
            assert!(feature_flags.contains_key("enable_llm"));
            assert!(feature_flags.contains_key("enable_cache"));

            // Verify default values
            assert_eq!(
                feature_flags.get("enable_llm").unwrap(),
                &Value::Bool(false)
            );
            assert_eq!(
                feature_flags.get("enable_cache").unwrap(),
                &Value::Bool(true)
            );
        } else {
            panic!("feature_flags should be an object");
        }
    }

    #[test]
    fn test_env_namespace_field_access() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Access env.max_score
        let value = ctx
            .load_field(&[String::from("env"), String::from("max_score")])
            .unwrap();
        assert_eq!(value, Value::Number(100.0));

        // Access nested feature flag: env.feature_flags.enable_cache
        let value = ctx
            .load_field(&[
                String::from("env"),
                String::from("feature_flags"),
                String::from("enable_cache"),
            ])
            .unwrap();
        assert_eq!(value, Value::Bool(true));
    }

    #[test]
    fn test_sys_time_components() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        // Verify hour is valid (0-23)
        if let Some(Value::Number(hour)) = ctx.sys.get("hour") {
            assert!(*hour >= 0.0 && *hour < 24.0);
        } else {
            panic!("hour should be a number");
        }

        // Verify minute is valid (0-59)
        if let Some(Value::Number(minute)) = ctx.sys.get("minute") {
            assert!(*minute >= 0.0 && *minute < 60.0);
        } else {
            panic!("minute should be a number");
        }

        // Verify second is valid (0-59)
        if let Some(Value::Number(second)) = ctx.sys.get("second") {
            assert!(*second >= 0.0 && *second < 60.0);
        } else {
            panic!("second should be a number");
        }

        // Verify month is valid (1-12)
        if let Some(Value::Number(month)) = ctx.sys.get("month") {
            assert!(*month >= 1.0 && *month <= 12.0);
        } else {
            panic!("month should be a number");
        }

        // Verify day is valid (1-31)
        if let Some(Value::Number(day)) = ctx.sys.get("day") {
            assert!(*day >= 1.0 && *day <= 31.0);
        } else {
            panic!("day should be a number");
        }

        // Verify quarter is valid (1-4)
        if let Some(Value::Number(quarter)) = ctx.sys.get("quarter") {
            assert!(*quarter >= 1.0 && *quarter <= 4.0);
        } else {
            panic!("quarter should be a number");
        }

        // Verify day_of_week_num is valid (1-7)
        if let Some(Value::Number(dow)) = ctx.sys.get("day_of_week_num") {
            assert!(*dow >= 1.0 && *dow <= 7.0);
        } else {
            panic!("day_of_week_num should be a number");
        }
    }

    #[test]
    fn test_sys_time_of_day() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        if let Some(Value::String(time_of_day)) = ctx.sys.get("time_of_day") {
            // Should be one of the four periods
            assert!(
                time_of_day == "night"
                    || time_of_day == "morning"
                    || time_of_day == "afternoon"
                    || time_of_day == "evening"
            );
        } else {
            panic!("time_of_day should be a string");
        }
    }

    #[test]
    fn test_sys_weekday_weekend_consistency() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        let is_weekend = if let Some(Value::Bool(b)) = ctx.sys.get("is_weekend") {
            *b
        } else {
            panic!("is_weekend should be a bool");
        };

        let is_weekday = if let Some(Value::Bool(b)) = ctx.sys.get("is_weekday") {
            *b
        } else {
            panic!("is_weekday should be a bool");
        };

        // is_weekend and is_weekday should be opposites
        assert_eq!(is_weekend, !is_weekday);
    }

    #[test]
    fn test_sys_month_name() {
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        if let Some(Value::String(month_name)) = ctx.sys.get("month_name") {
            let valid_months = vec![
                "january", "february", "march", "april", "may", "june",
                "july", "august", "september", "october", "november", "december"
            ];
            assert!(valid_months.contains(&month_name.as_str()));
        } else {
            panic!("month_name should be a string");
        }
    }
}
