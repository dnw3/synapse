# Anthropic

This guide shows how to use the Anthropic Messages API as a chat model provider in Synaptic. `AnthropicChatModel` wraps the Anthropic REST API and supports streaming, tool calling, and all standard `ChatModel` operations.

## Setup

Add the `anthropic` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["anthropic"] }
```

### API key

Set your Anthropic API key as an environment variable:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

The key is passed to `AnthropicConfig` at construction time. Requests are authenticated with the `x-api-key` header (not a Bearer token).

## Configuration

Create an `AnthropicConfig` with your API key and model name:

```rust,ignore
use synaptic::anthropic::{AnthropicConfig, AnthropicChatModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = AnthropicConfig::new("sk-ant-...", "claude-sonnet-4-20250514");
let model = AnthropicChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Custom base URL

To use a proxy or alternative endpoint:

```rust,ignore
let config = AnthropicConfig::new(api_key, "claude-sonnet-4-20250514")
    .with_base_url("https://my-proxy.example.com");
```

### Model parameters

```rust,ignore
let config = AnthropicConfig::new(api_key, "claude-sonnet-4-20250514")
    .with_max_tokens(4096)
    .with_top_p(0.9)
    .with_stop(vec!["END".to_string()]);
```

## Usage

`AnthropicChatModel` implements the `ChatModel` trait:

```rust,ignore
use synaptic::anthropic::{AnthropicConfig, AnthropicChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = AnthropicConfig::new(
    std::env::var("ANTHROPIC_API_KEY").unwrap(),
    "claude-sonnet-4-20250514",
);
let model = AnthropicChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("Explain Rust's ownership model in one sentence."),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## Streaming

`AnthropicChatModel` supports native SSE streaming via the `stream_chat` method:

```rust,ignore
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![
    Message::human("Write a short poem about Rust."),
]);

let mut stream = model.stream_chat(request);
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if !chunk.content.is_empty() {
        print!("{}", chunk.content);
    }
}
```

## Tool calling

Anthropic models support tool calling through `tool_use` and `tool_result` content blocks. Synaptic maps `ToolDefinition` and `ToolChoice` to the Anthropic format automatically.

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message, ToolDefinition, ToolChoice};

let tools = vec![ToolDefinition {
    name: "get_weather".into(),
    description: "Get the current weather for a city".into(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "city": { "type": "string", "description": "City name" }
        },
        "required": ["city"]
    }),
}];

let request = ChatRequest::new(vec![
    Message::human("What is the weather in Tokyo?"),
])
.with_tools(tools)
.with_tool_choice(ToolChoice::Auto);

let response = model.chat(request).await?;

// Check if the model requested a tool call
for tc in response.message.tool_calls() {
    println!("Tool: {}, Args: {}", tc.name, tc.arguments);
}
```

`ToolChoice` variants map to Anthropic's `tool_choice` as follows:

| Synaptic | Anthropic |
|----------|-----------|
| `Auto` | `{"type": "auto"}` |
| `Required` | `{"type": "any"}` |
| `None` | `{"type": "none"}` |
| `Specific(name)` | `{"type": "tool", "name": "..."}` |

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Anthropic API key |
| `model` | `String` | required | Model name (e.g. `claude-sonnet-4-20250514`) |
| `base_url` | `String` | `"https://api.anthropic.com"` | API base URL |
| `max_tokens` | `u32` | `1024` | Maximum tokens to generate |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling parameter |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
