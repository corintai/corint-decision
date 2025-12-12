# LLM Provider Integration

CORINT Runtime supports multiple LLM providers for text generation, including both standard models and advanced thinking/reasoning models.

## Supported Providers

| Provider | Standard Models | Thinking Models | Notes |
|----------|----------------|-----------------|-------|
| **OpenAI** | ✅ | ✅ | GPT-4, GPT-3.5, O1/O3 reasoning models |
| **Anthropic** | ✅ | ✅ | Claude models with extended thinking mode |
| **Google Gemini** | ✅ | ❌ | Gemini 1.5 models |
| **DeepSeek** | ✅ | ❌ | OpenAI-compatible API |
| **Mock** | ✅ | ✅ | For testing purposes |

## Model Types

### Standard Models
Standard LLM models for general text generation tasks:
- **OpenAI**: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- **Anthropic**: `claude-3-5-sonnet-20241022`, `claude-3-5-haiku-20241022`, `claude-3-opus-20240229`
- **Gemini**: `gemini-1.5-pro`, `gemini-1.5-flash`, `gemini-1.0-pro`
- **DeepSeek**: `deepseek-chat`, `deepseek-coder`

### Thinking Models (Reasoning Models)
Advanced models with extended reasoning capabilities:
- **OpenAI O1/O3**: `o1-preview`, `o1-mini`, `o3-mini` - Internal chain-of-thought reasoning
- **Anthropic Claude**: Extended thinking mode with explicit reasoning budget

## Quick Start

### Basic Text Generation

```rust
use corint_runtime::{OpenAIProvider, LLMClient, LLMRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create provider
    let provider = OpenAIProvider::new("your-api-key".to_string());

    // Create request
    let request = LLMRequest::new(
        "What is Rust?".to_string(),
        "gpt-4o-mini".to_string(),
    )
    .with_max_tokens(100)
    .with_temperature(0.7)
    .with_system("You are a helpful assistant.".to_string());

    // Call LLM
    let response = provider.call(request).await?;

    println!("Response: {}", response.content);
    println!("Tokens used: {}", response.tokens_used);

    Ok(())
}
```

### Thinking Models

#### OpenAI O1/O3 Models

```rust
use corint_runtime::{OpenAIProvider, LLMClient, LLMRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenAIProvider::new("your-api-key".to_string());

    // O1 models automatically use extended reasoning
    let request = LLMRequest::new(
        "Solve this complex math problem: ...".to_string(),
        "o1-preview".to_string(),
    )
    .with_max_tokens(5000);  // O1 uses max_completion_tokens

    let response = provider.call(request).await?;

    println!("Answer: {}", response.content);

    // O1 models include internal reasoning (if available)
    if let Some(thinking) = response.thinking {
        println!("Reasoning: {}", thinking);
    }

    Ok(())
}
```

**Notes for O1/O3 Models:**
- Do not support `temperature` parameter (automatically optimized)
- Use `max_completion_tokens` instead of `max_tokens` internally
- May include reasoning trace in `thinking` field
- Best for complex reasoning tasks

#### Claude Extended Thinking

```rust
use corint_runtime::{AnthropicProvider, LLMClient, LLMRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = AnthropicProvider::new("your-api-key".to_string());

    // Enable extended thinking for Claude
    let request = LLMRequest::new(
        "Analyze this complex scenario: ...".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
    )
    .with_max_tokens(2000)
    .with_thinking(true);  // Enable extended thinking mode

    let response = provider.call(request).await?;

    println!("Answer: {}", response.content);

    // Extended thinking content
    if let Some(thinking) = response.thinking {
        println!("Thinking process: {}", thinking);
    }

    Ok(())
}
```

**Notes for Claude Thinking:**
- Explicitly enable with `.with_thinking(true)`
- Thinking budget defaults to 10,000 tokens
- Returns both thinking content and final answer
- Useful for complex reasoning tasks

## Provider-Specific Examples

### OpenAI

```rust
use corint_runtime::{OpenAIProvider, LLMClient, LLMRequest};

let provider = OpenAIProvider::new(api_key);

// Standard model (GPT-4)
let request = LLMRequest::new(prompt, "gpt-4o-mini".to_string())
    .with_system("You are a helpful assistant.".to_string())
    .with_temperature(0.7)
    .with_max_tokens(1000);

let response = provider.call(request).await?;

// Thinking model (O1)
let thinking_request = LLMRequest::new(complex_prompt, "o1-preview".to_string())
    .with_max_tokens(5000);

let thinking_response = provider.call(thinking_request).await?;
```

**Supported Models:**
- Standard: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Thinking: `o1-preview`, `o1-mini`, `o3-mini`

**Features:**
- Custom base URL support (Azure OpenAI compatible)
- Automatic O1/O3 model detection
- Internal reasoning extraction

### Anthropic

```rust
use corint_runtime::{AnthropicProvider, LLMClient, LLMRequest};

let provider = AnthropicProvider::new(api_key);

// Standard usage
let request = LLMRequest::new(prompt, "claude-3-5-sonnet-20241022".to_string())
    .with_system("You are a helpful assistant.".to_string())
    .with_max_tokens(1024);

let response = provider.call(request).await?;

// With extended thinking
let thinking_request = LLMRequest::new(prompt, "claude-3-5-sonnet-20241022".to_string())
    .with_thinking(true)
    .with_max_tokens(2000);

let thinking_response = provider.call(thinking_request).await?;
```

**Supported Models:**
- `claude-3-5-sonnet-20241022`
- `claude-3-5-haiku-20241022`
- `claude-3-opus-20240229`

**Features:**
- Extended thinking mode support
- Configurable thinking budget (default: 10,000 tokens)
- Content block parsing for thinking and text

### Google Gemini

```rust
use corint_runtime::{GeminiProvider, LLMClient, LLMRequest};

let provider = GeminiProvider::new(api_key);

let request = LLMRequest::new(prompt, "gemini-1.5-flash".to_string())
    .with_temperature(0.7)
    .with_max_tokens(1000);

let response = provider.call(request).await?;
```

**Supported Models:**
- `gemini-1.5-pro`, `gemini-1.5-flash`, `gemini-1.0-pro`

**Features:**
- System message support via user/model dialogue
- API key via URL parameter

### DeepSeek

```rust
use corint_runtime::{DeepSeekProvider, LLMClient, LLMRequest};

let provider = DeepSeekProvider::new(api_key);

let request = LLMRequest::new(prompt, "deepseek-chat".to_string())
    .with_temperature(0.7)
    .with_max_tokens(1000);

let response = provider.call(request).await?;
```

**Supported Models:**
- `deepseek-chat`, `deepseek-coder`

**Features:**
- OpenAI-compatible API
- Full compatibility with OpenAI SDK patterns

## Caching

All providers support optional caching to reduce API calls:

```rust
use std::sync::Arc;
use corint_runtime::{OpenAIProvider, InMemoryLLMCache};

let cache = Arc::new(InMemoryLLMCache::new());
let provider = OpenAIProvider::with_cache(api_key, cache);

// First call hits the API
let response1 = provider.call(request.clone()).await?;

// Second call with same request returns cached result
let response2 = provider.call(request.clone()).await?;
```

**Note:** Cache keys include prompt, model, and all parameters, so thinking mode requests are cached separately from standard requests.

## Custom Base URLs

OpenAI provider supports custom base URLs (e.g., for Azure OpenAI):

```rust
use corint_runtime::OpenAIProvider;

let provider = OpenAIProvider::with_base_url(
    api_key,
    "https://your-resource.openai.azure.com/openai/deployments/your-deployment".to_string()
);
```

## Error Handling

All LLM operations return `Result<T, RuntimeError>`:

```rust
match provider.call(request).await {
    Ok(response) => {
        println!("Success: {}", response.content);

        // Check for thinking content
        if let Some(thinking) = response.thinking {
            println!("Reasoning: {}", thinking);
        }
    }
    Err(e) => {
        eprintln!("Error calling LLM: {}", e);
        // Handle error appropriately
    }
}
```

## Testing

Use `MockProvider` for testing without API calls:

```rust
use corint_runtime::{MockProvider, LLMClient, LLMRequest};

#[tokio::test]
async fn test_llm_integration() {
    let provider = MockProvider::with_response("Test response".to_string());

    let request = LLMRequest::new("Test".to_string(), "mock".to_string());
    let response = provider.call(request).await.unwrap();

    assert_eq!(response.content, "Test response");
}

#[tokio::test]
async fn test_thinking_mode() {
    let provider = MockProvider::with_thinking(
        "Answer".to_string(),
        "Let me think...".to_string()
    );

    let request = LLMRequest::new("Question".to_string(), "mock".to_string())
        .with_thinking(true);

    let response = provider.call(request).await.unwrap();

    assert_eq!(response.content, "Answer");
    assert_eq!(response.thinking, Some("Let me think...".to_string()));
}
```

## API Reference

### LLMRequest

- `new(prompt: String, model: String)` - Create new request
- `with_max_tokens(tokens: u32)` - Set max tokens to generate
- `with_temperature(temp: f32)` - Set temperature (0.0 - 1.0)
- `with_system(message: String)` - Set system message/instructions
- `with_thinking(enable: bool)` - Enable extended thinking mode (for compatible models)

### LLMResponse

- `content: String` - Generated text (final answer)
- `model: String` - Model that generated the response
- `tokens_used: u32` - Total tokens consumed
- `finish_reason: String` - Reason for completion ("stop", "length", etc.)
- `thinking: Option<String>` - Extended reasoning/thinking content (if available)

### LLMClient Trait

- `async fn call(&self, request: LLMRequest) -> Result<LLMResponse>` - Generate text
- `fn supports_thinking(&self) -> bool` - Whether provider supports thinking mode
- `fn name(&self) -> &str` - Provider name

## Configuration

See `config/server.yaml` for LLM configuration options:

```yaml
llm:
  default_provider: openai
  enable_cache: true
  enable_thinking: false  # Global default for thinking mode

  openai:
    api_key: "${OPENAI_API_KEY}"
    default_model: "gpt-4o-mini"
    max_tokens: 1000
    temperature: 0.7

  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"
    default_model: "claude-3-5-sonnet-20241022"
    thinking_budget: 10000
    max_tokens: 1000
    temperature: 0.7
```

## Examples

See `crates/corint-runtime/examples/llm_providers.rs` for complete examples:

```bash
# OpenAI (standard model)
OPENAI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers openai

# OpenAI (thinking model - O1)
OPENAI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers openai-thinking

# Anthropic (standard)
ANTHROPIC_API_KEY=your_key cargo run --package corint-runtime --example llm_providers anthropic

# Anthropic (with extended thinking)
ANTHROPIC_API_KEY=your_key cargo run --package corint-runtime --example llm_providers anthropic-thinking

# Gemini
GEMINI_API_KEY=your_key cargo run --package corint-runtime --example llm_providers gemini

# DeepSeek
DEEPSEEK_API_KEY=your_key cargo run --package corint-runtime --example llm_providers deepseek

# Mock (no API key needed)
cargo run --package corint-runtime --example llm_providers mock
```

## Best Practices

### General
1. **Use caching** for repeated queries to reduce costs
2. **Set reasonable token limits** to control costs
3. **Handle errors gracefully** - API calls can fail
4. **Use mock provider** for tests to avoid API calls
5. **Secure API keys** - never commit them to version control

### Thinking Models
1. **Use for complex reasoning** - Simple queries don't need thinking models
2. **Allow higher token limits** - Thinking models need more tokens for reasoning
3. **Parse thinking content** - Use the reasoning trace for debugging/explanation
4. **Consider cost** - Thinking models typically cost more per token
5. **Don't use temperature** - O1/O3 models optimize this automatically

### Model Selection
- **GPT-4o-mini**: Fast, cost-effective for simple tasks
- **GPT-4o**: Best for complex standard tasks
- **O1-preview**: Complex reasoning, math, code
- **Claude Sonnet**: Balanced performance and cost
- **Claude with thinking**: Deep analysis and reasoning
- **Gemini Flash**: Fast responses, low cost
- **DeepSeek**: Cost-effective alternative

## Performance Considerations

1. **Thinking models are slower** - They perform internal reasoning
2. **Token usage is higher** - Reasoning tokens + completion tokens
3. **Caching is critical** - Cache thinking responses aggressively
4. **Parallel requests** - Use async/await for concurrent calls
5. **Batch when possible** - Group related queries

## License

See main project LICENSE file.
