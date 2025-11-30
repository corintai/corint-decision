//! LLM response caching

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::llm::client::{LLMRequest, LLMResponse};

/// LLM cache trait
#[async_trait]
pub trait LLMCache: Send + Sync {
    /// Get a cached response
    async fn get(&self, request: &LLMRequest) -> Option<LLMResponse>;

    /// Store a response in cache
    async fn set(&self, request: LLMRequest, response: LLMResponse);

    /// Clear the cache
    async fn clear(&self);
}

/// In-memory LLM cache implementation
pub struct InMemoryLLMCache {
    cache: Arc<RwLock<HashMap<String, LLMResponse>>>,
}

impl InMemoryLLMCache {
    /// Create a new in-memory cache
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate cache key from request
    fn cache_key(request: &LLMRequest) -> String {
        // Simple hash-based key using prompt and model
        // In production, consider using a proper hash function
        format!(
            "{}:{}:{}:{}",
            request.model,
            request.prompt,
            request.max_tokens.unwrap_or(0),
            request.temperature.unwrap_or(0.0)
        )
    }
}

impl Default for InMemoryLLMCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMCache for InMemoryLLMCache {
    async fn get(&self, request: &LLMRequest) -> Option<LLMResponse> {
        let key = Self::cache_key(request);
        self.cache.read().unwrap().get(&key).cloned()
    }

    async fn set(&self, request: LLMRequest, response: LLMResponse) {
        let key = Self::cache_key(&request);
        self.cache.write().unwrap().insert(key, response);
    }

    async fn clear(&self) {
        self.cache.write().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_get_set() {
        let cache = InMemoryLLMCache::new();

        let request = LLMRequest::new("Test prompt".to_string(), "gpt-4".to_string());
        let response = LLMResponse::new("Test response".to_string(), "gpt-4".to_string());

        // Initially empty
        assert!(cache.get(&request).await.is_none());

        // Set and retrieve
        cache.set(request.clone(), response.clone()).await;
        let cached = cache.get(&request).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().content, "Test response");
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = InMemoryLLMCache::new();

        let request = LLMRequest::new("Test".to_string(), "gpt-4".to_string());
        let response = LLMResponse::new("Response".to_string(), "gpt-4".to_string());

        cache.set(request.clone(), response).await;
        assert!(cache.get(&request).await.is_some());

        cache.clear().await;
        assert!(cache.get(&request).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_key_uniqueness() {
        let cache = InMemoryLLMCache::new();

        let request1 = LLMRequest::new("Prompt 1".to_string(), "gpt-4".to_string());
        let request2 = LLMRequest::new("Prompt 2".to_string(), "gpt-4".to_string());

        let response1 = LLMResponse::new("Response 1".to_string(), "gpt-4".to_string());
        let response2 = LLMResponse::new("Response 2".to_string(), "gpt-4".to_string());

        cache.set(request1.clone(), response1).await;
        cache.set(request2.clone(), response2).await;

        let cached1 = cache.get(&request1).await.unwrap();
        let cached2 = cache.get(&request2).await.unwrap();

        assert_eq!(cached1.content, "Response 1");
        assert_eq!(cached2.content, "Response 2");
    }
}
