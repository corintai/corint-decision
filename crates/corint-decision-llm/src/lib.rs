//! CORINT LLM Integration
//!
//! This crate provides LLM integration for CORINT decision engine, focusing on:
//! - Code generation: Generate rules, rulesets, and pipelines from natural language
//! - Offline analysis: Batch analysis of historical data
//! - Development assistance: Rule optimization suggestions
//!
//! **Note**: This crate is NOT for real-time pipeline execution.
//! LLM calls have 2-5 second latency, unsuitable for real-time decisions.

// Re-export core types
pub use cache::{InMemoryLLMCache, LLMCache};
pub use client::{LLMClient, LLMRequest, LLMResponse};
pub use error::{LLMError, Result};

// Re-export providers
pub use provider::{
    AnthropicProvider, DeepSeekProvider, GeminiProvider, LLMProvider, MockProvider, OpenAIProvider,
};

// Re-export generators
pub use generator::{
    APIConfigGenerator, APIConfigGeneratorConfig, DecisionFlow, DecisionFlowGenerator,
    DecisionFlowGeneratorConfig, PipelineGenerator, PipelineGeneratorConfig, RuleGenerator,
    RuleGeneratorConfig, RulesetGenerator, RulesetGeneratorConfig,
};

pub mod cache;
pub mod client;
pub mod error;
pub mod generator;
pub mod provider;

#[cfg(feature = "core-generation")]
pub use generator::core_generator::{CoreGeneration, CoreGenerationError, CoreGenerator};
