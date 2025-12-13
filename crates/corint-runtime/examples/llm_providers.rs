//! LLM Provider Examples
//!
//! This example demonstrates how to use different LLM providers
//! for text generation, including both standard and thinking models.
//!
//! To run this example:
//! ```bash
//! # Standard Models
//! OPENAI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers openai
//! ANTHROPIC_API_KEY=your_key cargo run --package corint-runtime --example llm_providers anthropic
//! GEMINI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers gemini
//! DEEPSEEK_API_KEY=your_key cargo run --package corint-runtime --example llm_providers deepseek
//!
//! # Thinking/Reasoning Models
//! OPENAI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers openai-thinking
//! ANTHROPIC_API_KEY=your_key cargo run --package corint-runtime --example llm_providers anthropic-thinking
//!
//! # Mock (testing, no API key needed)
//! cargo run --package corint-runtime --example llm_providers mock
//! cargo run --package corint-runtime --example llm_providers mock-thinking
//! ```

use corint_runtime::{
    AnthropicProvider, DeepSeekProvider, GeminiProvider, LLMClient, LLMRequest, MockProvider,
    OpenAIProvider,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get provider name from command line
    let args: Vec<String> = env::args().collect();
    let provider_name = args.get(1).map(|s| s.as_str()).unwrap_or("mock");

    println!("🤖 LLM Provider Example");
    println!("Provider: {}\n", provider_name);

    match provider_name {
        "openai" => {
            let api_key =
                env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

            let provider = OpenAIProvider::new(api_key);

            println!("📝 Testing standard text generation (GPT-4o-mini)...");
            let request = LLMRequest::new(
                "What is Rust programming language? Give a brief answer.".to_string(),
                "gpt-4o-mini".to_string(),
            )
            .with_max_tokens(150)
            .with_temperature(0.7)
            .with_system("You are a helpful programming assistant.".to_string());

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        "openai-thinking" => {
            let api_key =
                env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

            let provider = OpenAIProvider::new(api_key);

            println!("🧠 Testing OpenAI O1 thinking model...");
            println!("Note: O1 models use internal chain-of-thought reasoning\n");

            let request = LLMRequest::new(
                "Explain the concept of ownership in Rust and why it's important for memory safety.".to_string(),
                "o1-mini".to_string(),
            )
            .with_max_tokens(2000);
            // Note: O1 models don't use temperature parameter

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);

                    // O1 models may include reasoning trace
                    if let Some(thinking) = response.thinking {
                        println!("\n🤔 Internal Reasoning:");
                        println!("{}", thinking);
                    }
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        "anthropic" => {
            let api_key = env::var("ANTHROPIC_API_KEY")
                .expect("ANTHROPIC_API_KEY environment variable not set");

            let provider = AnthropicProvider::new(api_key);

            println!("📝 Testing standard text generation (Claude)...");
            let request = LLMRequest::new(
                "What is Rust programming language? Give a brief answer.".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            )
            .with_max_tokens(150)
            .with_temperature(0.7)
            .with_system("You are a helpful programming assistant.".to_string());

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        "anthropic-thinking" => {
            let api_key = env::var("ANTHROPIC_API_KEY")
                .expect("ANTHROPIC_API_KEY environment variable not set");

            let provider = AnthropicProvider::new(api_key);

            println!("🧠 Testing Claude with extended thinking mode...");
            println!("Note: Extended thinking allows Claude to reason step-by-step\n");

            let request = LLMRequest::new(
                "Explain the concept of ownership in Rust and why it's important for memory safety. Think through the implications carefully.".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            )
            .with_max_tokens(2000)
            .with_temperature(0.7)
            .with_thinking(true); // Enable extended thinking

            match provider.call(request).await {
                Ok(response) => {
                    // Display thinking process first (if available)
                    if let Some(thinking) = &response.thinking {
                        println!("🤔 Thinking Process:");
                        println!("{}\n", thinking);
                        println!("---\n");
                    }

                    println!("✅ Final Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        "gemini" => {
            let api_key =
                env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY environment variable not set");

            let provider = GeminiProvider::new(api_key);

            println!("📝 Testing text generation (Gemini)...");
            let request = LLMRequest::new(
                "What is Rust programming language? Give a brief answer.".to_string(),
                "gemini-1.5-flash".to_string(),
            )
            .with_max_tokens(150)
            .with_temperature(0.7)
            .with_system("You are a helpful programming assistant.".to_string());

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                }
                Err(e) => println!("❌ Error: {}", e),
            }

            println!("\nℹ️  Note: Gemini does not support thinking/reasoning mode");
        }

        "deepseek" => {
            let api_key = env::var("DEEPSEEK_API_KEY")
                .expect("DEEPSEEK_API_KEY environment variable not set");

            let provider = DeepSeekProvider::new(api_key);

            println!("📝 Testing text generation (DeepSeek)...");
            let request = LLMRequest::new(
                "What is Rust programming language? Give a brief answer.".to_string(),
                "deepseek-chat".to_string(),
            )
            .with_max_tokens(150)
            .with_temperature(0.7)
            .with_system("You are a helpful programming assistant.".to_string());

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                }
                Err(e) => println!("❌ Error: {}", e),
            }

            println!("\nℹ️  Note: DeepSeek does not support thinking/reasoning mode");
        }

        "mock-thinking" => {
            let provider = MockProvider::with_thinking(
                "Rust is a systems programming language focused on safety and performance.".to_string(),
                "First, I need to consider what makes Rust unique... The ownership system ensures memory safety... This prevents common bugs like use-after-free...".to_string(),
            );

            println!("🧠 Testing mock provider with thinking mode...\n");

            let request = LLMRequest::new("What is Rust?".to_string(), "mock-thinking".to_string())
                .with_thinking(true);

            match provider.call(request).await {
                Ok(response) => {
                    if let Some(thinking) = &response.thinking {
                        println!("🤔 Thinking Process:");
                        println!("{}\n", thinking);
                        println!("---\n");
                    }

                    println!("✅ Final Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Supports thinking: {}", provider.supports_thinking());
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        "mock" | _ => {
            let provider = MockProvider::with_response(
                "Rust is a systems programming language that focuses on safety, speed, and concurrency.".to_string()
            );

            println!("📝 Testing mock provider (standard mode)...\n");

            let request = LLMRequest::new("What is Rust?".to_string(), "mock-model".to_string());

            match provider.call(request).await {
                Ok(response) => {
                    println!("✅ Response: {}", response.content);
                    println!("   Model: {}", response.model);
                    println!("   Tokens used: {}", response.tokens_used);
                    println!("   Finish reason: {}", response.finish_reason);
                    println!("   Supports thinking: {}", provider.supports_thinking());
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }
    }

    println!("\n✨ Example completed!");
    Ok(())
}
