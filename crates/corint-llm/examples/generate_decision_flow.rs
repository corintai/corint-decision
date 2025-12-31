//! Example: Generate a complete decision flow
//!
//! This example demonstrates generating a complete decision flow including:
//! - Multiple rules
//! - Rulesets grouping related rules
//! - Pipeline orchestrating the flow
//! - API configurations if needed
//!
//! Run with:
//! ```bash
//! cargo run --example generate_decision_flow
//! ```

use corint_llm::{DecisionFlowGenerator, MockProvider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CORINT Decision Flow Generator Example ===\n");

    // Create a mock response for a complete fraud detection system
    let mock_response = r#"name: ipinfo
base_url: https://ipinfo.io
auth:
  type: header
  name: Authorization
  value: "{{env.IPINFO_TOKEN}}"
endpoints:
  ip_lookup:
    method: GET
    path: /{ip}
    params:
      ip: event.ip_address
---
rule:
  id: high_amount_check
  description: Flag transactions over $10,000
  when:
    all:
      - event.amount > 10000
  signal: review
  score: 50
  reason: "High transaction amount"
---
rule:
  id: velocity_check
  description: Flag users with more than 5 transactions per hour
  when:
    all:
      - count(event.user.id, last_1_hour) > 5
  signal: review
  score: 30
  reason: "High transaction velocity"
---
rule:
  id: new_account_check
  description: Flag high-value transactions from new accounts
  when:
    all:
      - event.amount > 5000
      - event.account.age_days < 90
  signal: decline
  score: 80
  reason: "High amount from new account"
---
ruleset:
  id: fraud_detection
  description: Comprehensive fraud detection rules
  rules:
    - high_amount_check
    - velocity_check
    - new_account_check
  strategy: score_sum
  default_action:
    signal: approve
    reason: "No fraud signals detected"
---
pipeline:
  id: payment_risk_pipeline
  description: Payment risk assessment pipeline
  entry: ip_check
  steps:
    - id: ip_check
      type: api
      api: ipinfo
      endpoint: ip_lookup
      output: ip_info
      next: fraud_rules
    - id: fraud_rules
      type: ruleset
      ruleset: fraud_detection
      next: risk_router
    - id: risk_router
      type: router
      routes:
        - when: risk_score > 70
          next: decline
        - when: risk_score > 40
          next: review
      default: approve"#;

    let provider = Arc::new(MockProvider::with_response(mock_response.to_string()));
    let generator = DecisionFlowGenerator::with_defaults(provider);

    println!("Generating complete fraud detection flow...\n");

    let description = r#"
    Create a comprehensive fraud detection system that:
    1. Checks IP reputation using IPInfo API
    2. Flags high-value transactions (>$10,000)
    3. Detects velocity anomalies (>5 tx/hour)
    4. Blocks high amounts from new accounts
    5. Routes based on cumulative risk score:
       - Score > 70: Decline
       - Score > 40: Manual review
       - Otherwise: Approve
    "#;

    match generator.generate(description).await {
        Ok(flow) => {
            println!("✓ Successfully generated decision flow!\n");
            println!("Summary:");
            println!("  - {} API configurations", flow.api_config_count);
            println!("  - {} rules", flow.rule_count);
            println!("  - {} rulesets", flow.ruleset_count);
            println!("  - {} pipelines", flow.pipeline_count);
            println!("  - {} total documents\n", flow.documents.len());

            println!("=== Generated YAML ===\n");
            println!("{}", flow.to_yaml());

            println!("\n=== Component Breakdown ===\n");

            if !flow.api_configs().is_empty() {
                println!("API Configurations ({}):", flow.api_configs().len());
                for (i, config) in flow.api_configs().iter().enumerate() {
                    let first_line = config.lines().next().unwrap_or("");
                    println!("  {}. {}", i + 1, first_line);
                }
                println!();
            }

            if !flow.rules().is_empty() {
                println!("Rules ({}):", flow.rules().len());
                for (i, rule) in flow.rules().iter().enumerate() {
                    // Extract rule ID from YAML
                    let id_line = rule.lines().find(|l| l.trim().starts_with("id:"));
                    if let Some(line) = id_line {
                        println!("  {}. {}", i + 1, line.trim());
                    }
                }
                println!();
            }

            if !flow.rulesets().is_empty() {
                println!("Rulesets ({}):", flow.rulesets().len());
                for (i, ruleset) in flow.rulesets().iter().enumerate() {
                    let id_line = ruleset.lines().find(|l| l.trim().starts_with("id:"));
                    if let Some(line) = id_line {
                        println!("  {}. {}", i + 1, line.trim());
                    }
                }
                println!();
            }

            if !flow.pipelines().is_empty() {
                println!("Pipelines ({}):", flow.pipelines().len());
                for (i, pipeline) in flow.pipelines().iter().enumerate() {
                    let id_line = pipeline.lines().find(|l| l.trim().starts_with("id:"));
                    if let Some(line) = id_line {
                        println!("  {}. {}", i + 1, line.trim());
                    }
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("✗ Error generating decision flow: {}", e);
            return Err(e.into());
        }
    }

    println!("=== Example Complete ===\n");
    println!("You can save these generated files to your repository:");
    println!("  - API configs → repository/configs/apis/");
    println!("  - Rules → repository/library/rules/fraud/");
    println!("  - Rulesets → repository/library/rulesets/");
    println!("  - Pipelines → repository/pipelines/");

    Ok(())
}
