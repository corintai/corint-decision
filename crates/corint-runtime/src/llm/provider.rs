//! LLM provider implementations

use async_trait::async_trait;
use crate::error::Result;
use crate::llm::client::{LLMClient, LLMRequest, LLMResponse};
use crate::llm::cache::LLMCache;
use std::sync::Arc;

/// LLM provider trait
pub trait LLMProvider: LLMClient {
    /// Get the provider name
    fn provider_name(&self) -> &str;
}

/// Mock LLM provider for testing
pub struct MockProvider {
    name: String,
    default_response: String,
}

impl MockProvider {
    /// Create a new mock provider
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            default_response: "Mock LLM response".to_string(),
        }
    }

    /// Create with custom default response
    pub fn with_response(response: String) -> Self {
        Self {
            name: "mock".to_string(),
            default_response: response,
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
        Ok(LLMResponse::new(self.default_response.clone(), request.model)
            .with_tokens(10)
            .with_finish_reason("stop".to_string()))
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

/// OpenAI provider (placeholder for Phase 3 full implementation)
pub struct OpenAIProvider {
    #[allow(dead_code)]
    api_key: String,
    cache: Option<Arc<dyn LLMCache>>,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            cache: None,
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            cache: Some(cache),
        }
    }
}

#[async_trait]
impl LLMClient for OpenAIProvider {
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(&request).await {
                return Ok(cached);
            }
        }

        // TODO: Actual OpenAI API call
        // For now, return a placeholder
        let response = LLMResponse::new(
            "OpenAI response placeholder".to_string(),
            request.model.clone(),
        )
        .with_tokens(15)
        .with_finish_reason("stop".to_string());

        // Store in cache
        if let Some(ref cache) = self.cache {
            cache.set(request, response.clone()).await;
        }

        Ok(response)
    }

    fn name(&self) -> &str {
        "openai"
    }
}

impl LLMProvider for OpenAIProvider {
    fn provider_name(&self) -> &str {
        "OpenAI"
    }
}

/// Anthropic provider (placeholder for Phase 3 full implementation)
pub struct AnthropicProvider {
    #[allow(dead_code)]
    api_key: String,
    cache: Option<Arc<dyn LLMCache>>,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            cache: None,
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            cache: Some(cache),
        }
    }
}

#[async_trait]
impl LLMClient for AnthropicProvider {
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(&request).await {
                return Ok(cached);
            }
        }

        // TODO: Actual Anthropic API call
        // For now, return a placeholder
        let response = LLMResponse::new(
            "Anthropic response placeholder".to_string(),
            request.model.clone(),
        )
        .with_tokens(20)
        .with_finish_reason("end_turn".to_string());

        // Store in cache
        if let Some(ref cache) = self.cache {
            cache.set(request, response.clone()).await;
        }

        Ok(response)
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

impl LLMProvider for AnthropicProvider {
    fn provider_name(&self) -> &str {
        "Anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::cache::InMemoryLLMCache;

    #[tokio::test]
    async fn test_mock_provider() {
        let provider = MockProvider::new();
        let request = LLMRequest::new("Test".to_string(), "mock-model".to_string());

        let response = provider.call(request).await.unwrap();
        assert_eq!(response.content, "Mock LLM response");
    }

    #[tokio::test]
    async fn test_mock_provider_custom_response() {
        let provider = MockProvider::with_response("Custom response".to_string());
        let request = LLMRequest::new("Test".to_string(), "mock-model".to_string());

        let response = provider.call(request).await.unwrap();
        assert_eq!(response.content, "Custom response");
    }

    #[tokio::test]
    async fn test_openai_provider_with_cache() {
        let cache = Arc::new(InMemoryLLMCache::new());
        let provider = OpenAIProvider::with_cache("test-key".to_string(), cache.clone());

        let request = LLMRequest::new("Test prompt".to_string(), "gpt-4".to_string());

        // First call should hit the API
        let response1 = provider.call(request.clone()).await.unwrap();

        // Second call should hit the cache
        let response2 = provider.call(request.clone()).await.unwrap();

        assert_eq!(response1.content, response2.content);
    }

    #[tokio::test]
    async fn test_anthropic_provider_with_cache() {
        let cache = Arc::new(InMemoryLLMCache::new());
        let provider = AnthropicProvider::with_cache("test-key".to_string(), cache.clone());

        let request = LLMRequest::new("Test prompt".to_string(), "claude-3-opus".to_string());

        // First call should hit the API
        let response1 = provider.call(request.clone()).await.unwrap();

        // Second call should hit the cache
        let response2 = provider.call(request.clone()).await.unwrap();

        assert_eq!(response1.content, response2.content);
    }
}
