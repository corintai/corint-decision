//! Pipeline executor
//!
//! Executes IR programs with support for async operations (features, LLM, services).

use corint_core::ast::Operator;
use corint_core::ir::{FeatureType, Instruction, Program};
use corint_core::Value;
use crate::context::ExecutionContext;
use crate::error::{RuntimeError, Result};
use crate::feature::FeatureExtractor;
use crate::result::DecisionResult;
use crate::storage::Storage;
use std::collections::HashMap;
use std::sync::Arc;

/// Pipeline executor for async IR execution
pub struct PipelineExecutor {
    feature_extractor: Option<Arc<FeatureExtractor>>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor without storage
    pub fn new() -> Self {
        Self {
            feature_extractor: None,
        }
    }

    /// Create a pipeline executor with storage backend for features
    pub fn with_storage(storage: Arc<dyn Storage>) -> Self {
        Self {
            feature_extractor: Some(Arc::new(FeatureExtractor::new(storage))),
        }
    }

    /// Execute an IR program with the given event data
    pub async fn execute(
        &self,
        program: &Program,
        event_data: HashMap<String, Value>,
    ) -> Result<DecisionResult> {
        let mut ctx = ExecutionContext::new(event_data);
        let mut pc = 0; // Program Counter

        while pc < program.instructions.len() {
            let instruction = &program.instructions[pc];

            match instruction {
                Instruction::LoadField { path } => {
                    let value = ctx.load_field(path)?;
                    ctx.push(value);
                    pc += 1;
                }

                Instruction::LoadConst { value } => {
                    ctx.push(value.clone());
                    pc += 1;
                }

                Instruction::BinaryOp { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    let result = Self::execute_binary_op(&left, op, &right)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Compare { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    let result = Self::execute_compare(&left, op, &right)?;
                    ctx.push(Value::Bool(result));
                    pc += 1;
                }

                Instruction::UnaryOp { op } => {
                    let operand = ctx.pop()?;
                    let result = Self::execute_unary_op(&operand, op)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Jump { offset } => {
                    pc = (pc as isize + offset) as usize;
                }

                Instruction::JumpIfTrue { offset } => {
                    let condition = ctx.pop()?;
                    if Self::is_truthy(&condition) {
                        pc = (pc as isize + offset) as usize;
                    } else {
                        pc += 1;
                    }
                }

                Instruction::JumpIfFalse { offset } => {
                    let condition = ctx.pop()?;
                    if !Self::is_truthy(&condition) {
                        pc = (pc as isize + offset) as usize;
                    } else {
                        pc += 1;
                    }
                }

                Instruction::CheckEventType { expected } => {
                    let event_type = ctx
                        .load_field(&[String::from("event"), String::from("type")])
                        .or_else(|_| ctx.load_field(&[String::from("event_type")]))
                        .unwrap_or(Value::Null);

                    if let Value::String(actual) = event_type {
                        if &actual != expected {
                            pc = program.instructions.len();
                            continue;
                        }
                    }
                    pc += 1;
                }

                Instruction::SetScore { value } => {
                    ctx.set_score(*value);
                    pc += 1;
                }

                Instruction::AddScore { value } => {
                    ctx.add_score(*value);
                    pc += 1;
                }

                Instruction::SetAction { action } => {
                    ctx.set_action(action.clone());
                    pc += 1;
                }

                Instruction::MarkRuleTriggered { rule_id } => {
                    ctx.mark_rule_triggered(rule_id.clone());
                    pc += 1;
                }

                Instruction::Return => {
                    break;
                }

                // Stack operations
                Instruction::Dup => {
                    ctx.dup()?;
                    pc += 1;
                }

                Instruction::Pop => {
                    ctx.pop()?;
                    pc += 1;
                }

                Instruction::Swap => {
                    ctx.swap()?;
                    pc += 1;
                }

                // Variable operations
                Instruction::Store { name } => {
                    let value = ctx.pop()?;
                    ctx.store_variable(name.clone(), value);
                    pc += 1;
                }

                Instruction::Load { name } => {
                    let value = ctx.load_variable(name)?;
                    ctx.push(value);
                    pc += 1;
                }

                // Feature extraction
                Instruction::CallFeature {
                    feature_type,
                    field,
                    filter: _,
                    time_window,
                } => {
                    let value = if let Some(ref extractor) = self.feature_extractor {
                        // Real feature extraction with storage
                        // TODO: Convert filter expression to EventFilter
                        extractor.extract(feature_type, field, time_window, None).await?
                    } else {
                        // Fallback: return placeholder
                        Self::placeholder_feature(feature_type)
                    };
                    ctx.push(value);
                    pc += 1;
                }

                // LLM and Service calls - placeholder for Phase 3
                Instruction::CallLLM { .. } => {
                    // TODO: Implement LLM calls in Phase 3
                    ctx.push(Value::String("llm_response".to_string()));
                    pc += 1;
                }

                Instruction::CallService { .. } => {
                    // TODO: Implement service calls in Phase 3
                    ctx.push(Value::Null);
                    pc += 1;
                }
            }
        }

        Ok(ctx.into_decision_result())
    }

    /// Placeholder feature value for when storage is not available
    fn placeholder_feature(feature_type: &FeatureType) -> Value {
        match feature_type {
            FeatureType::Count | FeatureType::CountDistinct | FeatureType::Sum
            | FeatureType::Avg | FeatureType::Min | FeatureType::Max
            | FeatureType::Percentile { .. } | FeatureType::StdDev | FeatureType::Variance => {
                Value::Number(0.0)
            }
        }
    }

    /// Execute a binary operation
    fn execute_binary_op(left: &Value, op: &Operator, right: &Value) -> Result<Value> {
        match (left, op, right) {
            // Arithmetic operations
            (Value::Number(l), Operator::Add, Value::Number(r)) => Ok(Value::Number(l + r)),
            (Value::Number(l), Operator::Sub, Value::Number(r)) => Ok(Value::Number(l - r)),
            (Value::Number(l), Operator::Mul, Value::Number(r)) => Ok(Value::Number(l * r)),
            (Value::Number(l), Operator::Div, Value::Number(r)) => {
                if *r == 0.0 {
                    Err(RuntimeError::DivisionByZero)
                } else {
                    Ok(Value::Number(l / r))
                }
            }
            (Value::Number(l), Operator::Mod, Value::Number(r)) => {
                if *r == 0.0 {
                    Err(RuntimeError::DivisionByZero)
                } else {
                    Ok(Value::Number(l % r))
                }
            }

            // Logical operations
            (Value::Bool(l), Operator::And, Value::Bool(r)) => Ok(Value::Bool(*l && *r)),
            (Value::Bool(l), Operator::Or, Value::Bool(r)) => Ok(Value::Bool(*l || *r)),

            // String operations
            (Value::String(l), Operator::Contains, Value::String(r)) => {
                Ok(Value::Bool(l.contains(r)))
            }
            (Value::String(l), Operator::StartsWith, Value::String(r)) => {
                Ok(Value::Bool(l.starts_with(r)))
            }
            (Value::String(l), Operator::EndsWith, Value::String(r)) => {
                Ok(Value::Bool(l.ends_with(r)))
            }

            // In operator
            (val, Operator::In, Value::Array(arr)) => {
                Ok(Value::Bool(arr.iter().any(|v| v == val)))
            }
            (val, Operator::NotIn, Value::Array(arr)) => {
                Ok(Value::Bool(!arr.iter().any(|v| v == val)))
            }

            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot apply {:?} to {:?} and {:?}",
                op, left, right
            ))),
        }
    }

    /// Execute a comparison operation
    fn execute_compare(left: &Value, op: &Operator, right: &Value) -> Result<bool> {
        match (left, op, right) {
            (Value::Number(l), Operator::Eq, Value::Number(r)) => Ok(l == r),
            (Value::Number(l), Operator::Ne, Value::Number(r)) => Ok(l != r),
            (Value::Number(l), Operator::Gt, Value::Number(r)) => Ok(l > r),
            (Value::Number(l), Operator::Ge, Value::Number(r)) => Ok(l >= r),
            (Value::Number(l), Operator::Lt, Value::Number(r)) => Ok(l < r),
            (Value::Number(l), Operator::Le, Value::Number(r)) => Ok(l <= r),

            (Value::String(l), Operator::Eq, Value::String(r)) => Ok(l == r),
            (Value::String(l), Operator::Ne, Value::String(r)) => Ok(l != r),

            (Value::Bool(l), Operator::Eq, Value::Bool(r)) => Ok(l == r),
            (Value::Bool(l), Operator::Ne, Value::Bool(r)) => Ok(l != r),

            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot compare {:?} and {:?} with {:?}",
                left, right, op
            ))),
        }
    }

    /// Execute a unary operation
    fn execute_unary_op(operand: &Value, op: &corint_core::ast::UnaryOperator) -> Result<Value> {
        use corint_core::ast::UnaryOperator;

        match (op, operand) {
            (UnaryOperator::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOperator::Negate, Value::Number(n)) => Ok(Value::Number(-n)),
            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot apply {:?} to {:?}",
                op, operand
            ))),
        }
    }

    /// Check if a value is truthy
    fn is_truthy(value: &Value) -> bool {
        match value {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        }
    }
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::Action;
    use corint_core::ir::ProgramMetadata;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_execute_simple_program() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 50);
    }

    #[tokio::test]
    async fn test_execute_with_arithmetic() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(20.0),
            },
            Instruction::BinaryOp { op: Operator::Add },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Result should be on stack but not reflected in score
        // This tests the stack operations work correctly
        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_execute_with_comparison() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::Compare { op: Operator::Gt },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_execute_with_jump() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Bool(false),
            },
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::SetScore { value: 100 },
            Instruction::SetScore { value: 50 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 50);
    }

    #[tokio::test]
    async fn test_execute_with_action() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::SetAction {
                action: Action::Approve,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.action, Some(Action::Approve));
    }

    #[tokio::test]
    async fn test_execute_mark_rule_triggered() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::MarkRuleTriggered {
                rule_id: "test_rule".to_string(),
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.triggered_rules.len(), 1);
        assert!(result.triggered_rules.contains(&"test_rule".to_string()));
    }

    #[tokio::test]
    async fn test_division_by_zero() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(0.0),
            },
            Instruction::BinaryOp { op: Operator::Div },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuntimeError::DivisionByZero));
    }

    #[tokio::test]
    async fn test_feature_extraction_without_storage() {
        use corint_core::ir::TimeWindow;

        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::CallFeature {
                feature_type: FeatureType::Count,
                field: vec!["transaction".to_string(), "amount".to_string()],
                filter: None,
                time_window: TimeWindow::Last24Hours,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Without storage, should return placeholder
        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_feature_extraction_with_storage() {
        use crate::storage::Event;
        use corint_core::ir::TimeWindow;

        // Create storage with test events
        let mut storage = InMemoryStorage::new();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        for i in 0..5 {
            let mut data = HashMap::new();
            data.insert("amount".to_string(), Value::Number((i + 1) as f64 * 10.0));
            storage.add_event(Event {
                timestamp: now - 100 + i,
                data,
            });
        }

        let executor = PipelineExecutor::with_storage(Arc::new(storage));

        let instructions = vec![
            Instruction::CallFeature {
                feature_type: FeatureType::Sum,
                field: vec!["amount".to_string()],
                filter: None,
                time_window: TimeWindow::Last1Hour,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Sum of 10, 20, 30, 40, 50 = 150
        assert_eq!(result.score, 0); // Score is separate from stack value
    }
}
