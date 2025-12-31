//! Google Gemini provider implementation

use crate::cache::LLMCache;
use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::provider::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

/// Google Gemini provider
pub struct GeminiProvider {
    api_key: String,
    base_url: String,
    cache: Option<Arc<dyn LLMCache>>,
    client: Client,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            cache: None,
            client: Client::new(),
        }
    }

    /// Create with cache
    pub fn with_cache(api_key: String, cache: Arc<dyn LLMCache>) -> Self {
        Self {
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            cache: Some(cache),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LLMClient for GeminiProvider {
    async fn call(&self, request: LLMRequest) -> Result<LLMResponse> {
        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(&request).await {
                return Ok(cached);
            }
        }

        // Build contents
        let mut contents = Vec::new();
        if let Some(system) = &request.system {
            contents.push(json!({
                "role": "user",
                "parts": [{"text": system}]
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{"text": "Understood."}]
            }));
        }
        contents.push(json!({
            "role": "user",
            "parts": [{"text": request.prompt}]
        }));

        // Build request body
        let mut body = json!({
            "contents": contents,
        });

        let mut generation_config = serde_json::Map::new();
        if let Some(max_tokens) = request.max_tokens {
            generation_config.insert("maxOutputTokens".to_string(), json!(max_tokens));
        }
        if let Some(temperature) = request.temperature {
            generation_config.insert("temperature".to_string(), json!(temperature));
        }
        if !generation_config.is_empty() {
            body["generationConfig"] = json!(generation_config);
        }

        // Make API call
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, request.model, self.api_key
        );

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ApiCallFailed(format!("Gemini API call failed: {}", e))
            })?;

        let status = resp.status();
        let resp_text = resp.text().await.map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to read response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::ApiCallFailed(format!(
                "Gemini API error ({}): {}",
                status, resp_text
            )));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text).map_err(|e| {
            LLMError::ApiCallFailed(format!("Failed to parse response: {}", e))
        })?;

        let content = resp_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| LLMError::InvalidResponse("No content in response".to_string()))?
            .to_string();

        let finish_reason = resp_json["candidates"][0]["finishReason"]
            .as_str()
            .unwrap_or("STOP")
            .to_string();

        let tokens_used = resp_json["usageMetadata"]["totalTokenCount"]
            .as_u64()
            .unwrap_or(0) as u32;

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
        "gemini"
    }
}

impl LLMProvider for GeminiProvider {
    fn provider_name(&self) -> &str {
        "Gemini"
    }
}
