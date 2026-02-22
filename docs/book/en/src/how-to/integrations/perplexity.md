# Perplexity AI

[Perplexity AI](https://www.perplexity.ai/) provides online search-augmented language models through its Sonar model family. Unlike traditional LLMs, Sonar models access real-time web information and return cited sources, making them ideal for factual queries and research tasks.

The `synaptic-perplexity` crate wraps `synaptic-openai` with Perplexity's base URL and a type-safe model enum.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["perplexity"] }
```

Sign up at [perplexity.ai](https://www.perplexity.ai/) to obtain an API key (prefixed with `pplx-`).

## Configuration

```rust,ignore
use synaptic::perplexity::{PerplexityChatModel, PerplexityConfig, PerplexityModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge);
let model = PerplexityChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder methods

```rust,ignore
let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge)
    .with_temperature(0.2)
    .with_max_tokens(1024);
```

## Available Models

| Enum Variant | API Model ID | Best For |
|---|---|---|
| `SonarLarge` | `sonar-large-online` | General web search (recommended) |
| `SonarSmall` | `sonar-small-online` | Fast, cost-effective web search |
| `SonarHuge` | `sonar-huge-online` | Maximum quality web search |
| `SonarReasoningPro` | `sonar-reasoning-pro` | Complex reasoning with citations |
| `Custom(String)` | _(any)_ | Preview models |

## Usage

```rust,ignore
use synaptic::perplexity::{PerplexityChatModel, PerplexityConfig, PerplexityModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge);
let model = PerplexityChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("Be precise and concise. Cite your sources."),
    Message::human("What is the current state of Rust adoption in systems programming?"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## Streaming

```rust,ignore
use futures::StreamExt;

let request = ChatRequest::new(vec![
    Message::human("What are the latest developments in LLM research?"),
]);

let mut stream = model.stream_chat(request);
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
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
| `api_key` | `String` | required | Perplexity API key (`pplx-...`) |
| `model` | `String` | from enum | API model identifier |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |
| `temperature` | `Option<f64>` | `None` | Sampling temperature (0.0–2.0) |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling threshold |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
