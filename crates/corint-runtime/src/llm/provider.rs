//! LLM provider implementations for standard and thinking models

use async_trait::async_trait;
use crate::error::{Result, RuntimeError};
use crate::llm::client::{LLMClient, LLMRequest, LLMResponse};
use crate::llm::cache::LLMCache;
use std::sync::Arc;
use reqwest::Client;
use serde_json::json;

/// LLM provider trait
pub trait LLMProvider: LLMClient {
    /// Get the provider name
    fn provider_name(&self) -> &str;
}

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
            let thinking = self.default_thinking.clone()
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
        let is_thinking_model = request.model.starts_with("o1-") || request.model.starts_with("o3-");

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
        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("OpenAI API call failed: {}", e)))?;

        let status = resp.status();
        let resp_text = resp.text().await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::ExternalCallFailed(
                format!("OpenAI API error ({}): {}", status, resp_text)
            ));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to parse response: {}", e)))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| RuntimeError::ExternalCallFailed("No content in response".to_string()))?
            .to_string();

        let finish_reason = resp_json["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        let tokens_used = resp_json["usage"]["total_tokens"]
            .as_u64()
            .unwrap_or(0) as u32;

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
        let resp = self.client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Anthropic API call failed: {}", e)))?;

        let status = resp.status();
        let resp_text = resp.text().await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::ExternalCallFailed(
                format!("Anthropic API error ({}): {}", status, resp_text)
            ));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to parse response: {}", e)))?;

        // Extract content blocks
        let content_blocks = resp_json["content"]
            .as_array()
            .ok_or_else(|| RuntimeError::ExternalCallFailed("No content in response".to_string()))?;

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

        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Gemini API call failed: {}", e)))?;

        let status = resp.status();
        let resp_text = resp.text().await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::ExternalCallFailed(
                format!("Gemini API error ({}): {}", status, resp_text)
            ));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to parse response: {}", e)))?;

        let content = resp_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| RuntimeError::ExternalCallFailed("No content in response".to_string()))?
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
        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("DeepSeek API call failed: {}", e)))?;

        let status = resp.status();
        let resp_text = resp.text().await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::ExternalCallFailed(
                format!("DeepSeek API error ({}): {}", status, resp_text)
            ));
        }

        // Parse response
        let resp_json: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to parse response: {}", e)))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| RuntimeError::ExternalCallFailed("No content in response".to_string()))?
            .to_string();

        let finish_reason = resp_json["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        let tokens_used = resp_json["usage"]["total_tokens"]
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
        "deepseek"
    }
}

impl LLMProvider for DeepSeekProvider {
    fn provider_name(&self) -> &str {
        "DeepSeek"
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
        assert!(provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_mock_provider_with_thinking() {
        let provider = MockProvider::new();
        let request = LLMRequest::new("Test".to_string(), "mock-model".to_string())
            .with_thinking(true);

        let response = provider.call(request).await.unwrap();
        assert!(response.thinking.is_some());
        assert_eq!(response.thinking.unwrap(), "Mock thinking process...");
    }

    #[tokio::test]
    async fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.provider_name(), "OpenAI");
        assert!(provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.provider_name(), "Anthropic");
        assert!(provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_gemini_provider_creation() {
        let provider = GeminiProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.provider_name(), "Gemini");
        assert!(!provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_deepseek_provider_creation() {
        let provider = DeepSeekProvider::new("test-key".to_string());
        assert_eq!(provider.name(), "deepseek");
        assert_eq!(provider.provider_name(), "DeepSeek");
        assert!(!provider.supports_thinking());
    }

    #[tokio::test]
    async fn test_provider_with_cache() {
        let cache = Arc::new(InMemoryLLMCache::new());
        let provider = OpenAIProvider::with_cache("test-key".to_string(), cache.clone());
        assert_eq!(provider.name(), "openai");
    }
}
