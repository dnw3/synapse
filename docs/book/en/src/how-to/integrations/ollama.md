# Ollama

This guide shows how to use [Ollama](https://ollama.com/) as a local chat model and embeddings provider in Synaptic. `OllamaChatModel` wraps the Ollama REST API and supports streaming, tool calling, and all standard `ChatModel` operations. Because Ollama runs locally, no API key is needed.

## Setup

Add the `ollama` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["ollama"] }
```

### Installing Ollama

Install Ollama from [ollama.com](https://ollama.com/) and pull a model before using the provider:

```bash
# Install Ollama (macOS)
brew install ollama

# Start the Ollama server
ollama serve

# Pull a model
ollama pull llama3.1
```

The default endpoint is `http://localhost:11434`. Make sure the Ollama server is running before sending requests.

## Configuration

Create an `OllamaConfig` with a model name. No API key is required:

```rust,ignore
use synaptic::ollama::{OllamaConfig, OllamaChatModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaConfig::new("llama3.1");
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Custom base URL

To connect to a remote Ollama instance or a non-default port:

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_base_url("http://192.168.1.100:11434");
```

### Model parameters

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_top_p(0.9)
    .with_stop(vec!["END".to_string()])
    .with_seed(42);
```

## Usage

`OllamaChatModel` implements the `ChatModel` trait:

```rust,ignore
use synaptic::ollama::{OllamaConfig, OllamaChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaConfig::new("llama3.1");
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("Explain Rust's ownership model in one sentence."),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## Streaming

`OllamaChatModel` supports native streaming via the `stream_chat` method. Unlike cloud providers that use SSE, Ollama uses NDJSON (newline-delimited JSON) where each line is a complete JSON object:

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

Ollama models that support function calling (such as `llama3.1`) can use tool calling through the `tool_calls` array format. Synaptic maps `ToolDefinition` and `ToolChoice` to the Ollama format automatically.

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

`ToolChoice` variants map to Ollama's `tool_choice` as follows:

| Synaptic | Ollama |
|----------|--------|
| `Auto` | `"auto"` |
| `Required` | `"required"` |
| `None` | `"none"` |
| `Specific(name)` | `{"type": "function", "function": {"name": "..."}}` |

## Reproducibility with seed

Ollama supports a `seed` parameter for reproducible generation. When set, the model will produce deterministic output for the same input:

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_seed(42);
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::human("Pick a random number between 1 and 100."),
]);

// Same seed + same input = same output
let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## Embeddings

`OllamaEmbeddings` provides local embedding generation through Ollama's `/api/embed` endpoint. Pull an embedding model first:

```bash
ollama pull nomic-embed-text
```

### Configuration

```rust,ignore
use synaptic::ollama::{OllamaEmbeddingsConfig, OllamaEmbeddings};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
let embeddings = OllamaEmbeddings::new(config, Arc::new(HttpBackend::new()));
```

To connect to a remote instance:

```rust,ignore
let config = OllamaEmbeddingsConfig::new("nomic-embed-text")
    .with_base_url("http://192.168.1.100:11434");
```

### Usage

`OllamaEmbeddings` implements the `Embeddings` trait:

```rust,ignore
use synaptic::core::Embeddings;

// Embed a single query
let vector = embeddings.embed_query("What is Rust?").await?;
println!("Dimension: {}", vector.len());

// Embed multiple documents
let vectors = embeddings.embed_documents(&["First doc", "Second doc"]).await?;
println!("Embedded {} documents", vectors.len());
```

## Configuration reference

### OllamaConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model` | `String` | required | Model name (e.g. `llama3.1`) |
| `base_url` | `String` | `"http://localhost:11434"` | Ollama server URL |
| `top_p` | `Option<f64>` | `None` | Nucleus sampling parameter |
| `stop` | `Option<Vec<String>>` | `None` | Stop sequences |
| `seed` | `Option<u64>` | `None` | Seed for reproducible generation |

### OllamaEmbeddingsConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model` | `String` | required | Embedding model name (e.g. `nomic-embed-text`) |
| `base_url` | `String` | `"http://localhost:11434"` | Ollama server URL |
