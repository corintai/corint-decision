//! Code generation from natural language descriptions
//!
//! This module provides LLM-powered generators for CORINT DSL components:
//! - Rules: Individual decision rules
//! - Rulesets: Collections of related rules
//! - Pipelines: Decision workflows
//! - API Configs: External API configurations
//! - Decision Flows: Complete decision flows with all components
//!
//! # Example - Rule Generation
//! ```no_run
//! use corint_llm::{RuleGenerator, RuleGeneratorConfig, OpenAIProvider};
//! use std::sync::Arc;
//!
//! # async fn example() -> corint_llm::Result<()> {
//! let provider = Arc::new(OpenAIProvider::new("your-api-key".to_string()));
//! let config = RuleGeneratorConfig::new("gpt-4").with_temperature(0.3);
//! let generator = RuleGenerator::new(provider, config);
//!
//! let description = "Block transactions over $10,000 from users with less than 3 months account age";
//! let rule_yaml = generator.generate(description).await?;
//!
//! println!("Generated Rule:\n{}", rule_yaml);
//! # Ok(())
//! # }
//! ```
//!
//! # Example - Complete Decision Flow
//! ```no_run
//! use corint_llm::{DecisionFlowGenerator, MockProvider};
//! use std::sync::Arc;
//!
//! # async fn example() -> corint_llm::Result<()> {
//! let provider = Arc::new(MockProvider::new());
//! let generator = DecisionFlowGenerator::with_defaults(provider);
//!
//! let description = "Create a fraud detection system with IP checks, velocity limits, and amount thresholds";
//! let flow = generator.generate(description).await?;
//!
//! println!("Generated {} rules, {} rulesets, {} pipelines",
//!     flow.rule_count, flow.ruleset_count, flow.pipeline_count);
//! # Ok(())
//! # }
//! ```

pub mod api_config_generator;
pub mod decision_flow_generator;
pub mod pipeline_generator;
pub mod prompt_templates;
pub mod rule_generator;
pub mod ruleset_generator;
pub mod yaml_extractor;

// Re-export main types
pub use api_config_generator::{APIConfigGenerator, APIConfigGeneratorConfig};
pub use decision_flow_generator::{DecisionFlow, DecisionFlowGenerator, DecisionFlowGeneratorConfig};
pub use pipeline_generator::{PipelineGenerator, PipelineGeneratorConfig};
pub use rule_generator::{RuleGenerator, RuleGeneratorConfig};
pub use ruleset_generator::{RulesetGenerator, RulesetGeneratorConfig};
pub use yaml_extractor::{extract_multiple_yaml, extract_yaml};
