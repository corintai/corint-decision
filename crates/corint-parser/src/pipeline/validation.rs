//! Pipeline step field validation
//!
//! Defines valid fields for each step type and provides validation utilities.

/// Common fields for all step types
pub(super) const COMMON_STEP_FIELDS: &[&str] = &["id", "name", "type", "next", "when"];

/// Fields specific to each step type (aligned with Pipeline DSL v2.0)
pub(super) const FUNCTION_STEP_FIELDS: &[&str] = &["function", "params"];
pub(super) const RULE_STEP_FIELDS: &[&str] = &["rule"];
pub(super) const RULESET_STEP_FIELDS: &[&str] = &["ruleset"];
pub(super) const PIPELINE_STEP_FIELDS: &[&str] = &["pipeline", "inline"];
pub(super) const API_STEP_FIELDS: &[&str] = &["api", "any", "all", "params", "endpoint", "output", "timeout", "on_error", "min_success"];
pub(super) const SERVICE_STEP_FIELDS: &[&str] = &["service", "query", "params"];
pub(super) const ROUTER_STEP_FIELDS: &[&str] = &["routes", "default"];
pub(super) const TRIGGER_STEP_FIELDS: &[&str] = &["target", "params"];
// Legacy step types (for backward compatibility only)
pub(super) const EXTRACT_STEP_FIELDS: &[&str] = &["features"];
pub(super) const REASON_STEP_FIELDS: &[&str] = &["provider", "model", "prompt", "output_schema"];

/// Helper function to combine common fields with type-specific fields
///
/// Returns the list of valid field names for a given step type, including both
/// common fields (id, name, type, next, when) and type-specific fields.
pub(super) fn get_valid_fields_for_step_type(step_type: &str) -> Vec<&'static str> {
    let mut fields: Vec<&str> = COMMON_STEP_FIELDS.to_vec();

    let type_specific = match step_type {
        // Pipeline DSL v2.0 step types
        "router" => ROUTER_STEP_FIELDS,
        "function" => FUNCTION_STEP_FIELDS,
        "rule" => RULE_STEP_FIELDS,
        "ruleset" => RULESET_STEP_FIELDS,
        "pipeline" => PIPELINE_STEP_FIELDS,
        "service" => SERVICE_STEP_FIELDS,
        "api" => API_STEP_FIELDS,
        "trigger" => TRIGGER_STEP_FIELDS,
        // Legacy step types (backward compatibility)
        "extract" => EXTRACT_STEP_FIELDS,
        "reason" => REASON_STEP_FIELDS,
        _ => &[],
    };

    fields.extend_from_slice(type_specific);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_valid_fields_for_router_step() {
        let fields = get_valid_fields_for_step_type("router");
        assert!(fields.contains(&"id"));
        assert!(fields.contains(&"name"));
        assert!(fields.contains(&"type"));
        assert!(fields.contains(&"routes"));
        assert!(fields.contains(&"default"));
    }

    #[test]
    fn test_get_valid_fields_for_function_step() {
        let fields = get_valid_fields_for_step_type("function");
        assert!(fields.contains(&"function"));
        assert!(fields.contains(&"params"));
    }

    #[test]
    fn test_get_valid_fields_for_unknown_step() {
        let fields = get_valid_fields_for_step_type("unknown");
        // Should only have common fields
        assert_eq!(fields.len(), COMMON_STEP_FIELDS.len());
    }
}
