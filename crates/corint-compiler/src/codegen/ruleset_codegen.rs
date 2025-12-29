//! Ruleset compiler
//!
//! Compiles Ruleset AST nodes into IR programs.

use crate::codegen::expression_codegen::ExpressionCompiler;
use crate::error::Result;
use corint_core::ast::{Expression, Ruleset, Signal};
use corint_core::ir::{Instruction, Program, ProgramMetadata};

/// Ruleset compiler
pub struct RulesetCompiler;

impl RulesetCompiler {
    /// Compile a ruleset into an IR program
    pub fn compile(ruleset: &Ruleset) -> Result<Program> {
        let mut instructions = Vec::new();

        // Compile decision logic
        // This evaluates conditions and executes appropriate actions
        instructions.extend(Self::compile_conclusion(ruleset)?);

        // Always end with return
        instructions.push(Instruction::Return);

        // Create program metadata and store the list of rules
        let mut metadata = ProgramMetadata::for_ruleset(ruleset.id.clone());

        // Store the rule IDs in custom metadata so DecisionEngine can execute them first
        if !ruleset.rules.is_empty() {
            metadata
                .custom
                .insert("rules".to_string(), ruleset.rules.join(","));
        }

        // Store conclusion as JSON for trace building
        if !ruleset.conclusion.is_empty() {
            let conclusion_json = Self::conclusion_to_json(&ruleset.conclusion);
            metadata
                .custom
                .insert("conclusion_json".to_string(), conclusion_json);
        }

        Ok(Program::new(instructions, metadata))
    }

    /// Compile decision logic
    ///
    /// Decision logic maps score ranges or conditions to signals
    fn compile_conclusion(ruleset: &Ruleset) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Iterate through decision rules in order
        for (idx, decision_rule) in ruleset.conclusion.iter().enumerate() {
            // Check if this is a default rule (no condition)
            if decision_rule.default {
                // Execute signal directly
                instructions.extend(Self::compile_signal(&decision_rule.signal)?);
                continue;
            }

            // If there's a condition, compile it
            if let Some(condition) = &decision_rule.condition {
                // Compile the condition expression
                instructions.extend(ExpressionCompiler::compile(condition)?);

                // Calculate the jump offset if condition is false
                // We need to count the instructions that will be executed if true
                let mut signal_instructions = Self::compile_signal(&decision_rule.signal)?;
                // Jump to the end after executing this signal if there are remaining rules
                let remaining_rules = ruleset.conclusion.len() - idx - 1;
                if remaining_rules > 0 {
                    // Add a jump to skip the remaining decision logic
                    signal_instructions.push(Instruction::Jump { offset: 999 });
                    // Placeholder, will be fixed in second pass
                }

                // Jump past the signal instructions if condition is false
                // Offset is relative to current instruction, so we need to skip:
                // +1 to get past the JumpIfFalse itself, then skip all signal_instructions
                let jump_offset = (signal_instructions.len() + 1) as isize;
                instructions.push(Instruction::JumpIfFalse {
                    offset: jump_offset,
                });

                // Add the signal instructions
                instructions.extend(signal_instructions);
            }
        }

        // Second pass: fix the Jump instructions to skip to the end
        // Calculate positions and fix offsets
        let total_len = instructions.len() as isize;
        for (i, inst) in instructions.iter_mut().enumerate() {
            if let Instruction::Jump { offset } = inst {
                if *offset == 999 {
                    // Calculate offset to jump past all remaining instructions
                    // The offset is relative to the CURRENT instruction position i
                    // To jump past the last instruction at position (total_len - 1),
                    // we need to reach position total_len
                    // Offset = total_len - i
                    // But since we want to exit the loop (pc >= total_len), we use:
                    *offset = total_len - i as isize;
                }
            }
        }

        Ok(instructions)
    }

    /// Compile a signal into IR instructions
    fn compile_signal(signal: &Signal) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        match signal {
            Signal::Approve => {
                // Set signal to approve
                instructions.push(Instruction::SetSignal {
                    signal: Signal::Approve,
                });
            }
            Signal::Decline => {
                // Set signal to decline
                instructions.push(Instruction::SetSignal {
                    signal: Signal::Decline,
                });
            }
            Signal::Review => {
                // Set signal to review
                instructions.push(Instruction::SetSignal {
                    signal: Signal::Review,
                });
            }
            Signal::Hold => {
                // Set signal to hold (temporarily suspend, require additional verification)
                instructions.push(Instruction::SetSignal {
                    signal: Signal::Hold,
                });
            }
            Signal::Pass => {
                // Set signal to pass (skip/no decision)
                instructions.push(Instruction::SetSignal {
                    signal: Signal::Pass,
                });
            }
        }

        Ok(instructions)
    }

    /// Convert conclusion to JSON for trace building
    fn conclusion_to_json(conclusion: &[corint_core::ast::DecisionRule]) -> String {
        let json_array: Vec<serde_json::Value> = conclusion
            .iter()
            .map(|rule| {
                let mut obj = serde_json::Map::new();

                // Add condition expression as string
                if let Some(ref condition) = rule.condition {
                    obj.insert(
                        "condition".to_string(),
                        serde_json::Value::String(Self::expression_to_readable_string(condition)),
                    );
                }

                // Add default flag
                obj.insert("default".to_string(), serde_json::Value::Bool(rule.default));

                // Add signal (the decision result)
                let signal_str = match &rule.signal {
                    Signal::Approve => "APPROVE",
                    Signal::Decline => "DECLINE",
                    Signal::Review => "REVIEW",
                    Signal::Hold => "HOLD",
                    Signal::Pass => "PASS",
                };
                obj.insert(
                    "signal".to_string(),
                    serde_json::Value::String(signal_str.to_string()),
                );

                // Add actions (user-defined actions)
                if !rule.actions.is_empty() {
                    obj.insert(
                        "actions".to_string(),
                        serde_json::Value::Array(
                            rule.actions
                                .iter()
                                .map(|a| serde_json::Value::String(a.clone()))
                                .collect(),
                        ),
                    );
                }

                // Add reason if present
                if let Some(ref reason) = rule.reason {
                    obj.insert(
                        "reason".to_string(),
                        serde_json::Value::String(reason.clone()),
                    );
                }

                serde_json::Value::Object(obj)
            })
            .collect();

        serde_json::to_string(&json_array).unwrap_or_else(|_| "[]".to_string())
    }

    /// Convert an Expression to a readable string representation
    fn expression_to_readable_string(expr: &Expression) -> String {
        use corint_core::ast::{Operator, UnaryOperator};
        use corint_core::Value;

        match expr {
            Expression::Literal(value) => match value {
                Value::Number(n) => n.to_string(),
                Value::String(s) => format!("\"{}\"", s),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                Value::Array(arr) => format!(
                    "[{}]",
                    arr.iter()
                        .map(|v| match v {
                            Value::String(s) => format!("\"{}\"", s),
                            Value::Number(n) => n.to_string(),
                            _ => format!("{:?}", v),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Value::Object(_) => "{...}".to_string(),
            },
            Expression::FieldAccess(path) => path.join("."),
            Expression::Binary { left, op, right } => {
                let op_str = match op {
                    Operator::Eq => "==",
                    Operator::Ne => "!=",
                    Operator::Lt => "<",
                    Operator::Le => "<=",
                    Operator::Gt => ">",
                    Operator::Ge => ">=",
                    Operator::And => "&&",
                    Operator::Or => "||",
                    Operator::Add => "+",
                    Operator::Sub => "-",
                    Operator::Mul => "*",
                    Operator::Div => "/",
                    Operator::Mod => "%",
                    Operator::In => "in",
                    Operator::NotIn => "not in",
                    Operator::Contains => "contains",
                    Operator::StartsWith => "starts_with",
                    Operator::EndsWith => "ends_with",
                    Operator::Regex => "=~",
                    Operator::InList => "in list",
                    Operator::NotInList => "not in list",
                };
                format!(
                    "{} {} {}",
                    Self::expression_to_readable_string(left),
                    op_str,
                    Self::expression_to_readable_string(right)
                )
            }
            Expression::Unary { op, operand } => {
                let op_str = match op {
                    UnaryOperator::Not => "!",
                    UnaryOperator::Negate => "-",
                };
                format!("{}{}", op_str, Self::expression_to_readable_string(operand))
            }
            Expression::FunctionCall { name, args } => {
                let args_str = args
                    .iter()
                    .map(|a| Self::expression_to_readable_string(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                format!(
                    "{} ? {} : {}",
                    Self::expression_to_readable_string(condition),
                    Self::expression_to_readable_string(true_expr),
                    Self::expression_to_readable_string(false_expr)
                )
            }
            Expression::LogicalGroup { op, conditions } => {
                use corint_core::ast::LogicalGroupOp;
                let op_str = match op {
                    LogicalGroupOp::Any => "any",
                    LogicalGroupOp::All => "all",
                };
                let conditions_str = conditions
                    .iter()
                    .map(|c| Self::expression_to_readable_string(c))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}:[{}]", op_str, conditions_str)
            }
            Expression::ListReference { list_id } => format!("list.{}", list_id),
            Expression::ResultAccess { ruleset_id, field } => match ruleset_id {
                Some(id) => format!("result.{}.{}", id, field),
                None => format!("result.{}", field),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::DecisionRule;

    #[test]
    fn test_compile_simple_ruleset() {
        let ruleset = Ruleset::new("test_ruleset".to_string())
            .with_name("Test Ruleset".to_string())
            .with_rules(vec!["rule1".to_string(), "rule2".to_string()]);

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        assert_eq!(program.metadata.source_type, "ruleset");
        assert_eq!(program.metadata.source_id, "test_ruleset");
    }

    #[test]
    fn test_compile_signal_approve() {
        let instructions = RulesetCompiler::compile_signal(&Signal::Approve).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetSignal {
                signal: Signal::Approve
            }
        ));
    }

    #[test]
    fn test_compile_signal_decline() {
        let instructions = RulesetCompiler::compile_signal(&Signal::Decline).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetSignal {
                signal: Signal::Decline
            }
        ));
    }

    #[test]
    fn test_compile_signal_hold() {
        let instructions = RulesetCompiler::compile_signal(&Signal::Hold).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetSignal {
                signal: Signal::Hold
            }
        ));
    }

    #[test]
    fn test_compile_signal_pass() {
        let instructions = RulesetCompiler::compile_signal(&Signal::Pass).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetSignal {
                signal: Signal::Pass
            }
        ));
    }

    #[test]
    fn test_compile_signal_review() {
        let instructions = RulesetCompiler::compile_signal(&Signal::Review).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::SetSignal {
                signal: Signal::Review
            }
        ));
    }

    #[test]
    fn test_compile_ruleset_with_conclusion() {
        let ruleset = Ruleset {
            id: "decision_ruleset".to_string(),
            name: Some("Decision Ruleset".to_string()),
            extends: None,
            rules: vec![],
            conclusion: vec![DecisionRule {
                condition: None,
                default: true,
                signal: Signal::Approve,
                actions: vec![],
                reason: Some("Default signal".to_string()),
            }],
            description: None,
            metadata: None,
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
            extends: None,
            rules: vec![],
            conclusion: vec![
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec!["amount".to_string()])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(1000.0))),
                    }),
                    default: false,
                    signal: Signal::Review,
                    actions: vec!["KYC_AUTH".to_string()],
                    reason: Some("High amount".to_string()),
                },
                DecisionRule {
                    condition: None,
                    default: true,
                    signal: Signal::Approve,
                    actions: vec![],
                    reason: None,
                },
            ],
            description: None,
            metadata: None,
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        // Print instructions for debugging
        println!("Compiled instructions:");
        for (i, inst) in program.instructions.iter().enumerate() {
            println!("{}: {:?}", i, inst);
        }

        // Should have instructions for condition check, signal setting, and default signal
        assert!(program.instructions.len() > 3);
    }

    #[test]
    fn test_compile_fraud_detection() {
        use corint_core::ast::{Expression, Operator};
        use corint_core::Value;

        let ruleset = Ruleset {
            id: "fraud_detection".to_string(),
            name: Some("Fraud Detection".to_string()),
            extends: None,
            rules: vec![],
            conclusion: vec![
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec![
                            "transaction_amount".to_string()
                        ])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(10000.0))),
                    }),
                    default: false,
                    signal: Signal::Decline,
                    actions: vec!["BLOCK_CARD".to_string()],
                    reason: Some("Extremely high value".to_string()),
                },
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec![
                            "transaction_amount".to_string()
                        ])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(1000.0))),
                    }),
                    default: false,
                    signal: Signal::Review,
                    actions: vec!["KYC_AUTH".to_string()],
                    reason: Some("High value".to_string()),
                },
                DecisionRule {
                    condition: Some(Expression::Binary {
                        left: Box::new(Expression::FieldAccess(vec![
                            "transaction_amount".to_string()
                        ])),
                        op: Operator::Gt,
                        right: Box::new(Expression::Literal(Value::Number(100.0))),
                    }),
                    default: false,
                    signal: Signal::Review,
                    actions: vec![],
                    reason: Some("Elevated amount".to_string()),
                },
                DecisionRule {
                    condition: None,
                    default: true,
                    signal: Signal::Approve,
                    actions: vec![],
                    reason: None,
                },
            ],
            description: None,
            metadata: None,
        };

        let program = RulesetCompiler::compile(&ruleset).unwrap();

        println!("\nFraud Detection Compiled Instructions:");
        for (i, inst) in program.instructions.iter().enumerate() {
            println!("{}: {:?}", i, inst);
        }

        assert!(program.instructions.len() > 10);
    }
}
#[test]
fn test_compile_simple_ruleset_outside() {
    use crate::codegen::RulesetCompiler;
    use corint_core::ast::Signal;
    use corint_core::ast::{DecisionRule, Expression, Operator, Ruleset};
    use corint_core::Value;

    let ruleset = Ruleset {
        id: "test_ruleset".to_string(),
        name: Some("Test Ruleset".to_string()),
        extends: None,
        rules: vec!["test_rule".to_string()],
        conclusion: vec![
            DecisionRule {
                condition: Some(Expression::Binary {
                    left: Box::new(Expression::FieldAccess(vec!["total_score".to_string()])),
                    op: Operator::Ge,
                    right: Box::new(Expression::Literal(Value::Number(50.0))),
                }),
                default: false,
                signal: Signal::Decline,
                actions: vec![],
                reason: None,
            },
            DecisionRule {
                condition: None,
                default: true,
                signal: Signal::Approve,
                actions: vec![],
                reason: None,
            },
        ],
        description: None,
        metadata: None,
    };

    let program = RulesetCompiler::compile(&ruleset).unwrap();

    println!("\nTest Simple Ruleset Instructions:");
    for (i, inst) in program.instructions.iter().enumerate() {
        println!("{}: {:?}", i, inst);
    }
}
