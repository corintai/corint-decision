//! Example: Generate a complete pipeline using DeepSeek
//!
//! This example demonstrates how to use the DeepSeek LLM provider to generate
//! a complete CORINT pipeline from a natural language description.
//!
//! The API key is read from config/server.yaml file.
//!
//! Usage:
//! ```bash
//! cargo run --example generate_pipeline_deepseek "Create a fraud detection pipeline for high-risk transactions"
//! ```

use corint_llm::{DeepSeekProvider, PipelineGenerator, PipelineGeneratorConfig};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct DeepSeekConfig {
    api_key: String,
    #[serde(default = "default_model")]
    default_model: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
    #[serde(default = "default_temperature")]
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct LlmConfig {
    deepseek: DeepSeekConfig,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    llm: LlmConfig,
}

fn default_model() -> String {
    "deepseek-chat".to_string()
}

fn default_max_tokens() -> u32 {
    2048
}

fn default_temperature() -> f32 {
    0.3
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get pipeline description from command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example generate_pipeline_deepseek <description>");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  cargo run --example generate_pipeline_deepseek \"Create a fraud detection pipeline for transactions\"");
        eprintln!();
        eprintln!("The DeepSeek API key is read from config/server.yaml");
        std::process::exit(1);
    }

    let description = args[1..].join(" ");

    // Load configuration from config/server.yaml
    let config_content = std::fs::read_to_string("config/server.yaml").unwrap_or_else(|_| {
        eprintln!("Error: Could not read config/server.yaml");
        eprintln!("Please ensure the file exists and contains DeepSeek API configuration");
        std::process::exit(1);
    });

    let config: ServerConfig = serde_yaml::from_str(&config_content).unwrap_or_else(|e| {
        eprintln!("Error: Failed to parse config/server.yaml: {}", e);
        std::process::exit(1);
    });

    let deepseek_config = &config.llm.deepseek;
    let api_key = &deepseek_config.api_key;

    println!("Generating pipeline with DeepSeek...");
    println!("Model: {}", deepseek_config.default_model);
    println!("Description: {}\n", description);

    // Create DeepSeek provider
    let provider = Arc::new(DeepSeekProvider::new(api_key.clone()));

    // Create pipeline generator with configuration from server.yaml
    let generator_config = PipelineGeneratorConfig::new(&deepseek_config.default_model)
        .with_max_tokens(deepseek_config.max_tokens)
        .with_temperature(deepseek_config.temperature);

    let generator = PipelineGenerator::new(provider, generator_config);

    // Generate the pipeline
    match generator.generate(&description).await {
        Ok(yaml) => {
            println!("Generated Pipeline:\n");
            println!("{}", yaml);
        }
        Err(e) => {
            eprintln!("Error generating pipeline: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
