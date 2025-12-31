//! OpenAI provider implementation

use crate::cache::LLMCache;
use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::provider::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

/// OpenAI provider (supports standard models and O1 thinking models)
pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    cache: Option<Arc<dyn LLMCache>>,
    client: Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            cache: None,
            client: Client::new(),
        }
    }

    /// Create with custom base URL (e.g., for Azure OpenAI)
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            cache: None,
            client: Client::new(),
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            cache: Some(cache),
            client: Client::new(),
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

        // Build messages
        let mut messages = Vec::new();

        // O1 models don't support system messages in the same way
        let is_thinking_model =
            request.model.starts_with("o1-") || request.model.starts_with("o3-");

        if !is_thinking_model {
            if let Some(system) = &request.system {
                messages.push(json!({
                    "role": "system",
                    "content": system
                }));
            }
        }

        messages.push(json!({
            "role": "user",
            "content": request.prompt
        }));

        // Build request body
        let mut body = json!({
            "model": request.model,
            "messages": messages,
        });

        // O1 models have different parameters
        if !is_thinking_model {
            if let Some(max_tokens) = request.max_tokens {
                body["max_tokens"] = json!(max_tokens);
            }
            if let Some(temperature) = request.temperature {
                body["temperature"] = json!(temperature);
            }
        } else {
            // O1 models use max_completion_tokens instead
            if let Some(max_tokens) = request.max_tokens {
                body["max_completion_tokens"] = json!(max_tokens);
            }
        }

        // Make API call
        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ApiCallFailed(format!("OpenAI API call failed: {}", e))
            })?;

        let status = resp.status();
        let resp_text = resp.text().await.map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::ApiCallFailed(format!(
                "OpenAI API error ({}): {}",
                status, resp_text
            )));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text).map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to parse response: {}", e))
        })?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| LLMError::InvalidResponse("No content in response".to_string()))?
            .to_string();

        let finish_reason = resp_json["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        let tokens_used = resp_json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32;

        // Extract thinking content for O1 models
        let thinking = if is_thinking_model {
            resp_json["choices"][0]["message"]["reasoning"]
                .as_str()
                .map(|s| s.to_string())
        } else {
            None
        };

        let mut response = LLMResponse::new(content, request.model.clone())
            .with_tokens(tokens_used)
            .with_finish_reason(finish_reason);

        if let Some(thinking_content) = thinking {
            response = response.with_thinking(thinking_content);
        }

        // Store in cache
        if let Some(ref cache) = self.cache {
            cache.set(request, response.clone()).await;
        }

        Ok(response)
    }

    fn supports_thinking(&self) -> bool {
        true // Supports O1 and O3 models
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::InMemoryLLMCache;

    #[tokio::test]
    async fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.provider_name(), "OpenAI");
        assert!(provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_provider_with_cache() {
        let cache = Arc::new(InMemoryLLMCache::new());
        let provider = OpenAIProvider::with_cache("test-key".to_string(), cache.clone());
        assert_eq!(provider.name(), "openai");
    }
}
