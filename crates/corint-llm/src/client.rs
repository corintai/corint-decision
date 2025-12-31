//! LLM client interface and types

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Request to an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRequest {
    /// The prompt to send to the LLM
    pub prompt: String,

    /// Model identifier (e.g., "gpt-4", "claude-3-opus", "o1-preview")
    pub model: String,

    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,

    /// Temperature for sampling (0.0 - 1.0)
    pub temperature: Option<f32>,

    /// System message/instructions
    pub system: Option<String>,

    /// Enable extended thinking for reasoning models (Claude, O1, etc.)
    pub enable_thinking: Option<bool>,
}

impl LLMRequest {
    /// Create a new LLM request
    pub fn new(prompt: String, model: String) -> Self {
        Self {
            prompt,
            model,
            max_tokens: None,
            temperature: None,
            system: None,
            enable_thinking: None,
        }
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set system message
    pub fn with_system(mut self, system: String) -> Self {
        self.system = Some(system);
        self
    }

    /// Enable extended thinking mode for reasoning models
    pub fn with_thinking(mut self, enable: bool) -> Self {
        self.enable_thinking = Some(enable);
        self
    }
}

/// Response from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// The generated text
    pub content: String,

    /// Model that generated the response
    pub model: String,

    /// Number of tokens used
    pub tokens_used: u32,

    /// Finish reason (e.g., "stop", "length")
    pub finish_reason: String,

    /// Extended thinking content (for reasoning models)
    pub thinking: Option<String>,
}

impl LLMResponse {
    /// Create a new LLM response
    pub fn new(content: String, model: String) -> Self {
        Self {
            content,
            model,
            tokens_used: 0,
            finish_reason: "stop".to_string(),
            thinking: None,
        }
    }

    /// Set tokens used
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.tokens_used = tokens;
        self
    }

    /// Set finish reason
    pub fn with_finish_reason(mut self, reason: String) -> Self {
        self.finish_reason = reason;
        self
    }

    /// Set thinking content
    pub fn with_thinking(mut self, thinking: String) -> Self {
        self.thinking = Some(thinking);
        self
    }
}

/// Async LLM client trait
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Call the LLM with a request for text generation
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse>;

    /// Check if this client supports extended thinking mode
    fn supports_thinking(&self) -> bool {
        false
    }

    /// Get the name of this client
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_request_builder() {
        let request = LLMRequest::new("Test prompt".to_string(), "gpt-4".to_string())
            .with_max_tokens(100)
            .with_temperature(0.7)
            .with_system("You are a helpful assistant".to_string());

        assert_eq!(request.prompt, "Test prompt");
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(
            request.system,
            Some("You are a helpful assistant".to_string())
        );
    }

    #[test]
    fn test_llm_response_builder() {
        let response = LLMResponse::new("Generated text".to_string(), "gpt-4".to_string())
            .with_tokens(50)
            .with_finish_reason("stop".to_string());

        assert_eq!(response.content, "Generated text");
        assert_eq!(response.model, "gpt-4");
        assert_eq!(response.tokens_used, 50);
        assert_eq!(response.finish_reason, "stop");
    }

    #[test]
    fn test_thinking_request_builder() {
        let request = LLMRequest::new("Solve this problem".to_string(), "o1-preview".to_string())
            .with_thinking(true);

        assert_eq!(request.prompt, "Solve this problem");
        assert_eq!(request.model, "o1-preview");
        assert_eq!(request.enable_thinking, Some(true));
    }

    #[test]
    fn test_thinking_response_builder() {
        let response = LLMResponse::new("Answer".to_string(), "claude-3-opus".to_string())
            .with_thinking("Let me think...".to_string())
            .with_tokens(100);

        assert_eq!(response.content, "Answer");
        assert_eq!(response.thinking, Some("Let me think...".to_string()));
        assert_eq!(response.tokens_used, 100);
    }
}
