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
pub use client::{LLMClient, LLMRequest, LLMResponse};
pub use cache::{LLMCache, InMemoryLLMCache};
pub use error::{LLMError, Result};

// Re-export providers
pub use provider::{
    LLMProvider,
    OpenAIProvider,
    AnthropicProvider,
    GeminiProvider,
    DeepSeekProvider,
    MockProvider,
};

// Re-export generators
pub use generator::{
    RuleGenerator, RuleGeneratorConfig,
    RulesetGenerator, RulesetGeneratorConfig,
    PipelineGenerator, PipelineGeneratorConfig,
    APIConfigGenerator, APIConfigGeneratorConfig,
    DecisionFlowGenerator, DecisionFlowGeneratorConfig, DecisionFlow,
};

pub mod client;
pub mod cache;
pub mod error;
pub mod provider;
pub mod generator;
