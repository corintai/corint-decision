//! Rule compiler
//!
//! Compiles Rule AST nodes into IR programs.

use corint_core::ast::Rule;
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use crate::error::Result;
use super::expression_codegen::ExpressionCompiler;

/// Rule compiler
pub struct RuleCompiler;

impl RuleCompiler {
    /// Compile a rule into an IR program
    pub fn compile(rule: &Rule) -> Result<Program> {
        let mut instructions = Vec::new();

        // 1. Check event type if specified
        if let Some(event_type) = &rule.when.event_type {
            instructions.push(Instruction::CheckEventType {
                expected: event_type.clone(),
            });
        }

        // 2. Compile all conditions
        // First, compile all condition expressions and calculate total instruction count
        let mut condition_blocks = Vec::new();
        for condition in &rule.when.conditions {
            let condition_instructions = ExpressionCompiler::compile(condition)?;
            condition_blocks.push(condition_instructions);
        }

        // Add conditions with correct jump offsets
        for (idx, condition_instructions) in condition_blocks.iter().enumerate() {
            instructions.extend(condition_instructions.clone());

            // Calculate how many instructions to skip if this condition is false:
            // - All remaining condition blocks (each with its own JumpIfFalse)
            // - SetScore instruction (1)
            // - MarkRuleTriggered instruction (1)
            // - Return instruction (1)
            let mut remaining_instruction_count = 3; // SetScore + MarkRuleTriggered + Return

            for remaining_block in &condition_blocks[idx + 1..] {
                remaining_instruction_count += remaining_block.len() + 1; // +1 for JumpIfFalse
            }

            instructions.push(Instruction::JumpIfFalse {
                offset: remaining_instruction_count as isize
            });
        }

        // 3. If all conditions passed, add the score (累加分数而不是设置分数)
        instructions.push(Instruction::AddScore { value: rule.score });

        // 4. Mark this rule as triggered
        instructions.push(Instruction::MarkRuleTriggered {
            rule_id: rule.id.clone(),
        });

        // 5. Return
        instructions.push(Instruction::Return);

        // Create program metadata
        let metadata = ProgramMetadata::for_rule(rule.id.clone())
            .with_name(rule.name.clone());

        let metadata = if let Some(desc) = &rule.description {
            metadata.with_description(desc.clone())
        } else {
            metadata
        };

        Ok(Program::new(instructions, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::{Expression, Operator, WhenBlock};
    use corint_core::Value;

    #[test]
    fn test_compile_simple_rule() {
        // Create a simple rule: user.age > 18
        let when = WhenBlock::new()
            .with_event_type("login".to_string())
            .add_condition(Expression::binary(
                Expression::field_access(vec!["user".to_string(), "age".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(18.0)),
            ));

        let rule = Rule {
            id: "age_check".to_string(),
            name: "Age Check".to_string(),
            description: Some("Check if user is over 18".to_string()),
            when,
            score: 50,
            params: None,
            metadata: None,
        };

        let program = RuleCompiler::compile(&rule).unwrap();

        // Should have:
        // 1. CheckEventType
        // 2-4. Condition (LoadField, LoadConst, Compare)
        // 5. JumpIfFalse
        // 6. SetScore
        // 7. MarkRuleTriggered
        // 8. Return
        assert_eq!(program.instructions.len(), 8);

        assert!(matches!(
            program.instructions[0],
            Instruction::CheckEventType { .. }
        ));
        assert!(matches!(
            program.instructions[4],
            Instruction::JumpIfFalse { .. }
        ));
        assert!(matches!(
            program.instructions[5],
            Instruction::AddScore { value: 50 }
        ));
        assert!(matches!(
            program.instructions[6],
            Instruction::MarkRuleTriggered { .. }
        ));
        assert!(matches!(program.instructions[7], Instruction::Return));
    }

    #[test]
    fn test_compile_rule_without_event_type() {
        let when = WhenBlock::new().add_condition(Expression::binary(
            Expression::field_access(vec!["score".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(100.0)),
        ));

        let rule = Rule {
            id: "score_check".to_string(),
            name: "Score Check".to_string(),
            description: None,
            when,
            score: 25,
            params: None,
            metadata: None,
        };

        let program = RuleCompiler::compile(&rule).unwrap();

        // Should NOT have CheckEventType
        assert!(!matches!(
            program.instructions[0],
            Instruction::CheckEventType { .. }
        ));
    }

    #[test]
    fn test_compile_rule_with_multiple_conditions() {
        let when = WhenBlock::new()
            .add_condition(Expression::binary(
                Expression::field_access(vec!["amount".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(1000.0)),
            ))
            .add_condition(Expression::binary(
                Expression::field_access(vec!["country".to_string()]),
                Operator::Eq,
                Expression::literal(Value::String("US".to_string())),
            ));

        let rule = Rule {
            id: "multi_cond".to_string(),
            name: "Multiple Conditions".to_string(),
            description: None,
            when,
            score: 75,
            params: None,
            metadata: None,
        };

        let program = RuleCompiler::compile(&rule).unwrap();

        // Each condition generates: 3 instructions + JumpIfFalse
        // Plus: SetScore, MarkRuleTriggered, Return
        assert!(program.instructions.len() > 10);
    }

    #[test]
    fn test_program_metadata() {
        let when = WhenBlock::new();

        let rule = Rule {
            id: "test_rule".to_string(),
            name: "Test Rule".to_string(),
            description: Some("Test description".to_string()),
            when,
            score: 100,
            params: None,
            metadata: None,
        };

        let program = RuleCompiler::compile(&rule).unwrap();

        assert_eq!(program.metadata.source_id, "test_rule");
        assert_eq!(program.metadata.source_type, "rule");
        assert_eq!(program.metadata.name, Some("Test Rule".to_string()));
        assert_eq!(
            program.metadata.description,
            Some("Test description".to_string())
        );
    }
}
