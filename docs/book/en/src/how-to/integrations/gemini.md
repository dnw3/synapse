# Google Gemini

This guide shows how to use the Google Generative Language API as a chat model provider in Synaptic. `GeminiChatModel` wraps Google's Generative Language REST API and supports streaming, tool calling, and all standard `ChatModel` operations.

## Setup

Add the `gemini` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["gemini"] }
```

### API key

Set your Google API key as an environment variable:

```bash
export GOOGLE_API_KEY="AIza..."
```

The key is passed to `GeminiConfig` at construction time. Unlike other providers, the API key is sent as a query parameter (`?key=...`) rather than in a request header.

## Configuration

Create a `GeminiConfig` with your API key and model name:

```rust,ignore
use synaptic::gemini::{GeminiConfig, GeminiChatModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = GeminiConfig::new("AIza...", "gemini-2.0-flash");
let model = GeminiChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Custom base URL

To use a proxy or alternative endpoint:

```rust,ignore
let config = GeminiConfig::new(api_key, "gemini-2.0-flash")
    .with_base_url("https://my-proxy.example.com");
```

### Model parameters

```rust,ignore
let config = GeminiConfig::new(api_key, "gemini-2.0-flash")
    .with_top_p(0.9)
    .with_stop(vec!["END".to_string()]);
```

## Usage

`GeminiChatModel` implements the `ChatModel` trait:

```rust,ignore
use synaptic::gemini::{GeminiConfig, GeminiChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = GeminiConfig::new(
    std::env::var("GOOGLE_API_KEY").unwrap(),
    "gemini-2.0-flash",
);
let model = GeminiChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("Explain Rust's ownership model in one sentence."),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## Streaming

`GeminiChatModel` supports native SSE streaming via the `stream_chat` method. The streaming endpoint uses `streamGenerateContent?alt=sse`:

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

Gemini models support tool calling through `functionCall` and `functionResponse` parts (camelCase format). Synaptic maps `ToolDefinition` and `ToolChoice` to the Gemini format automatically.

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

`ToolChoice` variants map to Gemini's `functionCallingConfig` as follows:

| Synaptic | Gemini |
|----------|--------|
| `Auto` | `{"mode": "AUTO"}` |
| `Required` | `{"mode": "ANY"}` |
| `None` | `{"mode": "NONE"}` |
| `Specific(name)` | `{"mode": "ANY", "allowedFunctionNames": ["..."]}` |

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Google API key |
| `model` | `String` | required | Model name (e.g. `gemini-2.0-flash`) |
| `base_url` | `String` | `"https://generativelanguage.googleapis.com"` | API base URL |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling parameter |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
