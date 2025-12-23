//! Execution tracing types for detailed decision debugging
//!
//! These structures capture detailed information about rule evaluation,
//! condition matching, and decision logic execution.

use corint_core::Value;
use serde::{Deserialize, Serialize};

/// Trace of a single condition evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionTrace {
    /// The condition expression as a string (e.g., "event.transaction.amount > 10000")
    pub expression: String,

    /// The actual left-hand value during evaluation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_value: Option<Value>,

    /// The operator used (e.g., ">", "==", "in", "contains")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// The actual right-hand value during evaluation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_value: Option<Value>,

    /// The evaluation result
    pub result: bool,

    /// Nested conditions for logical groups (any/all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested: Option<Vec<ConditionTrace>>,

    /// The logical group type if this is a group ("any" or "all")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
}

impl ConditionTrace {
    /// Create a new simple condition trace
    pub fn new(expression: String, result: bool) -> Self {
        Self {
            expression,
            left_value: None,
            operator: None,
            right_value: None,
            result,
            nested: None,
            group_type: None,
        }
    }

    /// Create a binary condition trace with left/right values
    pub fn binary(
        expression: String,
        left_value: Value,
        operator: &str,
        right_value: Value,
        result: bool,
    ) -> Self {
        Self {
            expression,
            left_value: Some(left_value),
            operator: Some(operator.to_string()),
            right_value: Some(right_value),
            result,
            nested: None,
            group_type: None,
        }
    }

    /// Create a logical group trace (any/all)
    pub fn group(group_type: &str, nested: Vec<ConditionTrace>, result: bool) -> Self {
        Self {
            expression: format!("{}:[...]", group_type),
            left_value: None,
            operator: None,
            right_value: None,
            result,
            nested: Some(nested),
            group_type: Some(group_type.to_string()),
        }
    }
}

/// Trace of a single rule evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTrace {
    /// The rule ID
    pub rule_id: String,

    /// The rule name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,

    /// Whether the rule was triggered
    pub triggered: bool,

    /// Score contribution (if triggered)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,

    /// Detailed condition evaluation traces
    pub conditions: Vec<ConditionTrace>,

    /// Execution time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
}

impl RuleTrace {
    /// Create a new rule trace
    pub fn new(rule_id: String) -> Self {
        Self {
            rule_id,
            rule_name: None,
            triggered: false,
            score: None,
            conditions: Vec::new(),
            execution_time_ms: None,
        }
    }

    /// Set the rule as triggered with a score
    pub fn set_triggered(mut self, score: i32) -> Self {
        self.triggered = true;
        self.score = Some(score);
        self
    }

    /// Add a condition trace
    pub fn add_condition(mut self, condition: ConditionTrace) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set execution time
    pub fn with_execution_time(mut self, ms: u64) -> Self {
        self.execution_time_ms = Some(ms);
        self
    }
}

/// Trace of conclusion logic evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionTrace {
    /// The condition being evaluated (e.g., "total_score >= 100")
    pub condition: String,

    /// Whether this condition matched
    pub matched: bool,

    /// The resulting signal if matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,

    /// The reason/explanation if matched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ConclusionTrace {
    /// Create a new conclusion trace
    pub fn new(condition: String, matched: bool) -> Self {
        Self {
            condition,
            matched,
            signal: None,
            reason: None,
        }
    }

    /// Create a matched conclusion trace with signal
    pub fn matched(condition: String, signal: &str, reason: Option<&str>) -> Self {
        Self {
            condition,
            matched: true,
            signal: Some(signal.to_string()),
            reason: reason.map(|s| s.to_string()),
        }
    }
}

// Backwards compatibility alias
#[deprecated(since = "0.1.0", note = "Use ConclusionTrace instead")]
pub type DecisionLogicTrace = ConclusionTrace;

/// Trace of a ruleset evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetTrace {
    /// The ruleset ID
    pub ruleset_id: String,

    /// Traces of all rules in this ruleset
    pub rules: Vec<RuleTrace>,

    /// Total accumulated score from this ruleset
    pub total_score: i32,

    /// Conclusion logic evaluation traces
    pub conclusion: Vec<ConclusionTrace>,

    /// The final signal from this ruleset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,

    /// The final reason/explanation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl RulesetTrace {
    /// Create a new ruleset trace
    pub fn new(ruleset_id: String) -> Self {
        Self {
            ruleset_id,
            rules: Vec::new(),
            total_score: 0,
            conclusion: Vec::new(),
            signal: None,
            reason: None,
        }
    }

    /// Add a rule trace
    pub fn add_rule(mut self, rule: RuleTrace) -> Self {
        if rule.triggered {
            if let Some(score) = rule.score {
                self.total_score += score;
            }
        }
        self.rules.push(rule);
        self
    }

    /// Set the final decision
    pub fn with_decision(mut self, signal: &str, reason: Option<&str>) -> Self {
        self.signal = Some(signal.to_string());
        self.reason = reason.map(|s| s.to_string());
        self
    }
}

/// Trace of a pipeline step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTrace {
    /// The step ID
    pub step_id: String,

    /// The step name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_name: Option<String>,

    /// The step type (router, ruleset, etc.)
    pub step_type: String,

    /// Whether this step was executed
    pub executed: bool,

    /// The next step ID (where execution continued)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,

    /// For router steps: whether default route was taken
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_route: Option<bool>,

    /// For ruleset steps: the ruleset ID being executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruleset_id: Option<String>,

    /// Condition evaluation traces for this step (for router steps, shows which condition matched)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionTrace>,

    /// Execution time for this step in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
}

impl StepTrace {
    /// Create a new step trace
    pub fn new(step_id: String, step_type: String) -> Self {
        Self {
            step_id,
            step_name: None,
            step_type,
            executed: false,
            next_step: None,
            default_route: None,
            ruleset_id: None,
            conditions: Vec::new(),
            execution_time_ms: None,
        }
    }

    /// Mark the step as executed
    pub fn mark_executed(mut self) -> Self {
        self.executed = true;
        self
    }

    /// Set the next step
    pub fn with_next_step(mut self, next: String) -> Self {
        self.next_step = Some(next);
        self
    }

    /// Mark that default route was taken
    pub fn with_default_route(mut self) -> Self {
        self.default_route = Some(true);
        self
    }

    /// Set the ruleset ID for ruleset steps
    pub fn with_ruleset(mut self, ruleset_id: String) -> Self {
        self.ruleset_id = Some(ruleset_id);
        self
    }

    /// Add a condition trace
    pub fn add_condition(mut self, condition: ConditionTrace) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set execution time
    pub fn with_execution_time(mut self, ms: u64) -> Self {
        self.execution_time_ms = Some(ms);
        self
    }

    /// Set step name
    pub fn with_name(mut self, name: String) -> Self {
        self.step_name = Some(name);
        self
    }
}

/// Trace of a pipeline execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTrace {
    /// The pipeline ID
    pub pipeline_id: String,

    /// When condition evaluation traces
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub when_conditions: Vec<ConditionTrace>,

    /// Step execution traces (ordered by execution)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepTrace>,

    /// Index of the executed branch (for branch steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_branch: Option<usize>,

    /// Branch condition evaluation traces
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub branch_conditions: Vec<ConditionTrace>,

    /// Ruleset execution traces
    pub rulesets: Vec<RulesetTrace>,

    /// Final conclusion evaluation traces
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub final_conclusion: Vec<ConclusionTrace>,
}

impl PipelineTrace {
    /// Create a new pipeline trace
    pub fn new(pipeline_id: String) -> Self {
        Self {
            pipeline_id,
            when_conditions: Vec::new(),
            steps: Vec::new(),
            executed_branch: None,
            branch_conditions: Vec::new(),
            rulesets: Vec::new(),
            final_conclusion: Vec::new(),
        }
    }

    /// Add a step trace
    pub fn add_step(mut self, step: StepTrace) -> Self {
        self.steps.push(step);
        self
    }

    /// Add a step trace by mutable reference
    pub fn push_step(&mut self, step: StepTrace) {
        self.steps.push(step);
    }

    /// Add a when condition trace
    pub fn add_when_condition(mut self, condition: ConditionTrace) -> Self {
        self.when_conditions.push(condition);
        self
    }

    /// Set the executed branch
    pub fn with_executed_branch(mut self, branch: usize, conditions: Vec<ConditionTrace>) -> Self {
        self.executed_branch = Some(branch);
        self.branch_conditions = conditions;
        self
    }

    /// Add a ruleset trace
    pub fn add_ruleset(mut self, ruleset: RulesetTrace) -> Self {
        self.rulesets.push(ruleset);
        self
    }
}

/// Complete execution trace for a decision request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Pipeline execution trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<PipelineTrace>,

    /// Total execution time in milliseconds (internal use only, not serialized)
    #[serde(skip)]
    pub total_time_ms: u64,

    /// Number of rules evaluated (internal use only, not serialized)
    #[serde(skip)]
    pub rules_evaluated: usize,

    /// Number of rules triggered (internal use only, not serialized)
    #[serde(skip)]
    pub rules_triggered: usize,
}

impl ExecutionTrace {
    /// Create a new execution trace
    pub fn new() -> Self {
        Self {
            pipeline: None,
            total_time_ms: 0,
            rules_evaluated: 0,
            rules_triggered: 0,
        }
    }

    /// Set the pipeline trace
    pub fn with_pipeline(mut self, pipeline: PipelineTrace) -> Self {
        // Calculate statistics from pipeline
        for ruleset in &pipeline.rulesets {
            self.rules_evaluated += ruleset.rules.len();
            self.rules_triggered += ruleset.rules.iter().filter(|r| r.triggered).count();
        }
        self.pipeline = Some(pipeline);
        self
    }

    /// Set total execution time
    pub fn with_time(mut self, ms: u64) -> Self {
        self.total_time_ms = ms;
        self
    }
}

impl Default for ExecutionTrace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_condition_trace_binary() {
        let trace = ConditionTrace::binary(
            "event.amount > 1000".to_string(),
            Value::Number(1500.0),
            ">",
            Value::Number(1000.0),
            true,
        );

        assert!(trace.result);
        assert_eq!(trace.operator, Some(">".to_string()));
        assert_eq!(trace.left_value, Some(Value::Number(1500.0)));
    }

    #[test]
    fn test_condition_trace_group() {
        let nested = vec![
            ConditionTrace::new("cond1".to_string(), true),
            ConditionTrace::new("cond2".to_string(), false),
        ];
        let trace = ConditionTrace::group("any", nested, true);

        assert!(trace.result);
        assert_eq!(trace.group_type, Some("any".to_string()));
        assert_eq!(trace.nested.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_rule_trace() {
        let trace = RuleTrace::new("test_rule".to_string())
            .set_triggered(50)
            .add_condition(ConditionTrace::new("cond".to_string(), true))
            .with_execution_time(5);

        assert!(trace.triggered);
        assert_eq!(trace.score, Some(50));
        assert_eq!(trace.conditions.len(), 1);
    }

    #[test]
    fn test_ruleset_trace() {
        let rule1 = RuleTrace::new("rule1".to_string()).set_triggered(30);
        let rule2 = RuleTrace::new("rule2".to_string()).set_triggered(20);

        let trace = RulesetTrace::new("test_ruleset".to_string())
            .add_rule(rule1)
            .add_rule(rule2)
            .with_decision("review", Some("High score"));

        assert_eq!(trace.total_score, 50);
        assert_eq!(trace.rules.len(), 2);
        assert_eq!(trace.signal, Some("review".to_string()));
    }

    #[test]
    fn test_execution_trace_statistics() {
        let rule1 = RuleTrace::new("rule1".to_string()).set_triggered(30);
        let rule2 = RuleTrace::new("rule2".to_string()); // Not triggered
        let rule3 = RuleTrace::new("rule3".to_string()).set_triggered(20);

        let ruleset = RulesetTrace::new("test_ruleset".to_string())
            .add_rule(rule1)
            .add_rule(rule2)
            .add_rule(rule3);

        let pipeline = PipelineTrace::new("test_pipeline".to_string()).add_ruleset(ruleset);

        let trace = ExecutionTrace::new().with_pipeline(pipeline).with_time(10);

        assert_eq!(trace.rules_evaluated, 3);
        assert_eq!(trace.rules_triggered, 2);
        assert_eq!(trace.total_time_ms, 10);
    }

    #[test]
    fn test_serialization() {
        let trace = ConditionTrace::binary(
            "event.amount > 1000".to_string(),
            Value::Number(1500.0),
            ">",
            Value::Number(1000.0),
            true,
        );

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("\"result\":true"));
        assert!(json.contains("\"operator\":\">\""));
    }
}
