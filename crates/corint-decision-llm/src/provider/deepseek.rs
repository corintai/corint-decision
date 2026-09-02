//! DeepSeek provider implementation

use crate::cache::LLMCache;
use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::provider::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

/// DeepSeek provider (OpenAI-compatible API)
pub struct DeepSeekProvider {
    api_key: String,
    base_url: String,
    cache: Option<Arc<dyn LLMCache>>,
    client: Client,
}

impl DeepSeekProvider {
    /// Create a new DeepSeek provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.deepseek.com/v1".to_string(),
            cache: None,
            client: Client::new(),
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            base_url: "https://api.deepseek.com/v1".to_string(),
            cache: Some(cache),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LLMClient for DeepSeekProvider {
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(&request).await {
                return Ok(cached);
            }
        }

        // Build messages (DeepSeek uses OpenAI-compatible API)
        let mut messages = Vec::new();
        if let Some(system) = &request.system {
            messages.push(json!({
                "role": "system",
                "content": system
            }));
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

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
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
                LLMError::ApiCallFailed(format!("DeepSeek API call failed: {}", e))
            })?;

        let status = resp.status();
        let resp_text = resp.text().await.map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::ApiCallFailed(format!(
                "DeepSeek API error ({}): {}",
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

        let response = LLMResponse::new(content, request.model.clone())
            .with_tokens(tokens_used)
            .with_finish_reason(finish_reason);

        // Store in cache
        if let Some(ref cache) = self.cache {
            cache.set(request, response.clone()).await;
        }

        Ok(response)
    }

    fn name(&self) -> &str {
        "deepseek"
    }
}

impl LLMProvider for DeepSeekProvider {
    fn provider_name(&self) -> &str {
        "DeepSeek"
    }
}
