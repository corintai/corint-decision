//! LLM provider implementations

use crate::client::LLMClient;

/// LLM provider trait
pub trait LLMProvider: LLMClient {
    /// Get the provider name
    fn provider_name(&self) -> &str;
}

// Re-export all providers
mod mock;
mod openai;
mod anthropic;
mod gemini;
mod deepseek;

pub use mock::MockProvider;
pub use openai::OpenAIProvider;
pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use deepseek::DeepSeekProvider;
