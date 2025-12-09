//! Dead code elimination optimizer
//!
//! Removes unreachable code from IR programs.

use corint_core::ir::{Instruction, Program};

/// Dead code eliminator
pub struct DeadCodeEliminator;

impl DeadCodeEliminator {
    /// Create a new dead code eliminator
    pub fn new() -> Self {
        Self
    }

    /// Optimize a program by removing dead code
    pub fn eliminate(&self, program: &Program) -> Program {
        // IMPROVED: Use proper control flow analysis instead of naive linear scan
        // Build a map of potentially reachable instructions
        let reachable = self.compute_reachable_instructions(program);

        let mut optimized_instructions = Vec::new();
        for (i, instruction) in program.instructions.iter().enumerate() {
            if reachable.contains(&i) {
                optimized_instructions.push(instruction.clone());
            }
        }

        Program::new(optimized_instructions, program.metadata.clone())
    }

    /// Compute which instructions are reachable via control flow analysis
    fn compute_reachable_instructions(&self, program: &Program) -> std::collections::HashSet<usize> {
        use std::collections::{HashSet, VecDeque};

        let mut reachable = HashSet::new();
        let mut worklist = VecDeque::new();

        // Start from instruction 0 (entry point)
        if !program.instructions.is_empty() {
            worklist.push_back(0);
        }

        while let Some(pc) = worklist.pop_front() {
            // Skip if already processed or out of bounds
            if pc >= program.instructions.len() || reachable.contains(&pc) {
                continue;
            }

            // Mark as reachable
            reachable.insert(pc);

            // Determine successor instructions
            match &program.instructions[pc] {
                Instruction::Return => {
                    // No successors - execution ends
                }
                Instruction::Jump { offset } => {
                    // Unconditional jump - only successor is jump target
                    let target = (pc as isize + offset) as usize;
                    if target < program.instructions.len() {
                        worklist.push_back(target);
                    }
                }
                Instruction::JumpIfTrue { offset } | Instruction::JumpIfFalse { offset } => {
                    // Conditional jump - two successors:
                    // 1. Next instruction (fall-through)
                    // 2. Jump target
                    worklist.push_back(pc + 1);
                    let target = (pc as isize + offset) as usize;
                    if target < program.instructions.len() {
                        worklist.push_back(target);
                    }
                }
                _ => {
                    // All other instructions fall through to next
                    worklist.push_back(pc + 1);
                }
            }
        }

        reachable
    }

    /// Eliminate duplicate consecutive instructions
    pub fn eliminate_duplicates(&self, program: &Program) -> Program {
        let mut optimized_instructions = Vec::new();
        let mut last_instruction: Option<&Instruction> = None;

        for instruction in &program.instructions {
            // Skip if it's the same as the last instruction
            // (only for certain instruction types that are safe to deduplicate)
            let should_skip = match (last_instruction, instruction) {
                // Don't duplicate SetScore with same value
                (
                    Some(Instruction::SetScore { value: v1 }),
                    Instruction::SetScore { value: v2 },
                ) if v1 == v2 => true,

                // Don't duplicate SetAction with same action
                (
                    Some(Instruction::SetAction { action: a1 }),
                    Instruction::SetAction { action: a2 },
                ) if a1 == a2 => true,

                _ => false,
            };

            if !should_skip {
                optimized_instructions.push(instruction.clone());
                last_instruction = Some(instruction);
            }
        }

        Program::new(optimized_instructions, program.metadata.clone())
    }

    /// Remove no-op instructions
    pub fn eliminate_nops(&self, program: &Program) -> Program {
        let optimized_instructions: Vec<Instruction> = program
            .instructions
            .iter()
            .filter(|instr| !self.is_nop(instr))
            .cloned()
            .collect();

        Program::new(optimized_instructions, program.metadata.clone())
    }

    /// Check if an instruction is a no-op
    fn is_nop(&self, instruction: &Instruction) -> bool {
        match instruction {
            // Adding 0 to score is a no-op
            Instruction::AddScore { value } if *value == 0 => true,

            // Jump with offset 1 is a no-op (jumps to next instruction)
            Instruction::Jump { offset } if *offset == 1 => true,

            _ => false,
        }
    }

    /// Run all optimizations
    pub fn optimize(&self, program: &Program) -> Program {
        let program = self.eliminate(program);
        let program = self.eliminate_duplicates(&program);
        let program = self.eliminate_nops(&program);
        program
    }
}

impl Default for DeadCodeEliminator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::Action;
    use corint_core::ir::ProgramMetadata;

    #[test]
    fn test_eliminate_code_after_return() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::Return,
            Instruction::SetScore { value: 100 }, // Dead code
            Instruction::AddScore { value: 25 },  // Dead code
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_no_dead_code() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::AddScore { value: 25 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions.clone(),
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate(&program);

        assert_eq!(optimized.instructions.len(), 3);
        assert_eq!(optimized.instructions, instructions);
    }

    #[test]
    fn test_eliminate_duplicate_set_score() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::SetScore { value: 50 }, // Duplicate
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_duplicates(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_eliminate_duplicate_set_action() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetAction {
                action: Action::Approve,
            },
            Instruction::SetAction {
                action: Action::Approve,
            }, // Duplicate
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_duplicates(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetAction {
                action: Action::Approve
            }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_no_eliminate_different_values() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::SetScore { value: 75 }, // Different value
            Instruction::Return,
        ];

        let program = Program::new(
            instructions.clone(),
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_duplicates(&program);

        assert_eq!(optimized.instructions.len(), 3);
        assert_eq!(optimized.instructions, instructions);
    }

    #[test]
    fn test_eliminate_add_zero() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::AddScore { value: 0 }, // No-op
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_nops(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_eliminate_jump_to_next() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::Jump { offset: 1 }, // No-op (jumps to next instruction)
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_nops(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_optimize_combined() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::AddScore { value: 0 },  // No-op
            Instruction::SetScore { value: 75 },
            Instruction::SetScore { value: 75 }, // Duplicate
            Instruction::Return,
            Instruction::SetScore { value: 100 }, // Dead code
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.optimize(&program);

        // Should have: SetScore(50), SetScore(75), Return
        assert_eq!(optimized.instructions.len(), 3);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(
            optimized.instructions[1],
            Instruction::SetScore { value: 75 }
        ));
        assert!(matches!(optimized.instructions[2], Instruction::Return));
    }

    #[test]
    fn test_empty_program() {
        let eliminator = DeadCodeEliminator::new();

        let program = Program::new(vec![], ProgramMetadata::for_rule("test".to_string()));

        let optimized = eliminator.optimize(&program);

        assert_eq!(optimized.instructions.len(), 0);
    }

    #[test]
    fn test_keep_meaningful_add_score() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::AddScore { value: 25 }, // Not a no-op
            Instruction::Return,
        ];

        let program = Program::new(
            instructions.clone(),
            ProgramMetadata::for_rule("test".to_string()),
        );

        let optimized = eliminator.eliminate_nops(&program);

        assert_eq!(optimized.instructions.len(), 3);
        assert_eq!(optimized.instructions, instructions);
    }

    #[test]
    fn test_default_action_with_conditional_actions() {
        // This test reproduces the bug with default actions
        // Scenario: conditional action followed by default action
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            // Conditional check
            Instruction::LoadField {
                path: vec!["amount".to_string()],
            },
            Instruction::LoadConst {
                value: corint_core::Value::Number(1000.0),
            },
            Instruction::Compare {
                op: corint_core::ast::Operator::Gt,
            },
            // If amount > 1000, Review (then jump to end)
            Instruction::JumpIfFalse { offset: 3 }, // Skip Review + Jump
            Instruction::SetAction {
                action: Action::Review,
            },
            Instruction::Jump { offset: 2 }, // Jump past default action
            // Default action - Approve (should NOT be eliminated)
            Instruction::SetAction {
                action: Action::Approve,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions.clone(),
            ProgramMetadata::for_ruleset("test".to_string()),
        );

        let optimized = eliminator.optimize(&program);

        // The bug: dead code eliminator might incorrectly remove the default SetAction(Approve)
        // because it thinks the Jump makes it unreachable
        // But the Jump is only taken when the condition is TRUE
        // When the condition is FALSE, we fall through to the default action

        // Verify the default action is still there
        assert!(
            optimized.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::SetAction {
                    action: Action::Approve
                }
            )),
            "Default action (Approve) should not be eliminated"
        );
    }
}
