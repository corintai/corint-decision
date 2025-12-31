//! Anthropic provider implementation

use crate::cache::LLMCache;
use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::provider::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

/// Anthropic provider (supports Claude with extended thinking)
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    cache: Option<Arc<dyn LLMCache>>,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            cache: None,
            client: Client::new(),
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            cache: Some(cache),
            client: Client::new(),
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

        // Build request body
        let mut body = json!({
            "model": request.model,
            "messages": [{
                "role": "user",
                "content": request.prompt
            }],
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }

        // Enable extended thinking if requested
        if request.enable_thinking.unwrap_or(false) {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": 10000
            });
        }

        // Make API call
        let resp = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ApiCallFailed(format!("Anthropic API call failed: {}", e))
            })?;

        let status = resp.status();
        let resp_text = resp.text().await.map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::ApiCallFailed(format!(
                "Anthropic API error ({}): {}",
                status, resp_text
            )));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text).map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to parse response: {}", e))
        })?;

        // Extract content blocks
        let content_blocks = resp_json["content"].as_array().ok_or_else(|| {
            LLMError::InvalidResponse("No content in response".to_string())
        })?;

        let mut main_content = String::new();
        let mut thinking_content = None;

        for block in content_blocks {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(text) = block["text"].as_str() {
                        main_content.push_str(text);
                    }
                }
                Some("thinking") => {
                    if let Some(thinking) = block["thinking"].as_str() {
                        thinking_content = Some(thinking.to_string());
                    }
                }
                _ => {}
            }
        }

        let finish_reason = resp_json["stop_reason"]
            .as_str()
            .unwrap_or("end_turn")
            .to_string();

        let tokens_used = resp_json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
            + resp_json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

        let mut response = LLMResponse::new(main_content, request.model.clone())
            .with_tokens(tokens_used)
            .with_finish_reason(finish_reason);

        if let Some(thinking) = thinking_content {
            response = response.with_thinking(thinking);
        }

        // Store in cache
        if let Some(ref cache) = self.cache {
            cache.set(request, response.clone()).await;
        }

        Ok(response)
    }

    fn supports_thinking(&self) -> bool {
        true // Claude supports extended thinking
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
