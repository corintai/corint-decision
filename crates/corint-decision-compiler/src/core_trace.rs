//! Bind normalized condition trees to existing code generation. No second evaluator.
use crate::codegen::ExpressionCompiler;
use crate::error::{CompileError, Result};
use corint_decision_model::ast::{Expression, UnaryOperator};
use corint_decision_model::ir::condition_map::{ConditionMap, ConditionNode};
use corint_decision_model::ir::Instruction;

pub(crate) fn condition_map(
    instructions: &[Instruction],
    expr: &Expression,
    end: usize,
    field_path: String,
) -> Result<ConditionMap> {
    let code = ExpressionCompiler::compile(expr)?;
    let start = end.checked_sub(code.len()).ok_or_else(mismatch)?;
    if instructions.get(start..end) != Some(code.as_slice()) {
        return Err(mismatch());
    }
    let mut nodes = Vec::new();
    locate(expr, start, "".into(), instructions, &mut nodes)?;
    Ok(ConditionMap { field_path, nodes })
}

fn mismatch() -> CompileError {
    CompileError::InvalidExpression(
        "Core condition source map does not match generated code".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_decision_model::ast::LogicalGroupOp;
    use corint_decision_model::Value;

    #[test]
    fn source_map_checks_generated_ranges_and_rejects_drift() {
        let expr = Expression::LogicalGroup {
            op: LogicalGroupOp::All,
            conditions: vec![
                Expression::literal(Value::Bool(false)),
                Expression::literal(Value::Bool(true)),
            ],
        };
        let code = ExpressionCompiler::compile(&expr).unwrap();
        let map = condition_map(&code, &expr, code.len(), "/rule/when".into()).unwrap();
        assert_eq!(map.nodes.len(), 3);
        assert_eq!((map.nodes[0].start, map.nodes[0].end), (0, code.len()));
        assert_eq!((map.nodes[1].start, map.nodes[1].end), (0, 1));
        assert_eq!((map.nodes[2].start, map.nodes[2].end), (2, 3));
        assert_eq!(map.nodes[2].node_path, "/children/1");
        let mut drifted = code.clone();
        drifted[0] = Instruction::Pop;
        assert!(condition_map(&drifted, &expr, code.len(), "/rule/when".into()).is_err());
        assert!(condition_map(&code, &expr, 0, "/rule/when".into()).is_err());
    }
}

fn locate(
    expr: &Expression,
    start: usize,
    node_path: String,
    instructions: &[Instruction],
    nodes: &mut Vec<ConditionNode>,
) -> Result<()> {
    let code = ExpressionCompiler::compile(expr)?;
    let end = start + code.len();
    if instructions.get(start..end) != Some(code.as_slice()) {
        return Err(mismatch());
    }
    nodes.push(ConditionNode {
        node_path: node_path.clone(),
        start,
        end,
    });
    match expr {
        Expression::LogicalGroup { conditions, .. } => {
            let mut offset = start;
            for (index, child) in conditions.iter().enumerate() {
                locate(
                    child,
                    offset,
                    format!("{node_path}/children/{index}"),
                    instructions,
                    nodes,
                )?;
                offset += ExpressionCompiler::compile(child)?.len();
                // The shared compiler places one conditional jump between children.
                if index + 1 < conditions.len() {
                    offset += 1;
                }
            }
        }
        Expression::Unary {
            op: UnaryOperator::Not,
            operand,
        } => {
            locate(
                operand,
                start,
                format!("{node_path}/children/0"),
                instructions,
                nodes,
            )?;
        }
        // Comparisons are atomic boolean conditions. Raw operands are not collected.
        _ => {}
    }
    Ok(())
}
