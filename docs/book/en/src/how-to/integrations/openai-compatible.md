# OpenAI-Compatible Providers

Many LLM providers expose an OpenAI-compatible API. Synaptic ships convenience constructors for nine popular providers so you can connect without building configuration by hand.

## Setup

Add the `openai` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai"] }
```

All OpenAI-compatible providers use the `synaptic-openai` crate under the hood, so only the `openai` feature is required.

## Supported Providers

The `synaptic::openai::compat` module provides two functions per provider:

- `{provider}_config(api_key, model)` -- returns an `OpenAiConfig` pre-configured with the correct base URL.
- `{provider}_chat_model(api_key, model, backend)` -- returns a ready-to-use `OpenAiChatModel`.

Some providers also offer embeddings variants.

| Provider | Config function | Chat model function | Embeddings? |
|----------|----------------|---------------------|-------------|
| Groq | `groq_config` | `groq_chat_model` | No |
| DeepSeek | `deepseek_config` | `deepseek_chat_model` | No |
| Fireworks | `fireworks_config` | `fireworks_chat_model` | No |
| Together | `together_config` | `together_chat_model` | No |
| xAI | `xai_config` | `xai_chat_model` | No |
| MistralAI | `mistral_config` | `mistral_chat_model` | Yes |
| HuggingFace | `huggingface_config` | `huggingface_chat_model` | Yes |
| Cohere | `cohere_config` | `cohere_chat_model` | Yes |
| OpenRouter | `openrouter_config` | `openrouter_chat_model` | No |

## Usage

### Chat model

```rust,ignore
use std::sync::Arc;
use synaptic::openai::compat::{groq_chat_model, deepseek_chat_model};
use synaptic::models::HttpBackend;
use synaptic::core::{ChatModel, ChatRequest, Message};

let backend = Arc::new(HttpBackend::new());

// Groq
let model = groq_chat_model("gsk-...", "llama-3.3-70b-versatile", backend.clone());
let request = ChatRequest::new(vec![Message::human("Hello from Groq!")]);
let response = model.chat(&request).await?;

// DeepSeek
let model = deepseek_chat_model("sk-...", "deepseek-chat", backend.clone());
let response = model.chat(&request).await?;
```

### Config-first approach

If you need to customize the config further before creating the model:

```rust,ignore
use std::sync::Arc;
use synaptic::openai::compat::fireworks_config;
use synaptic::openai::OpenAiChatModel;
use synaptic::models::HttpBackend;

let config = fireworks_config("fw-...", "accounts/fireworks/models/llama-v3p1-70b-instruct")
    .with_temperature(0.7)
    .with_max_tokens(2048);

let model = OpenAiChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Embeddings

Providers that support embeddings have `{provider}_embeddings_config` and `{provider}_embeddings` functions:

```rust,ignore
use std::sync::Arc;
use synaptic::openai::compat::{mistral_embeddings, cohere_embeddings, huggingface_embeddings};
use synaptic::models::HttpBackend;
use synaptic::core::Embeddings;

let backend = Arc::new(HttpBackend::new());

// MistralAI embeddings
let embeddings = mistral_embeddings("sk-...", "mistral-embed", backend.clone());
let vectors = embeddings.embed_documents(&["Hello world"]).await?;

// Cohere embeddings
let embeddings = cohere_embeddings("co-...", "embed-english-v3.0", backend.clone());

// HuggingFace embeddings
let embeddings = huggingface_embeddings("hf_...", "BAAI/bge-small-en-v1.5", backend.clone());
```

## Unlisted providers

Any provider that exposes an OpenAI-compatible API can be used by setting a custom base URL on `OpenAiConfig`:

```rust,ignore
use std::sync::Arc;
use synaptic::openai::{OpenAiConfig, OpenAiChatModel};
use synaptic::models::HttpBackend;

let config = OpenAiConfig::new("your-api-key", "model-name")
    .with_base_url("https://api.example.com/v1");

let model = OpenAiChatModel::new(config, Arc::new(HttpBackend::new()));
```

This works for any service that accepts the OpenAI chat completions request format at `{base_url}/chat/completions`.

## Streaming

All OpenAI-compatible models support streaming. Use `stream_chat()` just like you would with the standard `OpenAiChatModel`:

```rust,ignore
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![Message::human("Tell me a story")]);
let mut stream = model.stream_chat(&request).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if let Some(text) = &chunk.content {
        print!("{}", text);
    }
}
```

## Provider reference

| Provider | Base URL | Env variable (convention) |
|----------|----------|--------------------------|
| Groq | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` |
| DeepSeek | `https://api.deepseek.com/v1` | `DEEPSEEK_API_KEY` |
| Fireworks | `https://api.fireworks.ai/inference/v1` | `FIREWORKS_API_KEY` |
| Together | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` |
| xAI | `https://api.x.ai/v1` | `XAI_API_KEY` |
| MistralAI | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` |
| HuggingFace | `https://api-inference.huggingface.co/v1` | `HUGGINGFACE_API_KEY` |
| Cohere | `https://api.cohere.com/v1` | `CO_API_KEY` |
| OpenRouter | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
