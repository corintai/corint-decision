//! IR instruction executor
//!
//! Executes IR programs in a stack-based virtual machine.

use crate::context::ExecutionContext;
use crate::error::{Result, RuntimeError};
use crate::result::ExecutionResult;
use corint_core::ast::{Operator, UnaryOperator};
use corint_core::ir::{Instruction, Program};
use corint_core::Value;
use std::collections::HashMap;

/// IR program executor
pub struct Executor;

impl Executor {
    /// Execute an IR program with the given event data
    pub fn execute(
        program: &Program,
        event_data: HashMap<String, Value>,
    ) -> Result<ExecutionResult> {
        let mut ctx = ExecutionContext::from_event(event_data)?;
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
                    // Check if event_type matches expected
                    let event_type = ctx
                        .load_field(&[String::from("event_type")])
                        .unwrap_or(Value::Null);

                    if let Value::String(actual) = event_type {
                        if &actual != expected {
                            // Event type mismatch, skip to end
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

                Instruction::SetAction { .. } => {
                    // Action setting would be handled at a higher level
                    // For now, just skip it
                    pc += 1;
                }

                Instruction::MarkRuleTriggered { rule_id } => {
                    ctx.mark_rule_triggered(rule_id.clone());
                    pc += 1;
                }

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

                Instruction::Return => {
                    break;
                }

                _ => {
                    // Unsupported instruction (CallFeature, CallLLM, CallService, etc.)
                    return Err(RuntimeError::InvalidOperation(format!(
                        "Unsupported instruction: {:?}",
                        instruction
                    )));
                }
            }
        }

        Ok(ctx.result)
    }

    /// Execute a binary operation
    fn execute_binary_op(left: &Value, op: &Operator, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => {
                let result = match op {
                    Operator::Add => a + b,
                    Operator::Sub => a - b,
                    Operator::Mul => a * b,
                    Operator::Div => {
                        if *b == 0.0 {
                            return Err(RuntimeError::InvalidOperation(
                                "Division by zero".to_string(),
                            ));
                        }
                        a / b
                    }
                    Operator::Mod => a % b,
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "Invalid operation {:?} for numbers",
                            op
                        )))
                    }
                };
                Ok(Value::Number(result))
            }

            (Value::Bool(a), Value::Bool(b)) => {
                let result = match op {
                    Operator::And => *a && *b,
                    Operator::Or => *a || *b,
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "Invalid operation {:?} for booleans",
                            op
                        )))
                    }
                };
                Ok(Value::Bool(result))
            }

            _ => Err(RuntimeError::TypeError(format!(
                "Type mismatch for operation {:?}",
                op
            ))),
        }
    }

    /// Execute a comparison operation
    fn execute_compare(left: &Value, op: &Operator, right: &Value) -> Result<bool> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => {
                let result = match op {
                    Operator::Eq => a == b,
                    Operator::Ne => a != b,
                    Operator::Lt => a < b,
                    Operator::Gt => a > b,
                    Operator::Le => a <= b,
                    Operator::Ge => a >= b,
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "Invalid comparison {:?}",
                            op
                        )))
                    }
                };
                Ok(result)
            }

            (Value::String(a), Value::String(b)) => {
                let result = match op {
                    Operator::Eq => a == b,
                    Operator::Ne => a != b,
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "Invalid string comparison {:?}",
                            op
                        )))
                    }
                };
                Ok(result)
            }

            (Value::Bool(a), Value::Bool(b)) => {
                let result = match op {
                    Operator::Eq => a == b,
                    Operator::Ne => a != b,
                    _ => {
                        return Err(RuntimeError::TypeError(format!(
                            "Invalid boolean comparison {:?}",
                            op
                        )))
                    }
                };
                Ok(result)
            }

            _ => Ok(left == right),
        }
    }

    /// Execute a unary operation
    fn execute_unary_op(operand: &Value, op: &UnaryOperator) -> Result<Value> {
        match (operand, op) {
            (Value::Bool(b), UnaryOperator::Not) => Ok(Value::Bool(!b)),
            (Value::Number(n), UnaryOperator::Negate) => Ok(Value::Number(-n)),
            _ => Err(RuntimeError::TypeError(format!(
                "Invalid unary operation {:?} for value type",
                op
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

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ir::ProgramMetadata;

    #[test]
    fn test_execute_load_const() {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::Number(42.0),
                },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, HashMap::new()).unwrap();
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_execute_load_field() {
        let mut event = HashMap::new();
        event.insert("amount".to_string(), Value::Number(1000.0));

        let program = Program::new(
            vec![
                Instruction::LoadField {
                    path: vec!["amount".to_string()],
                },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, event).unwrap();
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_execute_binary_add() {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::Number(10.0),
                },
                Instruction::LoadConst {
                    value: Value::Number(32.0),
                },
                Instruction::BinaryOp { op: Operator::Add },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, HashMap::new()).unwrap();
        // Result should be on the stack but we're just checking execution succeeded
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_execute_compare() {
        let mut event = HashMap::new();
        event.insert("age".to_string(), Value::Number(25.0));

        let program = Program::new(
            vec![
                Instruction::LoadField {
                    path: vec!["age".to_string()],
                },
                Instruction::LoadConst {
                    value: Value::Number(18.0),
                },
                Instruction::Compare { op: Operator::Gt },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, event).unwrap();
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_execute_set_score() {
        let program = Program::new(
            vec![Instruction::SetScore { value: 50 }, Instruction::Return],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, HashMap::new()).unwrap();
        assert_eq!(result.score, 50);
    }

    #[test]
    fn test_execute_mark_rule_triggered() {
        let program = Program::new(
            vec![
                Instruction::MarkRuleTriggered {
                    rule_id: "test_rule".to_string(),
                },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, HashMap::new()).unwrap();
        assert_eq!(result.triggered_rules.len(), 1);
        assert_eq!(result.triggered_rules[0], "test_rule");
    }

    #[test]
    fn test_execute_jump_if_false() {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::Bool(false),
                },
                Instruction::JumpIfFalse { offset: 2 }, // Skip next instruction
                Instruction::SetScore { value: 100 },   // This should be skipped
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, HashMap::new()).unwrap();
        assert_eq!(result.score, 0); // Score should not be set
    }

    #[test]
    fn test_execute_complete_rule() {
        // Simulates: if age > 18 then score = 50
        let mut event = HashMap::new();
        event.insert("age".to_string(), Value::Number(25.0));

        let program = Program::new(
            vec![
                // Load age
                Instruction::LoadField {
                    path: vec!["age".to_string()],
                },
                // Load 18
                Instruction::LoadConst {
                    value: Value::Number(18.0),
                },
                // Compare: age > 18
                Instruction::Compare { op: Operator::Gt },
                // If false, skip to end
                Instruction::JumpIfFalse { offset: 3 },
                // Set score
                Instruction::SetScore { value: 50 },
                // Mark rule triggered
                Instruction::MarkRuleTriggered {
                    rule_id: "age_check".to_string(),
                },
                // Return
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );

        let result = Executor::execute(&program, event).unwrap();
        assert_eq!(result.score, 50);
        assert_eq!(result.triggered_rules.len(), 1);
    }
}
