//! Pipeline generation from natural language descriptions

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{PIPELINE_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_yaml;
use std::sync::Arc;

/// Configuration for pipeline generation
pub type PipelineGeneratorConfig = crate::generator::rule_generator::RuleGeneratorConfig;

/// Pipeline generator using LLM
pub struct PipelineGenerator {
    client: Arc<dyn LLMClient>,
    config: PipelineGeneratorConfig,
}

impl PipelineGenerator {
    /// Create a new pipeline generator
    pub fn new(client: Arc<dyn LLMClient>, config: PipelineGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: PipelineGeneratorConfig::default(),
        }
    }

    /// Generate a CORINT pipeline from natural language description
    ///
    /// # Arguments
    /// * `description` - Natural language description of the pipeline
    ///
    /// # Returns
    /// * `Ok(String)` - Generated YAML pipeline configuration
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_llm::{PipelineGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = PipelineGenerator::with_defaults(provider);
    ///
    /// let description = "Create a payment pipeline that checks IP info, runs fraud rules, then routes based on score";
    /// let pipeline_yaml = generator.generate(description).await?;
    /// println!("{}", pipeline_yaml);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate(&self, description: &str) -> Result<String> {
        let prompt = PIPELINE_GENERATION_PROMPT.replace("{description}", description);

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

        // Validate it starts with "pipeline:"
        if !yaml_content.trim().starts_with("pipeline:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'pipeline:'".to_string(),
            ));
        }

        Ok(yaml_content)
    }

    /// Generate a pipeline and return both the YAML and the raw LLM response
    pub async fn generate_with_metadata(
        &self,
        description: &str,
    ) -> Result<(String, LLMResponse)> {
        let prompt = PIPELINE_GENERATION_PROMPT.replace("{description}", description);

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

        if !yaml_content.trim().starts_with("pipeline:") {
            return Err(LLMError::InvalidResponse(
                "Generated YAML does not start with 'pipeline:'".to_string(),
            ));
        }

        Ok((yaml_content, response))
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: PipelineGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &PipelineGeneratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;

    #[tokio::test]
    async fn test_generate_simple_pipeline() {
        let mock_response = r#"pipeline:
  id: payment_pipeline
  description: Payment processing pipeline
  entry: check_ip
  steps:
    - id: check_ip
      type: api
      api: ipinfo
      endpoint: get_info
      output: ip_info
      next: fraud_check
    - id: fraud_check
      type: ruleset
      ruleset: fraud_detection"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = PipelineGenerator::with_defaults(provider);

        let result = generator
            .generate("Payment processing pipeline")
            .await
            .unwrap();

        assert!(result.contains("pipeline:"));
        assert!(result.contains("payment_pipeline"));
        assert!(result.contains("steps:"));
    }

    #[tokio::test]
    async fn test_generate_with_router() {
        let mock_response = r#"pipeline:
  id: conditional_pipeline
  description: Pipeline with conditional routing
  entry: step1
  steps:
    - id: step1
      type: ruleset
      ruleset: initial_check
      next: router
    - id: router
      type: router
      routes:
        - when: score > 80
          next: decline_step
        - when: score < 20
          next: approve_step
      default: review_step"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = PipelineGenerator::with_defaults(provider);

        let result = generator
            .generate("Pipeline with conditional routing")
            .await
            .unwrap();

        assert!(result.contains("pipeline:"));
        assert!(result.contains("router"));
    }

    #[tokio::test]
    async fn test_generate_with_markdown() {
        let mock_response = r#"```yaml
pipeline:
  id: test_pipeline
  description: Test
  entry: step1
  steps:
    - step:
        type: api
        id: step1
```"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = PipelineGenerator::with_defaults(provider);

        let result = generator.generate("Test pipeline").await.unwrap();

        assert!(result.contains("pipeline:"));
        assert!(!result.contains("```"));
    }

    #[tokio::test]
    async fn test_wrong_yaml_type_error() {
        let mock_response = r#"rule:
  id: not_a_pipeline
  description: This is a rule"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = PipelineGenerator::with_defaults(provider);

        let result = generator.generate("Test").await;
        assert!(result.is_err());
        if let Err(LLMError::InvalidResponse(msg)) = result {
            assert!(msg.contains("does not start with 'pipeline:'"));
        }
    }

    #[tokio::test]
    async fn test_generate_with_metadata() {
        let mock_response = r#"pipeline:
  id: test_pipeline
  description: Test
  entry: step1
  steps: []"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = PipelineGenerator::with_defaults(provider);

        let (yaml, metadata) = generator
            .generate_with_metadata("Test pipeline")
            .await
            .unwrap();

        assert!(yaml.contains("pipeline:"));
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }
}
