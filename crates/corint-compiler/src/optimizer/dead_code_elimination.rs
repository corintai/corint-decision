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
        let mut optimized_instructions = Vec::new();
        let mut reachable = true;

        for instruction in &program.instructions {
            // If we've hit unreachable code, skip it
            if !reachable {
                // Check if this is a jump target or label (not implemented yet)
                // For now, we stop at Return
                continue;
            }

            // Add the instruction
            optimized_instructions.push(instruction.clone());

            // Check if this instruction makes subsequent code unreachable
            match instruction {
                Instruction::Return => {
                    // Everything after return is unreachable
                    reachable = false;
                }
                Instruction::Jump { .. } => {
                    // Unconditional jump makes next instruction unreachable
                    // (unless it's a jump target)
                    reachable = false;
                }
                _ => {
                    // Other instructions don't affect reachability
                }
            }
        }

        Program::new(optimized_instructions, program.metadata.clone())
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
    use corint_core::Value;

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
}
