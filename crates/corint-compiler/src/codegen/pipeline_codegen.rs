//! Pipeline compiler
//!
//! Compiles Pipeline AST nodes into IR programs.

use corint_core::ast::{Pipeline, Step};
use corint_core::ir::{Instruction, Program, ProgramMetadata, FeatureType, TimeWindow};
use crate::error::Result;
use std::collections::HashMap;

/// Pipeline compiler
pub struct PipelineCompiler;

impl PipelineCompiler {
    /// Compile a pipeline into an IR program
    pub fn compile(pipeline: &Pipeline) -> Result<Program> {
        let mut instructions = Vec::new();

        // 1. Check event type if specified in when block
        if let Some(ref when_block) = pipeline.when {
            if let Some(ref event_type) = when_block.event_type {
                instructions.push(Instruction::CheckEventType {
                    expected: event_type.clone(),
                });
            }

            // 2. Compile when conditions (if any)
            if !when_block.conditions.is_empty() {
                use crate::codegen::expression_codegen::ExpressionCompiler;
                
                // Compile all conditions
                let mut condition_blocks = Vec::new();
                for condition in &when_block.conditions {
                    let condition_instructions = ExpressionCompiler::compile(condition)?;
                    condition_blocks.push(condition_instructions);
                }

                // Calculate total instructions to skip if conditions fail
                let mut steps_instruction_count = 0;
                for step in &pipeline.steps {
                    steps_instruction_count += Self::compile_step(step)?.len();
                }
                steps_instruction_count += 1; // +1 for Return instruction

                // Add conditions with correct jump offsets
                for (idx, condition_instructions) in condition_blocks.iter().enumerate() {
                    instructions.extend(condition_instructions.clone());

                    // Calculate how many instructions to skip if this condition is false
                    let mut remaining_instruction_count = steps_instruction_count;

                    for remaining_block in &condition_blocks[idx + 1..] {
                        remaining_instruction_count += remaining_block.len() + 1; // +1 for JumpIfFalse
                    }

                    instructions.push(Instruction::JumpIfFalse {
                        offset: remaining_instruction_count as isize
                    });
                }
            }
        }

        // 3. Compile each step in the pipeline
        for step in &pipeline.steps {
            instructions.extend(Self::compile_step(step)?);
        }

        // 4. Add return instruction
        instructions.push(Instruction::Return);

        // Create program metadata with pipeline id, name, description
        let pipeline_id = pipeline.id.clone().unwrap_or_else(|| "pipeline".to_string());
        let mut metadata = ProgramMetadata::for_pipeline(pipeline_id);
        
        // Add name and description to custom metadata
        if let Some(ref name) = pipeline.name {
            metadata.custom.insert("name".to_string(), name.clone());
        }
        if let Some(ref description) = pipeline.description {
            metadata.custom.insert("description".to_string(), description.clone());
        }
        // Add event_type hint for pre-filtering pipelines at runtime
        if let Some(ref when_block) = pipeline.when {
            if let Some(ref event_type) = when_block.event_type {
                metadata.custom.insert("event_type".to_string(), event_type.clone());
            }
        }

        Ok(Program::new(instructions, metadata))
    }

    /// Compile a single pipeline step
    fn compile_step(step: &Step) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        match step {
            Step::Extract { id: _, features } => {
                // Feature extraction step
                for _feature in features {
                    // TODO: Compile feature definition properly
                    // For now, create a basic count feature as placeholder
                    instructions.push(Instruction::CallFeature {
                        feature_type: FeatureType::Count,
                        field: vec!["data".to_string()],
                        filter: None,
                        time_window: TimeWindow::Last24Hours,
                    });
                }
            }

            Step::Reason {
                id: _,
                provider,
                model,
                prompt,
                output_schema: _,
            } => {
                // LLM reasoning step
                instructions.push(Instruction::CallLLM {
                    provider: provider.clone(),
                    model: model.clone(),
                    prompt: prompt.template.clone(),
                });
            }

            Step::Service {
                id: _,
                service,
                operation,
                params: _,
                output,
            } => {
                // Internal service call
                instructions.push(Instruction::CallService {
                    service: service.clone(),
                    operation: operation.clone(),
                    params: HashMap::new(), // TODO: Compile params
                });

                // If output is specified, store the result
                if let Some(output_path) = output {
                    instructions.push(Instruction::Store {
                        name: output_path.clone(),
                    });
                }
            }

            Step::Api {
                id: _,
                api,
                endpoint,
                params,
                output,
                timeout,
                on_error,
            } => {
                // External API call
                use corint_core::Value;

                // Extract fallback value if present
                let fallback = on_error.as_ref().and_then(|eh| {
                    eh.fallback.as_ref().map(|f| {
                        // Convert serde_json::Value to corint_core::Value
                        serde_json::from_value(f.clone()).unwrap_or(Value::Null)
                    })
                });

                // Compile params - for now, only handle literal expressions
                let mut compiled_params = HashMap::new();
                for (key, expr) in params {
                    // Extract literal values from expressions
                    if let corint_core::ast::Expression::Literal(value) = expr {
                        compiled_params.insert(key.clone(), value.clone());
                    }
                    // TODO: Support field access expressions by emitting LoadField instructions
                }

                instructions.push(Instruction::CallExternal {
                    api: api.clone(),
                    endpoint: endpoint.clone(),
                    params: compiled_params,
                    timeout: *timeout,
                    fallback,
                });

                // Store the result to the output variable
                instructions.push(Instruction::Store {
                    name: output.clone(),
                });
            }

            Step::Include { ruleset } => {
                // Include a ruleset - mark it for execution
                // We use a special instruction to indicate which ruleset to execute
                instructions.push(Instruction::CallRuleset {
                    ruleset_id: ruleset.clone(),
                });
            }

            Step::Branch { branches } => {
                // Conditional branching
                // Two-pass compilation: first compile all branches, then calculate jump offsets
                use crate::codegen::expression_codegen::ExpressionCompiler;

                // First pass: compile all branch conditions and bodies
                struct CompiledBranch {
                    condition: Vec<Instruction>,
                    body: Vec<Instruction>,
                }

                let mut compiled_branches = Vec::new();
                for branch in branches {
                    let condition = ExpressionCompiler::compile(&branch.condition)?;
                    let mut body = Vec::new();
                    for step in &branch.pipeline {
                        body.extend(Self::compile_step(step)?);
                    }
                    compiled_branches.push(CompiledBranch { condition, body });
                }

                // Second pass: assemble with correct jump offsets
                for (i, compiled) in compiled_branches.iter().enumerate() {
                    // Add condition instructions
                    instructions.extend(compiled.condition.clone());

                    if i < compiled_branches.len() - 1 {
                        // Not the last branch: need JumpIfFalse to next branch AND Jump to end
                        // JumpIfFalse offset = length of body + 1 (for the Jump instruction) + 1 (to skip past JumpIfFalse itself)
                        let jump_if_false_offset = (compiled.body.len() + 1 + 1) as isize;
                        instructions.push(Instruction::JumpIfFalse {
                            offset: jump_if_false_offset
                        });

                        // Add branch body
                        instructions.extend(compiled.body.clone());

                        // Calculate jump to end: skip all remaining branches
                        let mut remaining_size = 0;
                        for j in (i + 1)..compiled_branches.len() {
                            remaining_size += compiled_branches[j].condition.len();
                            remaining_size += compiled_branches[j].body.len();
                            remaining_size += 1; // JumpIfFalse instruction
                            if j < compiled_branches.len() - 1 {
                                remaining_size += 1; // Jump instruction (except for last branch)
                            }
                        }

                        // +1 to skip past the Jump instruction itself
                        instructions.push(Instruction::Jump {
                            offset: (remaining_size + 1) as isize
                        });
                    } else {
                        // Last branch: just JumpIfFalse and body
                        // +1 to skip past the JumpIfFalse instruction itself
                        instructions.push(Instruction::JumpIfFalse {
                            offset: (compiled.body.len() + 1) as isize
                        });
                        instructions.extend(compiled.body.clone());
                    }
                }
            }

            Step::Parallel { steps, merge: _ } => {
                // Parallel execution
                for parallel_step in steps {
                    instructions.extend(Self::compile_step(parallel_step)?);
                }
                // TODO: Handle merge strategy
            }
        }

        Ok(instructions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::{FeatureDefinition, Expression, PromptTemplate};

    #[test]
    fn test_compile_empty_pipeline() {
        let pipeline = Pipeline {
            id: Some("test_pipeline".to_string()),
            name: None,
            description: None,
            when: None,
            steps: vec![],
        };

        let program = PipelineCompiler::compile(&pipeline).unwrap();

        assert_eq!(program.metadata.source_type, "pipeline");
        assert!(program.instructions.len() >= 1); // At least Return
    }

    #[test]
    fn test_compile_extract_step() {
        let step = Step::Extract {
            id: "extract_features".to_string(),
            features: vec![FeatureDefinition {
                name: "user_age".to_string(),
                value: Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            }],
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        assert!(!instructions.is_empty());
        assert!(matches!(
            instructions[0],
            Instruction::CallFeature { .. }
        ));
    }

    #[test]
    fn test_compile_reason_step() {
        let step = Step::Reason {
            id: "llm_analysis".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            prompt: PromptTemplate {
                template: "Analyze this transaction".to_string(),
            },
            output_schema: None,
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        assert!(!instructions.is_empty());
        assert!(matches!(
            instructions[0],
            Instruction::CallLLM { .. }
        ));
    }

    #[test]
    fn test_compile_service_step() {
        let step = Step::Service {
            id: "check_blacklist".to_string(),
            service: "fraud_db".to_string(),
            operation: "check".to_string(),
            params: std::collections::HashMap::new(),
            output: Some("context.result".to_string()),
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        // Should generate CallService + Store
        assert_eq!(instructions.len(), 2);
        assert!(matches!(
            instructions[0],
            Instruction::CallService { .. }
        ));
        assert!(matches!(
            instructions[1],
            Instruction::Store { .. }
        ));
    }

    #[test]
    fn test_compile_include_step() {
        let step = Step::Include {
            ruleset: "fraud_rules".to_string(),
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        // Include step produces one CallRuleset instruction
        assert_eq!(instructions.len(), 1);
        assert!(matches!(
            instructions[0],
            Instruction::CallRuleset { .. }
        ));
    }

    #[test]
    fn test_compile_pipeline_with_multiple_steps() {
        let pipeline = Pipeline {
            id: Some("multi_step_pipeline".to_string()),
            name: None,
            description: None,
            when: None,
            steps: vec![
                Step::Extract {
                    id: "extract".to_string(),
                    features: vec![],
                },
                Step::Include {
                    ruleset: "rules".to_string(),
                },
            ],
        };

        let program = PipelineCompiler::compile(&pipeline).unwrap();

        assert_eq!(program.metadata.source_type, "pipeline");
        assert!(!program.instructions.is_empty());
    }
}
