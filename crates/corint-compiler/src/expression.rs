//! Expression compiler
//!
//! Compiles Expression AST nodes into IR instructions.

use corint_core::ast::{Expression, Operator, UnaryOperator};
use corint_core::ir::Instruction;
use crate::error::{CompileError, Result};

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
                Ok(vec![Instruction::LoadField {
                    path: path.clone(),
                }])
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

            Expression::FunctionCall { name, args } => {
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
        }
    }

    /// Check if operator is a comparison operator
    fn is_comparison_op(op: &Operator) -> bool {
        matches!(
            op,
            Operator::Eq
                | Operator::Ne
                | Operator::Lt
                | Operator::Gt
                | Operator::Le
                | Operator::Ge
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
        assert!(matches!(
            instructions[0],
            Instruction::LoadConst { .. }
        ));
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
}
