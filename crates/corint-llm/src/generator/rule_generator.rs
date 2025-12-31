//! Rule generation from natural language descriptions

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{RULE_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_yaml;
use std::sync::Arc;

/// Configuration for rule generation
#[derive(Debug, Clone)]
pub struct RuleGeneratorConfig {
    /// Model to use for generation (e.g., "gpt-4", "claude-3-opus")
    pub model: String,
    /// Maximum tokens for response
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 1.0, lower = more deterministic)
    pub temperature: Option<f32>,
    /// Enable extended thinking for supported models
    pub enable_thinking: bool,
}

impl Default for RuleGeneratorConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.3), // Lower temperature for more consistent YAML generation
            enable_thinking: false,
        }
    }
}

impl RuleGeneratorConfig {
    /// Create a new configuration with a specific model
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Enable extended thinking
    pub fn with_thinking(mut self, enable: bool) -> Self {
        self.enable_thinking = enable;
        self
    }
}

/// Rule generator using LLM
pub struct RuleGenerator {
    client: Arc<dyn LLMClient>,
    config: RuleGeneratorConfig,
}

impl RuleGenerator {
    /// Create a new rule generator
    pub fn new(client: Arc<dyn LLMClient>, config: RuleGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: RuleGeneratorConfig::default(),
        }
    }

    /// Generate a CORINT rule from natural language description
    ///
    /// # Arguments
    /// * `description` - Natural language description of the rule
    ///
    /// # Returns
    /// * `Ok(String)` - Generated YAML rule configuration
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_llm::{RuleGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = RuleGenerator::with_defaults(provider);
    ///
    /// let description = "Block transactions over $10,000 from new users";
    /// let rule_yaml = generator.generate(description).await?;
    /// println!("{}", rule_yaml);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate(&self, description: &str) -> Result<String> {
        // Build prompt by replacing {description} placeholder
        let prompt = RULE_GENERATION_PROMPT.replace("{description}", description);

        // Create LLM request
        let request = LLMRequest {
            prompt,
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: Some(SYSTEM_MESSAGE.to_string()),
            enable_thinking: Some(self.config.enable_thinking),
        };

        // Call LLM
        let response = self.client.call(request).await?;

        // Extract YAML from response
        let yaml_content = extract_yaml(&response.content)?;

        // Validate it starts with "rule:"
        if !yaml_content.trim().starts_with("rule:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'rule:'".to_string(),
            ));
        }

        Ok(yaml_content)
    }

    /// Generate a rule and return both the YAML and the raw LLM response
    ///
    /// Useful for debugging or analyzing the LLM's thinking process
    pub async fn generate_with_metadata(
        &self,
        description: &str,
    ) -> Result<(String, LLMResponse)> {
        let prompt = RULE_GENERATION_PROMPT.replace("{description}", description);

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

        if !yaml_content.trim().starts_with("rule:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'rule:'".to_string(),
            ));
        }

        Ok((yaml_content, response))
    }

    /// Update the model configuration
    pub fn set_config(&mut self, config: RuleGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &RuleGeneratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;

    #[tokio::test]
    async fn test_generate_simple_rule() {
        let mock_response = r#"rule:
  id: high_amount_block
  description: Block transactions over $10,000
  when:
    all:
      - event.amount > 10000
  signal: decline
  reason: "Amount exceeds limit""#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RuleGenerator::with_defaults(provider);

        let result = generator
            .generate("Block transactions over $10,000")
            .await
            .unwrap();

        assert!(result.contains("rule:"));
        assert!(result.contains("high_amount_block"));
    }

    #[tokio::test]
    async fn test_generate_with_markdown() {
        let mock_response = r#"Here's your rule:

```yaml
rule:
  id: velocity_check
  description: Check transaction velocity
  when:
    all:
      - count(event.user.id, last_1_hour) > 5
  signal: review
```

This rule checks velocity."#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RuleGenerator::with_defaults(provider);

        let result = generator
            .generate("Flag users with more than 5 transactions per hour")
            .await
            .unwrap();

        assert!(result.contains("rule:"));
        assert!(result.contains("velocity_check"));
        assert!(!result.contains("Here's your rule"));
        assert!(!result.contains("```"));
    }

    #[tokio::test]
    async fn test_generate_with_metadata() {
        let mock_response = r#"rule:
  id: test_rule
  description: Test"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RuleGenerator::with_defaults(provider);

        let (yaml, metadata) = generator
            .generate_with_metadata("Test rule")
            .await
            .unwrap();

        assert!(yaml.contains("rule:"));
        // The MockProvider echoes back the request model, which is "gpt-4" from default config
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }

    #[tokio::test]
    async fn test_invalid_response_error() {
        let mock_response = "This is not a valid YAML rule";
        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RuleGenerator::with_defaults(provider);

        let result = generator.generate("Test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wrong_yaml_type_error() {
        // Valid YAML but not a rule
        let mock_response = r#"pipeline:
  id: not_a_rule
  description: This is a pipeline"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RuleGenerator::with_defaults(provider);

        let result = generator.generate("Test").await;
        assert!(result.is_err());
        if let Err(LLMError::InvalidResponse(msg)) = result {
            assert!(msg.contains("does not start with 'rule:'"));
        }
    }

    #[tokio::test]
    async fn test_custom_config() {
        let mock_response = r#"rule:
  id: custom_rule
  description: Custom config test"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let config = RuleGeneratorConfig::new("custom-model")
            .with_max_tokens(4096)
            .with_temperature(0.7)
            .with_thinking(true);

        let generator = RuleGenerator::new(provider, config);

        assert_eq!(generator.config().model, "custom-model");
        assert_eq!(generator.config().max_tokens, Some(4096));
        assert_eq!(generator.config().temperature, Some(0.7));
        assert!(generator.config().enable_thinking);
    }
}
