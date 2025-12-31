//! Complete decision flow generation from natural language descriptions
//!
//! This generator creates a full decision flow including:
//! - Individual rules
//! - Rulesets grouping related rules
//! - Pipeline orchestrating the flow
//! - API configurations if needed

use crate::client::{LLMClient, LLMRequest, LLMResponse};
use crate::error::{LLMError, Result};
use crate::generator::prompt_templates::{DECISION_FLOW_GENERATION_PROMPT, SYSTEM_MESSAGE};
use crate::generator::yaml_extractor::extract_multiple_yaml;
use std::sync::Arc;

/// Configuration for decision flow generation
pub type DecisionFlowGeneratorConfig = crate::generator::rule_generator::RuleGeneratorConfig;

/// Generated decision flow components
#[derive(Debug, Clone)]
pub struct DecisionFlow {
    /// All generated YAML documents
    pub documents: Vec<String>,
    /// Number of rule documents
    pub rule_count: usize,
    /// Number of ruleset documents
    pub ruleset_count: usize,
    /// Number of pipeline documents
    pub pipeline_count: usize,
    /// Number of API config documents
    pub api_config_count: usize,
}

impl DecisionFlow {
    /// Parse documents and count each type
    pub fn from_documents(documents: Vec<String>) -> Self {
        let mut rule_count = 0;
        let mut ruleset_count = 0;
        let mut pipeline_count = 0;
        let mut api_config_count = 0;

        for doc in &documents {
            let trimmed = doc.trim();
            if trimmed.starts_with("rule:") {
                rule_count += 1;
            } else if trimmed.starts_with("ruleset:") {
                ruleset_count += 1;
            } else if trimmed.starts_with("pipeline:") {
                pipeline_count += 1;
            } else if trimmed.starts_with("name:") {
                // API configs start with "name:"
                api_config_count += 1;
            }
        }

        Self {
            documents,
            rule_count,
            ruleset_count,
            pipeline_count,
            api_config_count,
        }
    }

    /// Get all rule documents
    pub fn rules(&self) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|doc| doc.trim().starts_with("rule:"))
            .map(|s| s.as_str())
            .collect()
    }

    /// Get all ruleset documents
    pub fn rulesets(&self) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|doc| doc.trim().starts_with("ruleset:"))
            .map(|s| s.as_str())
            .collect()
    }

    /// Get all pipeline documents
    pub fn pipelines(&self) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|doc| doc.trim().starts_with("pipeline:"))
            .map(|s| s.as_str())
            .collect()
    }

    /// Get all API config documents
    pub fn api_configs(&self) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|doc| doc.trim().starts_with("name:"))
            .map(|s| s.as_str())
            .collect()
    }

    /// Get the full YAML as a single string with document separators
    pub fn to_yaml(&self) -> String {
        self.documents.join("\n---\n")
    }
}

/// Decision flow generator using LLM
pub struct DecisionFlowGenerator {
    client: Arc<dyn LLMClient>,
    config: DecisionFlowGeneratorConfig,
}

impl DecisionFlowGenerator {
    /// Create a new decision flow generator
    pub fn new(client: Arc<dyn LLMClient>, config: DecisionFlowGeneratorConfig) -> Self {
        Self { client, config }
    }

    /// Create with default configuration
    pub fn with_defaults(client: Arc<dyn LLMClient>) -> Self {
        Self {
            client,
            config: DecisionFlowGeneratorConfig::default(),
        }
    }

    /// Generate a complete decision flow from natural language description
    ///
    /// # Arguments
    /// * `description` - High-level description of the decision flow
    ///
    /// # Returns
    /// * `Ok(DecisionFlow)` - Generated decision flow with all components
    /// * `Err(LLMError)` - If generation fails
    ///
    /// # Example
    /// ```no_run
    /// use corint_llm::{DecisionFlowGenerator, MockProvider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> corint_llm::Result<()> {
    /// let provider = Arc::new(MockProvider::new());
    /// let generator = DecisionFlowGenerator::with_defaults(provider);
    ///
    /// let description = r#"
    /// Create a fraud detection system that:
    /// 1. Checks IP reputation using IPInfo API
    /// 2. Blocks high-value transactions from new accounts
    /// 3. Flags velocity anomalies
    /// 4. Routes to manual review if score > 70
    /// "#;
    ///
    /// let flow = generator.generate(description).await?;
    /// println!("Generated {} rules, {} rulesets, {} pipelines",
    ///     flow.rule_count, flow.ruleset_count, flow.pipeline_count);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn generate(&self, description: &str) -> Result<DecisionFlow> {
        let prompt = DECISION_FLOW_GENERATION_PROMPT.replace("{description}", description);

        let request = LLMRequest {
            prompt,
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens.map(|t| t.max(4096)), // Need more tokens for full flow
            temperature: self.config.temperature,
            system: Some(SYSTEM_MESSAGE.to_string()),
            enable_thinking: Some(self.config.enable_thinking),
        };

        let response = self.client.call(request).await?;
        let documents = extract_multiple_yaml(&response.content)?;

        if documents.is_empty() {
            return Err(LLMError::InvalidResponse(
                "No valid YAML documents generated".to_string(),
            ));
        }

        Ok(DecisionFlow::from_documents(documents))
    }

    /// Generate a decision flow and return both the flow and the raw LLM response
    pub async fn generate_with_metadata(
        &self,
        description: &str,
    ) -> Result<(DecisionFlow, LLMResponse)> {
        let prompt = DECISION_FLOW_GENERATION_PROMPT.replace("{description}", description);

        let request = LLMRequest {
            prompt,
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens.map(|t| t.max(4096)),
            temperature: self.config.temperature,
            system: Some(SYSTEM_MESSAGE.to_string()),
            enable_thinking: Some(self.config.enable_thinking),
        };

        let response = self.client.call(request).await?;
        let documents = extract_multiple_yaml(&response.content)?;

        if documents.is_empty() {
            return Err(LLMError::InvalidResponse(
                "No valid YAML documents generated".to_string(),
            ));
        }

        Ok((DecisionFlow::from_documents(documents), response))
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: DecisionFlowGeneratorConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &DecisionFlowGeneratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockProvider;

    #[tokio::test]
    async fn test_generate_complete_flow() {
        let mock_response = r#"rule:
  id: high_amount_check
  description: Check for high amounts
  when:
    all:
      - event.amount > 10000
  signal: review
---
rule:
  id: velocity_check
  description: Check transaction velocity
  when:
    all:
      - count(event.user.id, last_1_hour) > 5
  signal: review
---
ruleset:
  id: fraud_detection
  description: Fraud detection rules
  rules:
    - high_amount_check
    - velocity_check
  strategy: score_sum
---
pipeline:
  id: payment_pipeline
  description: Payment processing
  entry: fraud_check
  steps:
    - step:
        type: ruleset
        id: fraud_check
        ruleset: fraud_detection"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = DecisionFlowGenerator::with_defaults(provider);

        let flow = generator.generate("Fraud detection flow").await.unwrap();

        assert_eq!(flow.rule_count, 2);
        assert_eq!(flow.ruleset_count, 1);
        assert_eq!(flow.pipeline_count, 1);
        assert_eq!(flow.documents.len(), 4);
    }

    #[tokio::test]
    async fn test_decision_flow_methods() {
        let documents = vec![
            "rule:\n  id: rule1".to_string(),
            "rule:\n  id: rule2".to_string(),
            "ruleset:\n  id: rs1".to_string(),
            "pipeline:\n  id: p1".to_string(),
        ];

        let flow = DecisionFlow::from_documents(documents);

        assert_eq!(flow.rule_count, 2);
        assert_eq!(flow.ruleset_count, 1);
        assert_eq!(flow.pipeline_count, 1);

        let rules = flow.rules();
        assert_eq!(rules.len(), 2);

        let rulesets = flow.rulesets();
        assert_eq!(rulesets.len(), 1);

        let pipelines = flow.pipelines();
        assert_eq!(pipelines.len(), 1);
    }

    #[tokio::test]
    async fn test_to_yaml() {
        let documents = vec![
            "rule:\n  id: rule1".to_string(),
            "ruleset:\n  id: rs1".to_string(),
        ];

        let flow = DecisionFlow::from_documents(documents);
        let yaml = flow.to_yaml();

        assert!(yaml.contains("rule:\n  id: rule1"));
        assert!(yaml.contains("---"));
        assert!(yaml.contains("ruleset:\n  id: rs1"));
    }

    #[tokio::test]
    async fn test_generate_with_api_config() {
        let mock_response = r#"name: ipinfo
base_url: https://ipinfo.io
endpoints:
  get_info:
    method: GET
    path: /{ip}
---
rule:
  id: ip_check
  description: Check IP reputation
---
pipeline:
  id: ip_pipeline
  entry: check_ip
  steps: []"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = DecisionFlowGenerator::with_defaults(provider);

        let flow = generator.generate("IP check flow").await.unwrap();

        assert_eq!(flow.api_config_count, 1);
        assert_eq!(flow.rule_count, 1);
        assert_eq!(flow.pipeline_count, 1);

        let api_configs = flow.api_configs();
        assert_eq!(api_configs.len(), 1);
        assert!(api_configs[0].contains("ipinfo"));
    }

    #[tokio::test]
    async fn test_generate_with_metadata() {
        let mock_response = r#"rule:
  id: test_rule
---
ruleset:
  id: test_ruleset
  rules: [test_rule]"#;

        let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
        let generator = DecisionFlowGenerator::with_defaults(provider);

        let (flow, metadata) = generator
            .generate_with_metadata("Test flow")
            .await
            .unwrap();

        assert_eq!(flow.rule_count, 1);
        assert_eq!(flow.ruleset_count, 1);
        assert_eq!(metadata.model, "gpt-4");
        assert!(metadata.tokens_used > 0);
    }
}
