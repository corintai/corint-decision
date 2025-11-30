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

        // Compile each step in the pipeline
        for step in &pipeline.steps {
            instructions.extend(Self::compile_step(step)?);
        }

        // Add return instruction
        instructions.push(Instruction::Return);

        // Create program metadata
        let metadata = ProgramMetadata::for_pipeline("pipeline".to_string());

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
            } => {
                // External service call
                instructions.push(Instruction::CallService {
                    service: service.clone(),
                    operation: operation.clone(),
                    params: HashMap::new(), // TODO: Compile params
                });
            }

            Step::Include { ruleset: _ } => {
                // Include another ruleset
                // This would typically load and execute the referenced ruleset
                // For now, this is a placeholder
            }

            Step::Branch { branches: _ } => {
                // Conditional branching
                // TODO: Compile branch condition and pipeline
                // For now, this is a placeholder
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
        let pipeline = Pipeline { steps: vec![] };

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
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        assert!(!instructions.is_empty());
        assert!(matches!(
            instructions[0],
            Instruction::CallService { .. }
        ));
    }

    #[test]
    fn test_compile_include_step() {
        let step = Step::Include {
            ruleset: "fraud_rules".to_string(),
        };

        let instructions = PipelineCompiler::compile_step(&step).unwrap();

        // Include step currently produces no instructions (placeholder)
        assert_eq!(instructions.len(), 0);
    }

    #[test]
    fn test_compile_pipeline_with_multiple_steps() {
        let pipeline = Pipeline {
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
