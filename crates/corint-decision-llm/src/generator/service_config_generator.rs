//! HTTP service binding generation from specifications or descriptions

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{SERVICE_CONFIG_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_yaml;
use std::sync::Arc;

/// Configuration for HTTP service config generation
pub type ServiceConfigGeneratorConfig = crate::generator::rule_generator::RuleGeneratorConfig;

/// HTTP service binding generator using LLM
pub struct ServiceConfigGenerator {
    client: Arc<dyn LLMClient>,
    config: ServiceConfigGeneratorConfig,
}

impl ServiceConfigGenerator {
    /// Create a new HTTP service config generator
    pub fn new(client: Arc<dyn LLMClient>, config: ServiceConfigGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: ServiceConfigGeneratorConfig::default(),
        }
    }

    /// Generate a CORINT HTTP service binding from description or spec
    ///
    /// # Arguments
    /// * `description` - API specification or natural language description
    ///
    /// # Returns
    /// * `Ok(String)` - Generated YAML HTTP service binding
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_decision_llm::{ServiceConfigGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_decision_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = ServiceConfigGenerator::with_defaults(provider);
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
        let prompt = SERVICE_CONFIG_GENERATION_PROMPT.replace("{description}", description);

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

        // Validate it starts with "name:" (HTTP service configs start with name)
        if !yaml_content.trim().starts_with("name:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'name:'".to_string(),
            ));
        }

        Ok(yaml_content)
    }

    /// Generate an HTTP service config and return both the YAML and the raw LLM response
    pub async fn generate_with_metadata(&self, description: &str) -> Result<(String, LLMResponse)> {
        let prompt = SERVICE_CONFIG_GENERATION_PROMPT.replace("{description}", description);

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
    pub fn set_config(&mut self, config: ServiceConfigGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &ServiceConfigGeneratorConfig {
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
  value: "test-token"
timeout_ms: 5000
operations:
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
        let generator = ServiceConfigGenerator::with_defaults(provider);

        let result = generator
            .generate("IPInfo HTTP service config")
            .await
            .unwrap();

        assert!(result.contains("name: ipinfo"));
        assert!(result.contains("base_url:"));
        assert!(result.contains("operations:"));
    }

    #[tokio::test]
    async fn test_generate_with_multiple_endpoints() {
        let mock_response = r#"name: fraud_api
base_url: https://api.fraud-detection.com
auth:
  type: header
  name: X-API-Key
  value: "{{env.FRAUD_API_KEY}}"
operations:
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
        let generator = ServiceConfigGenerator::with_defaults(provider);

        let result = generator
            .generate("Fraud API with multiple endpoints")
            .await
            .unwrap();

        assert!(result.contains("name: fraud_api"));
        assert!(result.contains("check_transaction"));
        assert!(result.contains("check_user"));
    }

    #[tokio::test]
    async fn test_generate_with_markdown() {
        let mock_response = r#"```yaml
name: test_api
base_url: https://example.com
operations:
  test:
    method: GET
    path: /test
```"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = ServiceConfigGenerator::with_defaults(provider);

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
        let generator = ServiceConfigGenerator::with_defaults(provider);

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
        let generator = ServiceConfigGenerator::with_defaults(provider);

        let (yaml, metadata) = generator.generate_with_metadata("Test API").await.unwrap();

        assert!(yaml.contains("name: test_api"));
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }
}
