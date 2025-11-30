//! Ruleset compiler
//!
//! Compiles Ruleset AST nodes into IR programs.

use corint_core::ast::{Action, Ruleset};
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use crate::error::Result;
use crate::codegen::expression_codegen::ExpressionCompiler;

/// Ruleset compiler
pub struct RulesetCompiler;

impl RulesetCompiler {
    /// Compile a ruleset into an IR program
    pub fn compile(ruleset: &Ruleset) -> Result<Program> {
        let mut instructions = Vec::new();

        // Compile decision logic
        // This evaluates conditions and executes appropriate actions
        instructions.extend(Self::compile_decision_logic(ruleset)?);

        // Always end with return
        instructions.push(Instruction::Return);

        // Create program metadata
        let metadata = ProgramMetadata::for_ruleset(ruleset.id.clone());

        Ok(Program::new(instructions, metadata))
    }

    /// Compile decision logic
    ///
    /// Decision logic maps score ranges or conditions to actions
    fn compile_decision_logic(ruleset: &Ruleset) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Iterate through decision rules in order
        for (idx, decision_rule) in ruleset.decision_logic.iter().enumerate() {
            // Check if this is a default rule (no condition)
            if decision_rule.default {
                // Execute action directly
                instructions.extend(Self::compile_action(&decision_rule.action)?);

                if decision_rule.terminate {
                    instructions.push(Instruction::Return);
                }
                continue;
            }

            // If there's a condition, compile it
            if let Some(condition) = &decision_rule.condition {
                // Compile the condition expression
                instructions.extend(ExpressionCompiler::compile(condition)?);

                // Calculate the jump offset if condition is false
                // We need to count the instructions that will be executed if true
                let mut action_instructions = Self::compile_action(&decision_rule.action)?;
                if decision_rule.terminate {
                    action_instructions.push(Instruction::Return);
                } else {
                    // If not terminating, jump to the end after executing this action
                    // We need to calculate how many instructions remain after this branch
                    let remaining_rules = ruleset.decision_logic.len() - idx - 1;
                    if remaining_rules > 0 {
                        // Add a jump to skip the remaining decision logic
                        action_instructions.push(Instruction::Jump { offset: 999 }); // Placeholder, will be fixed in second pass
                    }
                }

                // Jump past the action instructions if condition is false
                let jump_offset = action_instructions.len() as isize + 1; // +1 for the JumpIfFalse itself
                instructions.push(Instruction::JumpIfFalse { offset: jump_offset });

                // Add the action instructions
                instructions.extend(action_instructions);
            }
        }

        // Second pass: fix the Jump instructions to skip to the end
        // Calculate positions and fix offsets
        let total_len = instructions.len() as isize;
        for (i, inst) in instructions.iter_mut().enumerate() {
            if let Instruction::Jump { offset } = inst {
                if *offset == 999 {
                    // Calculate offset to the end
                    *offset = total_len - i as isize;
                }
            }
        }

        Ok(instructions)
    }

    /// Compile an action into IR instructions
    fn compile_action(action: &Action) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        match action {
            Action::Approve => {
                // Set action to approve
                instructions.push(Instruction::SetAction {
                    action: Action::Approve,
                });
            }
            Action::Deny => {
                // Set action to deny
                instructions.push(Instruction::SetAction {
                    action: Action::Deny,
                });
            }
            Action::Review => {
                // Set action to review
                instructions.push(Instruction::SetAction {
                    action: Action::Review,
                });
            }
            Action::Infer { config } => {
                // Set action to infer with configuration
                instructions.push(Instruction::SetAction {
                    action: Action::Infer { config: config.clone() },
                });
            }
        }

        Ok(instructions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::DecisionRule;

    #[test]
    fn test_compile_simple_ruleset() {
        let ruleset = Ruleset {
            id: "test_ruleset".to_string(),
            name: Some("Test Ruleset".to_string()),
            rules: vec!["rule1".to_string(), "rule2".to_string()],
            decision_logic: vec![],
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        assert_eq!(program.metadata.source_type, "ruleset");
        assert_eq!(program.metadata.source_id, "test_ruleset");
    }

    #[test]
    fn test_compile_action_approve() {
        let instructions = RulesetCompiler::compile_action(&Action::Approve).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetAction { action: Action::Approve }
        ));
    }

    #[test]
    fn test_compile_action_deny() {
        let instructions = RulesetCompiler::compile_action(&Action::Deny).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetAction { action: Action::Deny }
        ));
    }

    #[test]
    fn test_compile_action_review() {
        let instructions = RulesetCompiler::compile_action(&Action::Review).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetAction { action: Action::Review }
        ));
    }

    #[test]
    fn test_compile_ruleset_with_decision_logic() {
        let ruleset = Ruleset {
            id: "decision_ruleset".to_string(),
            name: Some("Decision Ruleset".to_string()),
            rules: vec![],
            decision_logic: vec![
                DecisionRule {
                    condition: None,
                    default: true,
                    action: Action::Approve,
                    reason: Some("Default action".to_string()),
                    terminate: true,
                },
            ],
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        assert_eq!(program.metadata.source_type, "ruleset");
    }

    #[test]
    fn test_compile_ruleset_with_conditions() {
        use corint_core::ast::{Expression, Operator};
        use corint_core::Value;

        let ruleset = Ruleset {
            id: "test_conditions".to_string(),
            name: Some("Test Conditions".to_string()),
            rules: vec![],
            decision_logic: vec![
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec!["amount".to_string()])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(1000.0))),
                    }),
                    default: false,
                    action: Action::Review,
                    reason: Some("High amount".to_string()),
                    terminate: false,
                },
                DecisionRule {
                    condition: None,
                    default: true,
                    action: Action::Approve,
                    reason: None,
                    terminate: false,
                },
            ],
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        // Print instructions for debugging
        println!("Compiled instructions:");
        for (i, inst) in program.instructions.iter().enumerate() {
            println!("{}: {:?}", i, inst);
        }

        // Should have instructions for condition check, action setting, and default action
        assert!(program.instructions.len() > 3);
    }

    #[test]
    fn test_compile_fraud_detection() {
        use corint_core::ast::{Expression, Operator};
        use corint_core::Value;

        let ruleset = Ruleset {
            id: "fraud_detection".to_string(),
            name: Some("Fraud Detection".to_string()),
            rules: vec![],
            decision_logic: vec![
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec!["transaction_amount".to_string()])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(10000.0))),
                    }),
                    default: false,
                    action: Action::Deny,
                    reason: Some("Extremely high value".to_string()),
                    terminate: true,
                },
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec!["transaction_amount".to_string()])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(1000.0))),
                    }),
                    default: false,
                    action: Action::Review,
                    reason: Some("High value".to_string()),
                    terminate: false,
                },
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec!["transaction_amount".to_string()])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(100.0))),
                    }),
                    default: false,
                    action: Action::Review,
                    reason: Some("Elevated amount".to_string()),
                    terminate: false,
                },
                DecisionRule {
                    condition: None,
                    default: true,
                    action: Action::Approve,
                    reason: None,
                    terminate: false,
                },
            ],
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        println!("\nFraud Detection Compiled Instructions:");
        for (i, inst) in program.instructions.iter().enumerate() {
            println!("{}: {:?}", i, inst);
        }

        assert!(program.instructions.len() > 10);
    }
}
