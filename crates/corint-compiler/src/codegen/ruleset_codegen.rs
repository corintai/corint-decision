//! Ruleset compiler
//!
//! Compiles Ruleset AST nodes into IR programs.

use corint_core::ast::{Action, Ruleset};
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use crate::error::Result;

/// Ruleset compiler
pub struct RulesetCompiler;

impl RulesetCompiler {
    /// Compile a ruleset into an IR program
    pub fn compile(ruleset: &Ruleset) -> Result<Program> {
        let mut instructions = Vec::new();

        // For now, a ruleset compilation is a placeholder
        // In a full implementation, this would:
        // 1. Load and compile all referenced rules
        // 2. Evaluate decision logic based on accumulated scores
        // 3. Execute the appropriate action

        // Add a comment instruction for documentation
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
        for decision_rule in &ruleset.decision_logic {
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
            if let Some(_condition) = &decision_rule.condition {
                // TODO: Compile condition expression
                // For now, this is a placeholder

                // If condition is true, execute action
                instructions.extend(Self::compile_action(&decision_rule.action)?);

                if decision_rule.terminate {
                    instructions.push(Instruction::Return);
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
}
