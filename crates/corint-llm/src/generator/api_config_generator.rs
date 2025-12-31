//! API configuration generation from specifications or descriptions

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{API_CONFIG_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_yaml;
use std::sync::Arc;

/// Configuration for API config generation
pub type APIConfigGeneratorConfig = crate::generator::rule_generator::RuleGeneratorConfig;

/// API configuration generator using LLM
pub struct APIConfigGenerator {
    client: Arc<dyn LLMClient>,
    config: APIConfigGeneratorConfig,
}

impl APIConfigGenerator {
    /// Create a new API config generator
    pub fn new(client: Arc<dyn LLMClient>, config: APIConfigGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: APIConfigGeneratorConfig::default(),
        }
    }

    /// Generate a CORINT API configuration from description or spec
    ///
    /// # Arguments
    /// * `description` - API specification or natural language description
    ///
    /// # Returns
    /// * `Ok(String)` - Generated YAML API configuration
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_llm::{APIConfigGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = APIConfigGenerator::with_defaults(provider);
    ///
    /// let description = r#"
    /// API: IPInfo
    /// Base URL: https://ipinfo.io
    /// Endpoint: GET /{ip}
    /// Auth: Bearer token in header
    /// Response: JSON with country, city, org fields
    /// "#;
    /// let api_yaml = generator.generate(description).await?;
    /// println!("{}", api_yaml);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate(&self, description: &str) -> Result<String> {
        let prompt = API_CONFIG_GENERATION_PROMPT.replace("{description}", description);

        let request = LLMRequest {
            prompt,
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: Some(SYSTEM_MESSAGE.to_string()),
            enable_thinking: Some(self.config.enable_thinking),
        };

        let response = self.client.call(request).await?;
        let yaml_content = extract_yaml(&response.content)?;

        // Validate it starts with "name:" (API configs start with name)
        if !yaml_content.trim().starts_with("name:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'name:'".to_string(),
            ));
        }

        Ok(yaml_content)
    }

    /// Generate an API config and return both the YAML and the raw LLM response
    pub async fn generate_with_metadata(
        &self,
        description: &str,
    ) -> Result<(String, LLMResponse)> {
        let prompt = API_CONFIG_GENERATION_PROMPT.replace("{description}", description);

        let request = LLMRequest {
            prompt,
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: Some(SYSTEM_MESSAGE.to_string()),
            enable_thinking: Some(self.config.enable_thinking),
        };

        let response = self.client.call(request).await?;
        let yaml_content = extract_yaml(&response.content)?;

        if !yaml_content.trim().starts_with("name:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'name:'".to_string(),
            ));
        }

        Ok((yaml_content, response))
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: APIConfigGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &APIConfigGeneratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;

    #[tokio::test]
    async fn test_generate_simple_api_config() {
        let mock_response = r#"name: ipinfo
base_url: https://ipinfo.io
auth:
  type: header
  name: Authorization
  value: "{{env.IPINFO_TOKEN}}"
timeout_ms: 5000
endpoints:
  get_info:
    method: GET
    path: /{ip}
    params:
      ip: event.ip_address
    response:
      mapping:
        country: country
        city: city
        org: org"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = APIConfigGenerator::with_defaults(provider);

        let result = generator.generate("IPInfo API config").await.unwrap();

        assert!(result.contains("name: ipinfo"));
        assert!(result.contains("base_url:"));
        assert!(result.contains("endpoints:"));
    }

    #[tokio::test]
    async fn test_generate_with_multiple_endpoints() {
        let mock_response = r#"name: fraud_api
base_url: https://api.fraud-detection.com
auth:
  type: header
  name: X-API-Key
  value: "{{env.FRAUD_API_KEY}}"
endpoints:
  check_transaction:
    method: POST
    path: /v1/transactions/check
    params:
      amount: event.amount
      user_id: event.user.id
  check_user:
    method: GET
    path: /v1/users/{user_id}/risk
    params:
      user_id: event.user.id"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = APIConfigGenerator::with_defaults(provider);

        let result = generator.generate("Fraud API with multiple endpoints").await.unwrap();

        assert!(result.contains("name: fraud_api"));
        assert!(result.contains("check_transaction"));
        assert!(result.contains("check_user"));
    }

    #[tokio::test]
    async fn test_generate_with_markdown() {
        let mock_response = r#"```yaml
name: test_api
base_url: https://example.com
endpoints:
  test:
    method: GET
    path: /test
```"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = APIConfigGenerator::with_defaults(provider);

        let result = generator.generate("Test API").await.unwrap();

        assert!(result.contains("name: test_api"));
        assert!(!result.contains("```"));
    }

    #[tokio::test]
    async fn test_wrong_yaml_type_error() {
        let mock_response = r#"rule:
  id: not_an_api
  description: This is a rule"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = APIConfigGenerator::with_defaults(provider);

        let result = generator.generate("Test").await;
        assert!(result.is_err());
        if let Err(LLMError::InvalidResponse(msg)) = result {
            assert!(msg.contains("does not start with 'name:'"));
        }
    }

    #[tokio::test]
    async fn test_generate_with_metadata() {
        let mock_response = r#"name: test_api
base_url: https://example.com"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = APIConfigGenerator::with_defaults(provider);

        let (yaml, metadata) = generator
            .generate_with_metadata("Test API")
            .await
            .unwrap();

        assert!(yaml.contains("name: test_api"));
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }
}
