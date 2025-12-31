//! Ruleset generation from natural language descriptions

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{RULESET_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_yaml;
use std::sync::Arc;

/// Configuration for ruleset generation
pub type RulesetGeneratorConfig = crate::generator::rule_generator::RuleGeneratorConfig;

/// Ruleset generator using LLM
pub struct RulesetGenerator {
    client: Arc<dyn LLMClient>,
    config: RulesetGeneratorConfig,
}

impl RulesetGenerator {
    /// Create a new ruleset generator
    pub fn new(client: Arc<dyn LLMClient>, config: RulesetGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: RulesetGeneratorConfig::default(),
        }
    }

    /// Generate a CORINT ruleset from natural language description
    ///
    /// # Arguments
    /// * `description` - Natural language description of the ruleset
    ///
    /// # Returns
    /// * `Ok(String)` - Generated YAML ruleset configuration
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_llm::{RulesetGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = RulesetGenerator::with_defaults(provider);
    ///
    /// let description = "Create a fraud detection ruleset that includes high amount checks and velocity checks";
    /// let ruleset_yaml = generator.generate(description).await?;
    /// println!("{}", ruleset_yaml);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate(&self, description: &str) -> Result<String> {
        let prompt = RULESET_GENERATION_PROMPT.replace("{description}", description);

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

        // Validate it starts with "ruleset:"
        if !yaml_content.trim().starts_with("ruleset:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'ruleset:'".to_string(),
            ));
        }

        Ok(yaml_content)
    }

    /// Generate a ruleset and return both the YAML and the raw LLM response
    pub async fn generate_with_metadata(
        &self,
        description: &str,
    ) -> Result<(String, LLMResponse)> {
        let prompt = RULESET_GENERATION_PROMPT.replace("{description}", description);

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

        if !yaml_content.trim().starts_with("ruleset:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'ruleset:'".to_string(),
            ));
        }

        Ok((yaml_content, response))
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: RulesetGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &RulesetGeneratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;

    #[tokio::test]
    async fn test_generate_simple_ruleset() {
        let mock_response = r#"ruleset:
  id: fraud_detection
  description: Fraud detection rules
  rules:
    - high_amount_check
    - velocity_check
  strategy: first_match
  default_action:
    signal: approve
    reason: "No fraud signals detected""#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RulesetGenerator::with_defaults(provider);

        let result = generator
            .generate("Fraud detection ruleset")
            .await
            .unwrap();

        assert!(result.contains("ruleset:"));
        assert!(result.contains("fraud_detection"));
        assert!(result.contains("rules:"));
    }

    #[tokio::test]
    async fn test_generate_with_markdown() {
        let mock_response = r#"Here's your ruleset:

```yaml
ruleset:
  id: payment_validation
  description: Payment validation rules
  rules:
    - amount_check
    - country_check
  strategy: all_match
```

This ruleset validates payments."#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RulesetGenerator::with_defaults(provider);

        let result = generator
            .generate("Payment validation ruleset")
            .await
            .unwrap();

        assert!(result.contains("ruleset:"));
        assert!(result.contains("payment_validation"));
        assert!(!result.contains("Here's your ruleset"));
        assert!(!result.contains("```"));
    }

    #[tokio::test]
    async fn test_wrong_yaml_type_error() {
        let mock_response = r#"rule:
  id: not_a_ruleset
  description: This is a rule"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RulesetGenerator::with_defaults(provider);

        let result = generator.generate("Test").await;
        assert!(result.is_err());
        if let Err(LLMError::InvalidResponse(msg)) = result {
            assert!(msg.contains("does not start with 'ruleset:'"));
        }
    }

    #[tokio::test]
    async fn test_generate_with_metadata() {
        let mock_response = r#"ruleset:
  id: test_ruleset
  description: Test
  rules:
    - rule1"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = RulesetGenerator::with_defaults(provider);

        let (yaml, metadata) = generator
            .generate_with_metadata("Test ruleset")
            .await
            .unwrap();

        assert!(yaml.contains("ruleset:"));
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }
}
