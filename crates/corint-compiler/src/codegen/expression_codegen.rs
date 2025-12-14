//! Expression compiler
//!
//! Compiles Expression AST nodes into IR instructions.

use crate::error::{CompileError, Result};
#[cfg(test)]
use corint_core::ast::UnaryOperator;
use corint_core::ast::{Expression, LogicalGroupOp, Operator};
use corint_core::ir::Instruction;

/// Expression compiler
pub struct ExpressionCompiler;

impl ExpressionCompiler {
    /// Compile an expression into IR instructions
    pub fn compile(expr: &Expression) -> Result<Vec<Instruction>> {
        match expr {
            Expression::Literal(value) => {
                // Load constant onto stack
                Ok(vec![Instruction::LoadConst {
                    value: value.clone(),
                }])
            }

            Expression::FieldAccess(path) => {
                // Load field value onto stack
                Ok(vec![Instruction::LoadField { path: path.clone() }])
            }

            Expression::Binary { left, op, right } => {
                let mut instructions = Vec::new();

                // Compile left operand
                instructions.extend(Self::compile(left)?);

                // Compile right operand
                instructions.extend(Self::compile(right)?);

                // Add operation instruction
                if Self::is_comparison_op(op) {
                    instructions.push(Instruction::Compare { op: *op });
                } else {
                    instructions.push(Instruction::BinaryOp { op: *op });
                }

                Ok(instructions)
            }

            Expression::Unary { op, operand } => {
                let mut instructions = Vec::new();

                // Compile operand
                instructions.extend(Self::compile(operand)?);

                // Add unary operation
                instructions.push(Instruction::UnaryOp { op: *op });

                Ok(instructions)
            }

            Expression::FunctionCall { name, args: _ } => {
                // For now, we'll handle function calls as a placeholder
                // In a real implementation, this would analyze the function
                // and generate appropriate CallFeature or other instructions
                Err(CompileError::UnsupportedFeature(format!(
                    "Function call compilation not yet implemented: {}",
                    name
                )))
            }

            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                let mut instructions = Vec::new();

                // Compile condition
                instructions.extend(Self::compile(condition)?);

                // Compile true branch
                let true_instructions = Self::compile(true_expr)?;
                let false_instructions = Self::compile(false_expr)?;

                // JumpIfFalse to false branch
                instructions.push(Instruction::JumpIfFalse {
                    offset: (true_instructions.len() + 1) as isize,
                });

                // True branch
                instructions.extend(true_instructions);

                // Jump over false branch
                instructions.push(Instruction::Jump {
                    offset: false_instructions.len() as isize,
                });

                // False branch
                instructions.extend(false_instructions);

                Ok(instructions)
            }

            Expression::LogicalGroup { op, conditions } => {
                match op {
                    LogicalGroupOp::Any => Self::compile_any_conditions(conditions),
                    LogicalGroupOp::All => Self::compile_all_conditions(conditions),
                }
            }
        }
    }

    /// Compile 'any' logical group (OR logic with short-circuit evaluation)
    /// Returns true if ANY condition is true
    ///
    /// Stack management strategy (CORRECTED):
    /// - JumpIfTrue/JumpIfFalse POP the value from stack
    /// - After jump, we need a result value on stack
    /// - Solution: Before jumping when true, duplicate and use conditional jump
    ///
    /// For [A, B, C] (any logic):
    /// - Eval A (pushes result)
    /// - Dup (duplicate it)
    /// - JumpIfTrue to "push_true" section (pops duplicated value, leaves original)
    /// - Pop (remove the false value since we didn't jump)
    /// - Eval B
    /// - Dup
    /// - JumpIfTrue to "push_true"
    /// - Pop
    /// - Eval C (final result)
    /// - Jump to end
    /// - push_true: LoadConst(true)
    /// - end:
    ///
    /// Wait, this still doesn't work because after JumpIfTrue, the original is still there...
    ///
    /// Let me reconsider: the REAL solution is simpler:
    /// - Don't use Dup at all
    /// - Just use JumpIfTrue which consumes the value
    /// - Before the jump destination, push TRUE
    fn compile_any_conditions(conditions: &[Expression]) -> Result<Vec<Instruction>> {
        if conditions.is_empty() {
            // Empty 'any' evaluates to false
            return Ok(vec![Instruction::LoadConst {
                value: corint_core::Value::Bool(false),
            }]);
        }

        if conditions.len() == 1 {
            // Single condition - just compile it directly
            return Self::compile(&conditions[0]);
        }

        let mut instructions = Vec::new();
        let mut jump_to_true_offsets = Vec::new();

        // Process all conditions except the last
        for condition in conditions.iter().take(conditions.len() - 1) {
            // Compile this condition (pushes result onto stack)
            instructions.extend(Self::compile(condition)?);

            // If true, we want to return true immediately (short-circuit)
            // JumpIfTrue will POP the value
            jump_to_true_offsets.push(instructions.len());
            instructions.push(Instruction::JumpIfTrue { offset: 0 });

            // If we get here, the condition was false and value was popped
            // Continue to next condition
        }

        // Compile the last condition - its result is the final answer if we reach here
        instructions.extend(Self::compile(&conditions[conditions.len() - 1])?);

        // Jump over the "return true" section (skip LoadConst instruction)
        // Jump { offset: 2 } means: pc = current_pc + 2, skipping the next instruction
        instructions.push(Instruction::Jump { offset: 2 });

        // This is where we land if any condition was true
        // We need to push true onto the stack
        let true_label_pos = instructions.len();
        instructions.push(Instruction::LoadConst {
            value: corint_core::Value::Bool(true),
        });

        // Fix all "jump to true" offsets
        // JumpIfTrue at position X should jump to true_label_pos
        // offset = true_label_pos - X (since pc = X + offset after jump)
        for jump_pos in jump_to_true_offsets {
            let offset = (true_label_pos - jump_pos) as isize;
            if let Instruction::JumpIfTrue { offset: ref mut o } = instructions[jump_pos] {
                *o = offset;
            }
        }

        // End - result is on stack (either last condition's result, or the true we pushed)

        Ok(instructions)
    }

    /// Compile 'all' logical group (AND logic with short-circuit evaluation)
    /// Returns true if ALL conditions are true
    ///
    /// Similar to 'any' but inverted:
    /// - If ANY condition is false, return false (short-circuit)
    /// - If all are true, return the last condition's result (which is true)
    ///
    /// For [A, B, C]:
    /// - Eval A
    /// - JumpIfFalse to "push_false"
    /// - Eval B
    /// - JumpIfFalse to "push_false"
    /// - Eval C (final result if we get here, must be checking true path)
    /// - Jump to end
    /// - push_false: LoadConst(false)
    /// - end:
    fn compile_all_conditions(conditions: &[Expression]) -> Result<Vec<Instruction>> {
        if conditions.is_empty() {
            // Empty 'all' evaluates to true
            return Ok(vec![Instruction::LoadConst {
                value: corint_core::Value::Bool(true),
            }]);
        }

        if conditions.len() == 1 {
            // Single condition - just compile it directly
            return Self::compile(&conditions[0]);
        }

        let mut instructions = Vec::new();
        let mut jump_to_false_offsets = Vec::new();

        // Process all conditions except the last
        for condition in conditions.iter().take(conditions.len() - 1) {
            // Compile this condition (pushes result onto stack)
            instructions.extend(Self::compile(condition)?);

            // If false, we want to return false immediately (short-circuit)
            // JumpIfFalse will POP the value
            jump_to_false_offsets.push(instructions.len());
            instructions.push(Instruction::JumpIfFalse { offset: 0 });

            // If we get here, the condition was true and value was popped
            // Continue to next condition
        }

        // Compile the last condition - its result is the final answer if we reach here
        instructions.extend(Self::compile(&conditions[conditions.len() - 1])?);

        // Jump over the "return false" section (skip LoadConst instruction)
        // Jump { offset: 2 } means: pc = current_pc + 2, skipping the next instruction
        instructions.push(Instruction::Jump { offset: 2 });

        // This is where we land if any condition was false
        // We need to push false onto the stack
        let false_label_pos = instructions.len();
        instructions.push(Instruction::LoadConst {
            value: corint_core::Value::Bool(false),
        });

        // Fix all "jump to false" offsets
        // JumpIfFalse at position X should jump to false_label_pos
        // offset = false_label_pos - X (since pc = X + offset after jump)
        for jump_pos in jump_to_false_offsets {
            let offset = (false_label_pos - jump_pos) as isize;
            if let Instruction::JumpIfFalse { offset: ref mut o } = instructions[jump_pos] {
                *o = offset;
            }
        }

        // End - result is on stack (either last condition's result, or the false we pushed)

        Ok(instructions)
    }

    /// Check if operator is a comparison operator
    fn is_comparison_op(op: &Operator) -> bool {
        matches!(
            op,
            Operator::Eq | Operator::Ne | Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::Value;

    #[test]
    fn test_compile_literal() {
        let expr = Expression::literal(Value::Number(42.0));
        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::LoadConst { .. }));
    }

    #[test]
    fn test_compile_field_access() {
        let expr = Expression::field_access(vec!["user".to_string(), "age".to_string()]);
        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 1);
        if let Instruction::LoadField { path } = &instructions[0] {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0], "user");
            assert_eq!(path[1], "age");
        } else {
            panic!("Expected LoadField instruction");
        }
    }

    #[test]
    fn test_compile_binary_comparison() {
        // user.age > 18
        let expr = Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(18.0)),
        );

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[0], Instruction::LoadField { .. }));
        assert!(matches!(instructions[1], Instruction::LoadConst { .. }));
        assert!(matches!(instructions[2], Instruction::Compare { .. }));
    }

    #[test]
    fn test_compile_binary_arithmetic() {
        // a + b
        let expr = Expression::binary(
            Expression::field_access(vec!["a".to_string()]),
            Operator::Add,
            Expression::field_access(vec!["b".to_string()]),
        );

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[0], Instruction::LoadField { .. }));
        assert!(matches!(instructions[1], Instruction::LoadField { .. }));
        assert!(matches!(instructions[2], Instruction::BinaryOp { .. }));
    }

    #[test]
    fn test_compile_unary() {
        // !user.active
        let expr = Expression::Unary {
            op: UnaryOperator::Not,
            operand: Box::new(Expression::field_access(vec![
                "user".to_string(),
                "active".to_string(),
            ])),
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 2);
        assert!(matches!(instructions[0], Instruction::LoadField { .. }));
        assert!(matches!(instructions[1], Instruction::UnaryOp { .. }));
    }

    #[test]
    fn test_compile_complex_expression() {
        // (a + b) > c
        let expr = Expression::binary(
            Expression::binary(
                Expression::field_access(vec!["a".to_string()]),
                Operator::Add,
                Expression::field_access(vec!["b".to_string()]),
            ),
            Operator::Gt,
            Expression::field_access(vec!["c".to_string()]),
        );

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        // LoadField(a), LoadField(b), BinaryOp(+), LoadField(c), Compare(>)
        assert_eq!(instructions.len(), 5);
    }

    #[test]
    fn test_compile_logical_and() {
        // a && b
        let expr = Expression::binary(
            Expression::field_access(vec!["a".to_string()]),
            Operator::And,
            Expression::field_access(vec!["b".to_string()]),
        );

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[2], Instruction::BinaryOp { .. }));
    }

    #[test]
    fn test_compile_any_logical_group_empty() {
        // any: [] should evaluate to false
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::Any,
            conditions: vec![],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::LoadConst {
                value: Value::Bool(false)
            }
        ));
    }

    #[test]
    fn test_compile_any_logical_group_single() {
        // any: [a > 5] should compile to just the condition
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::Any,
            conditions: vec![Expression::binary(
                Expression::field_access(vec!["a".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(5.0)),
            )],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        // Should be: LoadField(a), LoadConst(5), Compare(>)
        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[0], Instruction::LoadField { .. }));
        assert!(matches!(instructions[1], Instruction::LoadConst { .. }));
        assert!(matches!(instructions[2], Instruction::Compare { .. }));
    }

    #[test]
    fn test_compile_any_logical_group_multiple() {
        // any: [a > 5, b < 10]
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::Any,
            conditions: vec![
                Expression::binary(
                    Expression::field_access(vec!["a".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(5.0)),
                ),
                Expression::binary(
                    Expression::field_access(vec!["b".to_string()]),
                    Operator::Lt,
                    Expression::literal(Value::Number(10.0)),
                ),
            ],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        // Expected instructions:
        // - LoadField(a), LoadConst(5), Compare(>)  [3 instructions]
        // - JumpIfTrue to "push_true"               [1 instruction]
        // - LoadField(b), LoadConst(10), Compare(<) [3 instructions]
        // - Jump over "push_true"                   [1 instruction]
        // - LoadConst(true)                         [1 instruction at "push_true"]
        // Total: 9 instructions

        assert_eq!(instructions.len(), 9);

        // Verify structure
        assert!(matches!(instructions[0], Instruction::LoadField { .. })); // a
        assert!(matches!(instructions[1], Instruction::LoadConst { .. })); // 5
        assert!(matches!(instructions[2], Instruction::Compare { .. })); // >
        assert!(matches!(instructions[3], Instruction::JumpIfTrue { .. })); // jump if a > 5

        assert!(matches!(instructions[4], Instruction::LoadField { .. })); // b
        assert!(matches!(instructions[5], Instruction::LoadConst { .. })); // 10
        assert!(matches!(instructions[6], Instruction::Compare { .. })); // <
        assert!(matches!(instructions[7], Instruction::Jump { .. })); // jump to end

        assert!(matches!(
            instructions[8],
            Instruction::LoadConst {
                value: Value::Bool(true)
            }
        )); // push true
    }

    #[test]
    fn test_compile_all_logical_group_empty() {
        // all: [] should evaluate to true
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::All,
            conditions: vec![],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::LoadConst {
                value: Value::Bool(true)
            }
        ));
    }

    #[test]
    fn test_compile_all_logical_group_single() {
        // all: [a > 5] should compile to just the condition
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::All,
            conditions: vec![Expression::binary(
                Expression::field_access(vec!["a".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(5.0)),
            )],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        // Should be: LoadField(a), LoadConst(5), Compare(>)
        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[0], Instruction::LoadField { .. }));
        assert!(matches!(instructions[1], Instruction::LoadConst { .. }));
        assert!(matches!(instructions[2], Instruction::Compare { .. }));
    }

    #[test]
    fn test_compile_all_logical_group_multiple() {
        // all: [a > 5, b < 10]
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::All,
            conditions: vec![
                Expression::binary(
                    Expression::field_access(vec!["a".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(5.0)),
                ),
                Expression::binary(
                    Expression::field_access(vec!["b".to_string()]),
                    Operator::Lt,
                    Expression::literal(Value::Number(10.0)),
                ),
            ],
        };

        let instructions = ExpressionCompiler::compile(&expr).unwrap();

        // Expected instructions:
        // - LoadField(a), LoadConst(5), Compare(>)   [3 instructions]
        // - JumpIfFalse to "push_false"              [1 instruction]
        // - LoadField(b), LoadConst(10), Compare(<)  [3 instructions]
        // - Jump over "push_false"                   [1 instruction]
        // - LoadConst(false)                         [1 instruction at "push_false"]
        // Total: 9 instructions

        assert_eq!(instructions.len(), 9);

        // Verify structure
        assert!(matches!(instructions[0], Instruction::LoadField { .. })); // a
        assert!(matches!(instructions[1], Instruction::LoadConst { .. })); // 5
        assert!(matches!(instructions[2], Instruction::Compare { .. })); // >
        assert!(matches!(instructions[3], Instruction::JumpIfFalse { .. })); // jump if a <= 5

        assert!(matches!(instructions[4], Instruction::LoadField { .. })); // b
        assert!(matches!(instructions[5], Instruction::LoadConst { .. })); // 10
        assert!(matches!(instructions[6], Instruction::Compare { .. })); // <
        assert!(matches!(instructions[7], Instruction::Jump { .. })); // jump to end

        assert!(matches!(
            instructions[8],
            Instruction::LoadConst {
                value: Value::Bool(false)
            }
        )); // push false
    }
}
