//! Mock LLM provider for testing

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::Result;
use crate::provider::LLMProvider;
use async_trait::async_trait;

/// Mock LLM provider for testing
pub struct MockProvider {
    name: String,
    default_response: String,
    default_thinking: Option<String>,
}

impl MockProvider {
    /// Create a new mock provider
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            default_response: "Mock LLM response".to_string(),
            default_thinking: None,
        }
    }

    /// Create with custom default response
    pub fn with_response(response: String) -> Self {
        Self {
            name: "mock".to_string(),
            default_response: response,
            default_thinking: None,
        }
    }

    /// Create with thinking mode enabled
    pub fn with_thinking(response: String, thinking: String) -> Self {
        Self {
            name: "mock".to_string(),
            default_response: response,
            default_thinking: Some(thinking),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockProvider {
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse> {
        let mut response = LLMResponse::new(self.default_response.clone(), request.model)
            .with_tokens(10)
            .with_finish_reason("stop".to_string());

        // Add thinking if enabled or if default thinking is set
        if request.enable_thinking.unwrap_or(false) || self.default_thinking.is_some() {
            let thinking = self
                .default_thinking
                .clone()
                .unwrap_or_else(|| "Mock thinking process...".to_string());
            response = response.with_thinking(thinking);
        }

        Ok(response)
    }

    fn supports_thinking(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl LLMProvider for MockProvider {
    fn provider_name(&self) -> &str {
        "Mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider() {
        let provider = MockProvider::new();
        let request = LLMRequest::new("Test".to_string(), "mock-model".to_string());

        let response = provider.call(request).await.unwrap();
        assert_eq!(response.content, "Mock LLM response");
        assert!(provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_mock_provider_with_thinking() {
        let provider = MockProvider::new();
        let request =
            LLMRequest::new("Test".to_string(), "mock-model".to_string()).with_thinking(true);

        let response = provider.call(request).await.unwrap();
        assert!(response.thinking.is_some());
        assert_eq!(response.thinking.unwrap(), "Mock thinking process...");
    }
}
