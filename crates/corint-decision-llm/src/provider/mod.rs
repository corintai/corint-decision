//! LLM provider implementations

use crate::client::LLMClient;

/// LLM provider trait
pub trait LLMProvider: LLMClient {
    /// Get the provider name
    fn provider_name(&self) -> &str;
}

// Re-export all providers
mod anthropic;
mod deepseek;
mod gemini;
mod mock;
mod openai;

pub use anthropic::AnthropicProvider;
pub use deepseek::DeepSeekProvider;
pub use gemini::GeminiProvider;
pub use mock::MockProvider;
pub use openai::OpenAIProvider;
