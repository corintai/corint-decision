//! Example: Generate a CORINT rule from natural language description
//!
//! This example demonstrates how to use the RuleGenerator to create
//! CORINT rule YAML configurations from natural language descriptions.
//!
//! Run with:
//! ```bash
//! cargo run --example generate_rule
//! ```

use corint_llm::{MockProvider, RuleGenerator, RuleGeneratorConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CORINT Rule Generator Example ===\n");

    // For this example, we use MockProvider to avoid needing API keys
    // In production, you would use OpenAIProvider, AnthropicProvider, etc.
    let mock_yaml = r#"rule:
  id: high_value_transaction_block
  description: Block high-value transactions from new accounts
  when:
    all:
      - event.transaction.amount > 10000
      - event.account.age_days < 90
  signal: decline
  reason: "High-value transaction from new account - requires manual review"
  score: 100"#;

    let provider = Arc::new(MockProvider::with_response(mock_yaml.to_string()));

    // Create generator with custom configuration
    let config = RuleGeneratorConfig::new("gpt-4")
        .with_temperature(0.3) // Lower temperature for more consistent output
        .with_max_tokens(2048);

    let generator = RuleGenerator::new(provider, config);

    // Example 1: Simple rule generation
    println!("Example 1: Generating a fraud detection rule\n");
    let description = "Block transactions over $10,000 from accounts less than 90 days old";

    match generator.generate(description).await {
        Ok(yaml) => {
            println!("Generated Rule:");
            println!("{}\n", yaml);
        }
        Err(e) => {
            eprintln!("Error generating rule: {}", e);
        }
    }

    // Example 2: Generate with metadata (useful for debugging)
    println!("\nExample 2: Generating with metadata\n");
    let description2 = "Flag users who make more than 5 purchases per hour";

    match generator.generate_with_metadata(description2).await {
        Ok((yaml, metadata)) => {
            println!("Generated Rule:");
            println!("{}\n", yaml);
            println!("Generation Metadata:");
            println!("  Model: {}", metadata.model);
            println!("  Tokens: {}", metadata.tokens_used);
            println!("  Finish Reason: {}", metadata.finish_reason);
            if let Some(thinking) = metadata.thinking {
                println!("  Thinking Process:\n{}", thinking);
            }
        }
        Err(e) => {
            eprintln!("Error generating rule: {}", e);
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nNote: This example uses MockProvider for demonstration.");
    println!("In production, use a real LLM provider:");
    println!("  - OpenAIProvider for GPT-4, GPT-4o, O1, O3");
    println!("  - AnthropicProvider for Claude (with extended thinking)");
    println!("  - GeminiProvider for Gemini models");
    println!("  - DeepSeekProvider for DeepSeek models");

    Ok(())
}
