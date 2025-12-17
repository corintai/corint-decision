//! Rule compiler
//!
//! Compiles Rule AST nodes into IR programs.

use super::expression_codegen::ExpressionCompiler;
use crate::error::{CompileError, Result};
use corint_core::ast::rule::{Condition, ConditionGroup};
use corint_core::ast::{Rule, UnaryOperator};
use corint_core::ir::{Instruction, Program, ProgramMetadata};

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

        // 2. Compile conditions (support both old and new formats)
        // Conditions produce a single boolean on the stack
        let condition_instructions = if let Some(ref group) = rule.when.condition_group {
            // New format: use condition groups (produces single boolean)
            Self::compile_condition_group(group)?
        } else if let Some(ref conditions) = rule.when.conditions {
            // Legacy format: treat as implicit "all" (produces single boolean)
            Self::compile_legacy_conditions_chained(conditions)?
        } else {
            // No conditions - always passes
            Vec::new()
        };

        // Add the condition instructions
        if !condition_instructions.is_empty() {
            instructions.extend(condition_instructions);

            // 3. JumpIfFalse: skip AddScore and MarkRuleTriggered if condition is false
            // Offset = 3 to skip over: JumpIfFalse itself counted, AddScore, MarkRuleTriggered
            // Landing on Return
            instructions.push(Instruction::JumpIfFalse {
                offset: 3, // Skip AddScore + MarkRuleTriggered, land on Return
            });
        }

        // 4. If conditions passed (or no conditions), add the score
        instructions.push(Instruction::AddScore { value: rule.score });

        // 4. Mark this rule as triggered
        instructions.push(Instruction::MarkRuleTriggered {
            rule_id: rule.id.clone(),
        });

        // 5. Return
        instructions.push(Instruction::Return);

        // Create program metadata
        let mut metadata = ProgramMetadata::for_rule(rule.id.clone()).with_name(rule.name.clone());

        if let Some(desc) = &rule.description {
            metadata = metadata.with_description(desc.clone());
        }

        // Store condition information in metadata
        Self::store_condition_metadata(&rule.when, &mut metadata);

        // Store event type if specified
        if let Some(event_type) = &rule.when.event_type {
            metadata
                .custom
                .insert("event_type".to_string(), event_type.clone());
        }

        Ok(Program::new(instructions, metadata))
    }

    /// Compile legacy format conditions (implicit AND) using BinaryOp::And chaining
    /// Produces a single boolean on the stack
    fn compile_legacy_conditions_chained(
        conditions: &[corint_core::ast::Expression],
    ) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Handle empty conditions: no conditions means always true
        if conditions.is_empty() {
            return Ok(instructions);
        }

        // Compile each condition and chain with AND
        for (i, condition) in conditions.iter().enumerate() {
            let cond_instructions = ExpressionCompiler::compile(condition)?;
            instructions.extend(cond_instructions);

            // After first condition, AND with previous result
            if i > 0 {
                instructions.push(Instruction::BinaryOp {
                    op: corint_core::ast::Operator::And,
                });
            }
        }

        Ok(instructions)
    }

    /// Compile a condition group (new format) - produces a single boolean on the stack
    /// This version uses BinaryOp::And/Or chaining, which works correctly for nested groups
    fn compile_condition_group(group: &ConditionGroup) -> Result<Vec<Instruction>> {
        match group {
            ConditionGroup::All(conditions) => Self::compile_all_conditions_chained(conditions),
            ConditionGroup::Any(conditions) => Self::compile_any_conditions_chained(conditions),
            ConditionGroup::Not(conditions) => Self::compile_not_conditions_chained(conditions),
        }
    }

    /// Compile "all" conditions using BinaryOp::And chaining
    /// Produces a single boolean on the stack
    fn compile_all_conditions_chained(conditions: &[Condition]) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Handle empty conditions: ALL of nothing is true
        if conditions.is_empty() {
            instructions.push(Instruction::LoadConst {
                value: corint_core::Value::Bool(true),
            });
            return Ok(instructions);
        }

        // Compile each condition and chain with AND
        for (i, condition) in conditions.iter().enumerate() {
            let cond_instructions = Self::compile_condition(condition)?;
            instructions.extend(cond_instructions);

            // After first condition, AND with previous result
            if i > 0 {
                instructions.push(Instruction::BinaryOp {
                    op: corint_core::ast::Operator::And,
                });
            }
        }

        Ok(instructions)
    }

    /// Compile "any" conditions using BinaryOp::Or chaining
    /// Produces a single boolean on the stack
    fn compile_any_conditions_chained(conditions: &[Condition]) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Handle empty conditions: ANY of nothing is false
        if conditions.is_empty() {
            instructions.push(Instruction::LoadConst {
                value: corint_core::Value::Bool(false),
            });
            return Ok(instructions);
        }

        // Compile each condition and chain with OR
        for (i, condition) in conditions.iter().enumerate() {
            let cond_instructions = Self::compile_condition(condition)?;
            instructions.extend(cond_instructions);

            // After first condition, OR with previous result
            if i > 0 {
                instructions.push(Instruction::BinaryOp {
                    op: corint_core::ast::Operator::Or,
                });
            }
        }

        Ok(instructions)
    }

    /// Compile "not" conditions (negation)
    /// Produces a single boolean on the stack
    fn compile_not_conditions_chained(conditions: &[Condition]) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        if conditions.len() == 1 {
            // Single condition: compile and negate
            let cond_instructions = Self::compile_condition(&conditions[0])?;
            instructions.extend(cond_instructions);
        } else {
            // Multiple conditions: treat as implicit AND, then negate
            let all_cond_instructions = Self::compile_all_conditions_chained(conditions)?;
            instructions.extend(all_cond_instructions);
        }

        // Negate the result
        instructions.push(Instruction::UnaryOp {
            op: UnaryOperator::Not,
        });

        Ok(instructions)
    }

    /// Compile a single condition (expression or nested group)
    /// Produces a single boolean on the stack
    fn compile_condition(condition: &Condition) -> Result<Vec<Instruction>> {
        match condition {
            Condition::Expression(expr) => ExpressionCompiler::compile(expr),
            Condition::Group(group) => Self::compile_condition_group(group),
        }
    }

    /// Store condition metadata for tracing
    fn store_condition_metadata(
        when: &corint_core::ast::WhenBlock,
        metadata: &mut ProgramMetadata,
    ) {
        // For legacy format
        if let Some(ref conditions) = when.conditions {
            if !conditions.is_empty() {
                let conditions_json: Vec<serde_json::Value> = conditions
                    .iter()
                    .map(|c| ExpressionCompiler::expression_to_json(c))
                    .collect();
                if let Ok(json_str) = serde_json::to_string(&conditions_json) {
                    metadata.custom.insert("conditions_json".to_string(), json_str);
                }
                let condition_strs: Vec<String> = conditions
                    .iter()
                    .map(|c| ExpressionCompiler::expression_to_string(c))
                    .collect();
                metadata
                    .custom
                    .insert("conditions".to_string(), condition_strs.join(" && "));
            }
        }

        // For new format
        if let Some(ref group) = when.condition_group {
            if let Ok(json_str) = serde_json::to_string(group) {
                metadata
                    .custom
                    .insert("condition_group_json".to_string(), json_str);
            }
        }
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
