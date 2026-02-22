# Fireworks AI

[Fireworks AI](https://fireworks.ai/) delivers the fastest open-source model inference available, with sub-100ms time-to-first-token for popular models. It uses an OpenAI-compatible API and supports Llama, DeepSeek, Qwen, and other leading open models.

The `synaptic-fireworks` crate wraps `synaptic-openai` with Fireworks AI's base URL preset and a type-safe model enum.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["fireworks"] }
```

Sign up at [fireworks.ai](https://fireworks.ai/) to obtain an API key (prefixed with `fw-`).

## Configuration

```rust,ignore
use synaptic::fireworks::{FireworksChatModel, FireworksConfig, FireworksModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct);
let model = FireworksChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder methods

```rust,ignore
let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct)
    .with_temperature(0.7)
    .with_max_tokens(4096)
    .with_top_p(0.95);
```

## Available Models

| Enum Variant | API Model ID | Best For |
|---|---|---|
| `Llama3_1_70bInstruct` | `accounts/fireworks/models/llama-v3p1-70b-instruct` | General purpose (recommended) |
| `Llama3_1_8bInstruct` | `accounts/fireworks/models/llama-v3p1-8b-instruct` | Fastest, most cost-effective |
| `DeepSeekR1` | `accounts/fireworks/models/deepseek-r1` | Reasoning tasks |
| `Qwen2_5_72bInstruct` | `accounts/fireworks/models/qwen2p5-72b-instruct` | Multilingual |
| `Custom(String)` | _(any)_ | Unlisted / preview models |

## Usage

```rust,ignore
use synaptic::fireworks::{FireworksChatModel, FireworksConfig, FireworksModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct);
let model = FireworksChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("Explain the difference between async and threading in Rust."),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## Streaming

```rust,ignore
use futures::StreamExt;

let request = ChatRequest::new(vec![
    Message::human("Write a haiku about Rust programming."),
]);

let mut stream = model.stream_chat(request);
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
println!();
```

## Configuration Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Fireworks AI API key (`fw-...`) |
| `model` | `String` | from enum | API model identifier |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |
| `temperature` | `Option<f64>` | `None` | Sampling temperature (0.0–2.0) |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling threshold |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
