//! Example: Generate CORINT rules using OpenAI GPT-4
//!
//! This example demonstrates using a real LLM provider (OpenAI) to generate
//! CORINT rule configurations from natural language descriptions.
//!
//! Prerequisites:
//! - Set the OPENAI_API_KEY environment variable
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY=your-api-key
//! cargo run --example generate_with_openai
//! ```

use corint_llm::{OpenAIProvider, RuleGenerator, RuleGeneratorConfig, InMemoryLLMCache};
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CORINT Rule Generator with OpenAI ===\n");

    // Get API key from environment
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set it with: export OPENAI_API_KEY=your-api-key");
        std::process::exit(1);
    });

    // Create OpenAI provider with caching
    let cache = Arc::new(InMemoryLLMCache::new());
    let provider = Arc::new(OpenAIProvider::with_cache(api_key, cache));

    // Create generator with GPT-4
    let config = RuleGeneratorConfig::new("gpt-4")
        .with_temperature(0.3)
        .with_max_tokens(2048);

    let generator = RuleGenerator::new(provider, config);

    // Example rule descriptions
    let examples = vec![
        "Block transactions over $10,000 from accounts less than 3 months old",
        "Flag users who have made more than 5 failed login attempts in the last hour",
        "Approve transactions under $100 from verified accounts",
        "Review transactions from new IP addresses for existing users",
    ];

    println!("Generating {} rules...\n", examples.len());

    for (i, description) in examples.iter().enumerate() {
        println!("Rule {} - Description:", i + 1);
        println!("  {}\n", description);

        match generator.generate(description).await {
            Ok(yaml) => {
                println!("Generated YAML:");
                println!("{}", yaml);
                println!("\n{}\n", "-".repeat(80));
            }
            Err(e) => {
                eprintln!("Error generating rule {}: {}\n", i + 1, e);
            }
        }
    }

    println!("\n=== Generation Complete ===");
    println!("\nThe generated rules can be saved to your repository:");
    println!("  repository/library/rules/generated/");
    println!("\nYou can then reference them in rulesets and pipelines.");

    Ok(())
}
