# xAI Grok

[xAI](https://x.ai/) develops the Grok family of LLMs known for their real-time reasoning capabilities and integration with X (Twitter) data. The Grok API is OpenAI-compatible.

The `synaptic-xai` crate wraps `synaptic-openai` with xAI's base URL preset and a type-safe model enum.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["xai"] }
```

Sign up at [x.ai](https://x.ai/) to obtain an API key.

## Configuration

```rust,ignore
use synaptic::xai::{XaiChatModel, XaiConfig, XaiModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest);
let model = XaiChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder methods

```rust,ignore
let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest)
    .with_temperature(0.7)
    .with_max_tokens(8192);
```

## Available Models

| Enum Variant | API Model ID | Best For |
|---|---|---|
| `Grok2Latest` | `grok-2-latest` | General purpose (recommended) |
| `Grok2Mini` | `grok-2-mini` | Fast, cost-effective |
| `GrokBeta` | `grok-beta` | Legacy compatibility |
| `Custom(String)` | _(any)_ | Preview models |

## Usage

```rust,ignore
use synaptic::xai::{XaiChatModel, XaiConfig, XaiModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest);
let model = XaiChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are Grok, a helpful AI with wit and humor."),
    Message::human("What's happening in AI today?"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## Streaming

```rust,ignore
use futures::StreamExt;

let request = ChatRequest::new(vec![
    Message::human("Give me a quick summary of today's AI trends."),
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
| `api_key` | `String` | required | xAI API key |
| `model` | `String` | from enum | API model identifier |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |
| `temperature` | `Option<f64>` | `None` | Sampling temperature (0.0–2.0) |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling threshold |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
