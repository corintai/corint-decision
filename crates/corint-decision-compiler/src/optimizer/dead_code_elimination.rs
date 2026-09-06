//! Control-flow preserving elimination of redundant IR instructions.
use corint_decision_model::ir::condition_map::CONDITION_MAP;
use corint_decision_model::ir::{Instruction, Program};

pub struct DeadCodeEliminator;
impl DeadCodeEliminator {
    pub fn new() -> Self {
        Self
    }
    pub fn eliminate(&self, program: &Program) -> Program {
        let reachable = self.compute_reachable_instructions(program);
        self.rewrite(
            program,
            &(0..program.instructions.len())
                .map(|pc| reachable.contains(&pc))
                .collect::<Vec<_>>(),
        )
    }
    /// Compute which instructions are reachable via control flow analysis
    fn compute_reachable_instructions(
        &self,
        program: &Program,
    ) -> std::collections::HashSet<usize> {
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
                    if let Some(target) = pc
                        .checked_add_signed(*offset)
                        .filter(|target| *target < program.instructions.len())
                    {
                        worklist.push_back(target);
                    }
                }
                Instruction::JumpIfTrue { offset } | Instruction::JumpIfFalse { offset } => {
                    // Conditional jump - two successors:
                    // 1. Next instruction (fall-through)
                    // 2. Jump target
                    worklist.push_back(pc + 1);
                    if let Some(target) = pc
                        .checked_add_signed(*offset)
                        .filter(|target| *target < program.instructions.len())
                    {
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

    pub fn eliminate_duplicates(&self, program: &Program) -> Program {
        let targets: std::collections::HashSet<usize> = program
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(pc, instruction)| match instruction {
                Instruction::Jump { offset }
                | Instruction::JumpIfTrue { offset }
                | Instruction::JumpIfFalse { offset } => pc.checked_add_signed(*offset),
                _ => None,
            })
            .collect();
        let mut keep = vec![true; program.instructions.len()];
        for (pc, retained) in keep.iter_mut().enumerate().skip(1) {
            // A jump directly to the second write must still perform that write.
            if !targets.contains(&pc)
                && matches!(
                    &program.instructions[pc],
                    Instruction::SetScore { .. } | Instruction::SetSignal { .. }
                )
                && program.instructions[pc] == program.instructions[pc - 1]
            {
                *retained = false;
            }
        }
        self.rewrite(program, &keep)
    }
    pub fn eliminate_nops(&self, program: &Program) -> Program {
        self.rewrite(
            program,
            &program
                .instructions
                .iter()
                .map(|i| !self.is_nop(i))
                .collect::<Vec<_>>(),
        )
    }
    fn is_nop(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::AddScore { value: 0 } | Instruction::Jump { offset: 1 }
        )
    }
    fn rewrite(&self, program: &Program, keep: &[bool]) -> Program {
        // Observed programs retain their exact boundaries, including unreachable
        // conditions. Eliminating those would change skipped-condition traces.
        if program.metadata.custom.contains_key(CONDITION_MAP) {
            return program.clone();
        }
        let mut positions = vec![0usize; keep.len() + 1];
        for (pc, retained) in keep.iter().enumerate() {
            positions[pc + 1] = positions[pc] + usize::from(*retained);
        }
        let mut instructions = Vec::new();
        for (pc, instruction) in program.instructions.iter().enumerate() {
            if !keep[pc] {
                continue;
            }
            let mut instruction = instruction.clone();
            match &mut instruction {
                Instruction::Jump { offset }
                | Instruction::JumpIfTrue { offset }
                | Instruction::JumpIfFalse { offset } => {
                    let Some(target) = pc
                        .checked_add_signed(*offset)
                        .filter(|target| *target <= keep.len())
                    else {
                        // Leave malformed IR intact for the VM's validation error.
                        return program.clone();
                    };
                    *offset = positions[target] as isize - positions[pc] as isize;
                }
                _ => {}
            }
            instructions.push(instruction);
        }
        let mut result = program.clone();
        result.instructions = instructions;
        result
    }
    pub fn optimize(&self, program: &Program) -> Program {
        let program = self.eliminate(program);
        let program = self.eliminate_duplicates(&program);
        self.eliminate_nops(&program)
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
    use corint_decision_model::ast::Signal;
    use corint_decision_model::ir::ProgramMetadata;

    #[test]
    fn test_eliminate_code_after_return() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::Return,
            Instruction::SetScore { value: 100 }, // Dead code
            Instruction::AddScore { value: 25 },  // Dead code
        ];

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

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

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

        let optimized = eliminator.eliminate_duplicates(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetScore { value: 50 }
        ));
        assert!(matches!(optimized.instructions[1], Instruction::Return));
    }

    #[test]
    fn test_eliminate_duplicate_set_signal() {
        let eliminator = DeadCodeEliminator::new();

        let instructions = vec![
            Instruction::SetSignal {
                signal: Signal::Approve,
            },
            Instruction::SetSignal {
                signal: Signal::Approve,
            }, // Duplicate
            Instruction::Return,
        ];

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

        let optimized = eliminator.eliminate_duplicates(&program);

        assert_eq!(optimized.instructions.len(), 2);
        assert!(matches!(
            optimized.instructions[0],
            Instruction::SetSignal {
                signal: Signal::Approve
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

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

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

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

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
            Instruction::AddScore { value: 0 }, // No-op
            Instruction::SetScore { value: 75 },
            Instruction::SetScore { value: 75 }, // Duplicate
            Instruction::Return,
            Instruction::SetScore { value: 100 }, // Dead code
        ];

        let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

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
                value: corint_decision_model::Value::Number(1000.0),
            },
            Instruction::Compare {
                op: corint_decision_model::ast::Operator::Gt,
            },
            // If amount > 1000, Review (then jump to end)
            Instruction::JumpIfFalse { offset: 3 }, // Skip Review + Jump
            Instruction::SetSignal {
                signal: Signal::Review,
            },
            Instruction::Jump { offset: 2 }, // Jump past default signal
            // Default signal - Approve (should NOT be eliminated)
            Instruction::SetSignal {
                signal: Signal::Approve,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions.clone(),
            ProgramMetadata::for_ruleset("test".to_string()),
        );

        let optimized = eliminator.optimize(&program);

        // The bug: dead code eliminator might incorrectly remove the default SetSignal(Approve)
        // because it thinks the Jump makes it unreachable
        // But the Jump is only taken when the condition is TRUE
        // When the condition is FALSE, we fall through to the default signal

        // Verify the default signal is still there
        assert!(
            optimized.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::SetSignal {
                    signal: Signal::Approve
                }
            )),
            "Default signal (Approve) should not be eliminated"
        );
    }
}

#[cfg(test)]
mod control_flow_regressions {
    use super::*;
    use corint_decision_model::ir::ProgramMetadata;
    #[test]
    fn targeted_writes_and_observer_boundaries_are_preserved() {
        let mut program = Program::new(
            vec![
                Instruction::Jump { offset: 2 },
                Instruction::SetScore { value: 5 },
                Instruction::SetScore { value: 5 },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );
        let optimizer = DeadCodeEliminator::new();
        assert_eq!(optimizer.eliminate_duplicates(&program), program);
        program
            .metadata
            .custom
            .insert(CONDITION_MAP.into(), "[]".into());
        assert_eq!(optimizer.optimize(&program), program);
    }
    #[test]
    fn malformed_offsets_do_not_panic_in_optimizer() {
        let program = Program::new(
            vec![
                Instruction::AddScore { value: 1 },
                Instruction::Jump { offset: isize::MAX },
            ],
            ProgramMetadata::default(),
        );
        assert_eq!(DeadCodeEliminator::new().optimize(&program), program);
    }
}
