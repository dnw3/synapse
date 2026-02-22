# Together AI

[Together AI](https://www.together.ai/) provides access to leading open-source models (Llama, DeepSeek, Qwen, Mixtral) via an OpenAI-compatible API. It offers serverless inference at competitive prices, making it ideal for production workloads that require state-of-the-art open models.

The `synaptic-together` crate wraps `synaptic-openai` with Together AI's base URL preset and a type-safe model enum.

## Setup

Add the `together` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["together"] }
```

Sign up at [api.together.xyz](https://api.together.xyz/) to obtain an API key.

## Configuration

```rust,ignore
use synaptic::together::{TogetherChatModel, TogetherConfig, TogetherModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = TogetherConfig::new("your-api-key", TogetherModel::Llama3_3_70bInstructTurbo);
let model = TogetherChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder methods

```rust,ignore
let config = TogetherConfig::new("your-api-key", TogetherModel::Llama3_3_70bInstructTurbo)
    .with_temperature(0.7)
    .with_max_tokens(2048)
    .with_top_p(0.9)
    .with_stop(vec!["</s>".to_string()]);
```

For unlisted models:

```rust,ignore
let config = TogetherConfig::new_custom("your-api-key", "custom-org/custom-model-v1");
```

## Available Models

| Enum Variant | API Model ID | Best For |
|---|---|---|
| `Llama3_3_70bInstructTurbo` | `meta-llama/Llama-3.3-70B-Instruct-Turbo` | General purpose (recommended) |
| `Llama3_1_8bInstructTurbo` | `meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo` | Fast, cost-effective |
| `Llama3_1_405bInstructTurbo` | `meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo` | Maximum quality |
| `DeepSeekR1` | `deepseek-ai/DeepSeek-R1` | Reasoning tasks |
| `Qwen2_5_72bInstructTurbo` | `Qwen/Qwen2.5-72B-Instruct-Turbo` | Multilingual |
| `Mixtral8x7bInstruct` | `mistralai/Mixtral-8x7B-Instruct-v0.1` | Long-context MoE |
| `Custom(String)` | _(any)_ | Unlisted / preview models |

## Usage

```rust,ignore
use synaptic::together::{TogetherChatModel, TogetherConfig, TogetherModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = TogetherConfig::new("your-api-key", TogetherModel::Llama3_3_70bInstructTurbo);
let model = TogetherChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are a concise assistant."),
    Message::human("What is Rust famous for?"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## Streaming

```rust,ignore
use futures::StreamExt;

let request = ChatRequest::new(vec![
    Message::human("Explain Rust's ownership model in 3 sentences."),
]);

let mut stream = model.stream_chat(request);
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    print!("{}", chunk.content);
}
println!();
```

## Error Handling

```rust,ignore
use synaptic::core::SynapticError;

match model.chat(request).await {
    Ok(response) => println!("{}", response.message.content()),
    Err(SynapticError::RateLimit(msg)) => eprintln!("Rate limited: {}", msg),
    Err(e) => return Err(e.into()),
}
```

## Configuration Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Together AI API key |
| `model` | `String` | from enum | API model identifier |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |
| `temperature` | `Option<f64>` | `None` | Sampling temperature (0.0–2.0) |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling threshold |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
