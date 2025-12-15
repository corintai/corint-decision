//! IR Instructions
//!
//! Low-level instructions for the CORINT runtime execution engine.

use crate::ast::{Action, Expression, Operator};
use crate::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single IR instruction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    // ===== Data Loading =====
    /// Load a field value from the event/context onto the stack
    LoadField {
        /// Field path (e.g., ["user", "id"])
        path: Vec<String>,
    },

    /// Load a constant value onto the stack
    LoadConst {
        /// The constant value
        value: Value,
    },

    // ===== Operations =====
    /// Perform a binary operation (+ - * / etc.)
    BinaryOp {
        /// The operator to apply
        op: Operator,
    },

    /// Perform a comparison operation (== != < > etc.)
    Compare {
        /// The comparison operator
        op: Operator,
    },

    /// Perform a unary operation (! -)
    UnaryOp {
        /// The unary operator
        op: crate::ast::UnaryOperator,
    },

    // ===== Control Flow =====
    /// Unconditional jump to offset
    Jump {
        /// Offset to jump (can be negative)
        offset: isize,
    },

    /// Jump if top of stack is true
    JumpIfTrue {
        /// Offset to jump
        offset: isize,
    },

    /// Jump if top of stack is false
    JumpIfFalse {
        /// Offset to jump
        offset: isize,
    },

    /// Return from execution
    Return,

    // ===== Event Checking =====
    /// Check if event type matches expected value
    CheckEventType {
        /// Expected event type
        expected: String,
    },

    // ===== Feature Extraction =====
    /// Call a feature extraction function
    CallFeature {
        /// Type of feature to extract
        feature_type: FeatureType,
        /// Field to extract from
        field: Vec<String>,
        /// Optional filter expression
        filter: Option<Box<Expression>>,
        /// Time window for the query
        time_window: TimeWindow,
    },

    // ===== External Calls =====
    /// Call LLM for reasoning
    CallLLM {
        /// LLM provider (e.g., "openai")
        provider: String,
        /// Model name (e.g., "gpt-4")
        model: String,
        /// Prompt template
        prompt: String,
    },

    /// Call external service (internal)
    CallService {
        /// Service name
        service: String,
        /// Operation to perform
        operation: String,
        /// Parameters for the call
        params: HashMap<String, Value>,
    },

    /// Call external API (third-party)
    CallExternal {
        /// API identifier (e.g., "ipinfo")
        api: String,
        /// Endpoint name
        endpoint: String,
        /// Parameters for the call
        params: HashMap<String, Value>,
        /// Timeout in milliseconds
        timeout: Option<u64>,
        /// Fallback value on error
        fallback: Option<Value>,
    },

    // ===== Decision Making =====
    /// Set the score
    SetScore {
        /// Score value
        value: i32,
    },

    /// Add to the current score
    AddScore {
        /// Score to add
        value: i32,
    },

    /// Set the action
    SetAction {
        /// Action to take
        action: Action,
    },

    /// Mark a rule as triggered
    MarkRuleTriggered {
        /// Rule ID
        rule_id: String,
    },

    /// Mark a branch as executed (for tracing)
    MarkBranchExecuted {
        /// Branch index (0-based)
        branch_index: usize,
        /// Branch condition expression as string
        condition: String,
    },

    /// Call/execute a ruleset by ID
    CallRuleset {
        /// Ruleset ID to execute
        ruleset_id: String,
    },

    // ===== Stack Operations =====
    /// Duplicate the top stack value
    Dup,

    /// Pop and discard the top stack value
    Pop,

    /// Swap the top two stack values
    Swap,

    // ===== Variable Operations =====
    /// Store top of stack to a variable
    Store {
        /// Variable name
        name: String,
    },

    /// Load a variable onto the stack
    Load {
        /// Variable name
        name: String,
    },
}

/// Type of feature to extract
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureType {
    /// Count of occurrences
    Count,

    /// Count of distinct values
    CountDistinct,

    /// Sum of values
    Sum,

    /// Average of values
    Avg,

    /// Minimum value
    Min,

    /// Maximum value
    Max,

    /// Percentile calculation
    Percentile {
        /// Percentile value (0.0 - 1.0)
        p: f64,
    },

    /// Standard deviation
    StdDev,

    /// Variance
    Variance,
}

/// Time window for feature extraction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindow {
    /// Last 1 hour
    Last1Hour,

    /// Last 24 hours
    Last24Hours,

    /// Last 7 days
    Last7Days,

    /// Last 30 days
    Last30Days,

    /// Custom duration in seconds
    Custom {
        /// Duration in seconds
        seconds: u64,
    },
}

impl TimeWindow {
    /// Get duration in seconds
    pub fn seconds(&self) -> u64 {
        match self {
            TimeWindow::Last1Hour => 3600,
            TimeWindow::Last24Hours => 86400,
            TimeWindow::Last7Days => 604800,
            TimeWindow::Last30Days => 2592000,
            TimeWindow::Custom { seconds } => *seconds,
        }
    }
}

impl FeatureType {
    /// Check if this feature type requires aggregation
    pub fn is_aggregate(&self) -> bool {
        !matches!(self, FeatureType::Count)
    }

    /// Get the name of this feature type
    pub fn name(&self) -> &str {
        match self {
            FeatureType::Count => "count",
            FeatureType::CountDistinct => "count_distinct",
            FeatureType::Sum => "sum",
            FeatureType::Avg => "avg",
            FeatureType::Min => "min",
            FeatureType::Max => "max",
            FeatureType::Percentile { .. } => "percentile",
            FeatureType::StdDev => "stddev",
            FeatureType::Variance => "variance",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_instructions() {
        let load_field = Instruction::LoadField {
            path: vec!["user".to_string(), "id".to_string()],
        };

        let load_const = Instruction::LoadConst {
            value: Value::Number(42.0),
        };

        assert!(matches!(load_field, Instruction::LoadField { .. }));
        assert!(matches!(load_const, Instruction::LoadConst { .. }));
    }

    #[test]
    fn test_operation_instructions() {
        let binary_op = Instruction::BinaryOp { op: Operator::Add };
        let compare = Instruction::Compare { op: Operator::Gt };

        assert!(matches!(binary_op, Instruction::BinaryOp { .. }));
        assert!(matches!(compare, Instruction::Compare { .. }));
    }

    #[test]
    fn test_control_flow_instructions() {
        let jump = Instruction::Jump { offset: 10 };
        let jump_if_true = Instruction::JumpIfTrue { offset: 5 };
        let jump_if_false = Instruction::JumpIfFalse { offset: -3 };
        let return_inst = Instruction::Return;

        assert!(matches!(jump, Instruction::Jump { offset: 10 }));
        assert!(matches!(
            jump_if_true,
            Instruction::JumpIfTrue { offset: 5 }
        ));
        assert!(matches!(
            jump_if_false,
            Instruction::JumpIfFalse { offset: -3 }
        ));
        assert!(matches!(return_inst, Instruction::Return));
    }

    #[test]
    fn test_feature_extraction() {
        let call_feature = Instruction::CallFeature {
            feature_type: FeatureType::CountDistinct,
            field: vec!["device".to_string(), "id".to_string()],
            filter: None,
            time_window: TimeWindow::Last24Hours,
        };

        if let Instruction::CallFeature {
            feature_type,
            field,
            time_window,
            ..
        } = call_feature
        {
            assert_eq!(feature_type, FeatureType::CountDistinct);
            assert_eq!(field.len(), 2);
            assert_eq!(time_window, TimeWindow::Last24Hours);
        } else {
            panic!("Expected CallFeature instruction");
        }
    }

    #[test]
    fn test_llm_call() {
        let call_llm = Instruction::CallLLM {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            prompt: "Analyze this event".to_string(),
        };

        if let Instruction::CallLLM {
            provider,
            model,
            prompt,
        } = call_llm
        {
            assert_eq!(provider, "openai");
            assert_eq!(model, "gpt-4");
            assert_eq!(prompt, "Analyze this event");
        } else {
            panic!("Expected CallLLM instruction");
        }
    }

    #[test]
    fn test_decision_instructions() {
        let set_score = Instruction::SetScore { value: 100 };
        let add_score = Instruction::AddScore { value: 50 };
        let set_action = Instruction::SetAction {
            action: Action::Deny,
        };
        let mark_triggered = Instruction::MarkRuleTriggered {
            rule_id: "rule_123".to_string(),
        };

        assert!(matches!(set_score, Instruction::SetScore { value: 100 }));
        assert!(matches!(add_score, Instruction::AddScore { value: 50 }));
        assert!(matches!(
            set_action,
            Instruction::SetAction {
                action: Action::Deny
            }
        ));
        assert!(matches!(
            mark_triggered,
            Instruction::MarkRuleTriggered { .. }
        ));
    }

    #[test]
    fn test_stack_operations() {
        let dup = Instruction::Dup;
        let pop = Instruction::Pop;
        let swap = Instruction::Swap;

        assert!(matches!(dup, Instruction::Dup));
        assert!(matches!(pop, Instruction::Pop));
        assert!(matches!(swap, Instruction::Swap));
    }

    #[test]
    fn test_variable_operations() {
        let store = Instruction::Store {
            name: "temp_var".to_string(),
        };
        let load = Instruction::Load {
            name: "temp_var".to_string(),
        };

        assert!(matches!(store, Instruction::Store { .. }));
        assert!(matches!(load, Instruction::Load { .. }));
    }

    #[test]
    fn test_feature_types() {
        assert_eq!(FeatureType::Count.name(), "count");
        assert_eq!(FeatureType::CountDistinct.name(), "count_distinct");
        assert_eq!(FeatureType::Sum.name(), "sum");
        assert_eq!(FeatureType::Avg.name(), "avg");
        assert_eq!(FeatureType::Percentile { p: 0.95 }.name(), "percentile");

        assert!(!FeatureType::Count.is_aggregate());
        assert!(FeatureType::Sum.is_aggregate());
        assert!(FeatureType::CountDistinct.is_aggregate());
    }

    #[test]
    fn test_time_windows() {
        assert_eq!(TimeWindow::Last1Hour.seconds(), 3600);
        assert_eq!(TimeWindow::Last24Hours.seconds(), 86400);
        assert_eq!(TimeWindow::Last7Days.seconds(), 604800);
        assert_eq!(TimeWindow::Last30Days.seconds(), 2592000);
        assert_eq!(TimeWindow::Custom { seconds: 7200 }.seconds(), 7200);
    }

    #[test]
    fn test_instruction_serde() {
        let inst = Instruction::LoadConst {
            value: Value::Number(42.0),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&inst).unwrap();
        assert!(json.contains("LoadConst"));

        // Deserialize back
        let deserialized: Instruction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, inst);
    }
}
