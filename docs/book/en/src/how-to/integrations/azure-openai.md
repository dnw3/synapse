# Azure OpenAI

This guide shows how to use Azure OpenAI Service as a chat model and embeddings provider in Synaptic. Azure OpenAI uses deployment-based URLs and `api-key` header authentication instead of Bearer tokens.

## Setup

Add the `openai` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai"] }
```

Azure OpenAI support is included in the `synaptic-openai` crate, so no additional feature flag is needed.

## Configuration

Create an `AzureOpenAiConfig` with your API key, resource name, and deployment name:

```rust,ignore
use std::sync::Arc;
use synaptic::openai::{AzureOpenAiConfig, AzureOpenAiChatModel};
use synaptic::models::HttpBackend;

let config = AzureOpenAiConfig::new(
    "your-azure-api-key",
    "my-resource",         // Azure resource name
    "gpt-4o-deployment",   // Deployment name
);

let model = AzureOpenAiChatModel::new(config, Arc::new(HttpBackend::new()));
```

The resulting endpoint URL is:
```text
https://{resource_name}.openai.azure.com/openai/deployments/{deployment_name}/chat/completions?api-version={api_version}
```

### API version

The default API version is `"2024-10-21"`. You can override it:

```rust,ignore
let config = AzureOpenAiConfig::new("key", "resource", "deployment")
    .with_api_version("2024-12-01-preview");
```

### Model parameters

Configure temperature, max tokens, and other generation parameters:

```rust,ignore
let config = AzureOpenAiConfig::new("key", "resource", "deployment")
    .with_temperature(0.7)
    .with_max_tokens(4096);
```

## Usage

`AzureOpenAiChatModel` implements the `ChatModel` trait, so it works everywhere a standard model does:

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("What is Azure OpenAI?"),
]);

let response = model.chat(&request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

### Streaming

```rust,ignore
use futures::StreamExt;

let mut stream = model.stream_chat(&request).await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if let Some(text) = &chunk.content {
        print!("{}", text);
    }
}
```

### Tool calling

```rust,ignore
use synaptic::core::{ChatRequest, Message, ToolDefinition};

let tools = vec![ToolDefinition {
    name: "get_weather".into(),
    description: "Get the current weather".into(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "city": { "type": "string" }
        },
        "required": ["city"]
    }),
}];

let request = ChatRequest::new(vec![Message::human("What's the weather in Seattle?")])
    .with_tools(tools);

let response = model.chat(&request).await?;
```

## Embeddings

Use `AzureOpenAiEmbeddings` for text embedding with Azure-hosted models:

```rust,ignore
use std::sync::Arc;
use synaptic::openai::{AzureOpenAiEmbeddingsConfig, AzureOpenAiEmbeddings};
use synaptic::models::HttpBackend;
use synaptic::core::Embeddings;

let config = AzureOpenAiEmbeddingsConfig::new(
    "your-azure-api-key",
    "my-resource",
    "text-embedding-ada-002-deployment",
);

let embeddings = AzureOpenAiEmbeddings::new(config, Arc::new(HttpBackend::new()));
let vectors = embeddings.embed_documents(&["Hello world", "Rust is fast"]).await?;
```

## Environment variables

A common pattern is to read credentials from the environment:

```rust,ignore
let config = AzureOpenAiConfig::new(
    std::env::var("AZURE_OPENAI_API_KEY").unwrap(),
    std::env::var("AZURE_OPENAI_RESOURCE").unwrap(),
    std::env::var("AZURE_OPENAI_DEPLOYMENT").unwrap(),
);
```

## Configuration reference

### AzureOpenAiConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Azure OpenAI API key |
| `resource_name` | `String` | required | Azure resource name |
| `deployment_name` | `String` | required | Model deployment name |
| `api_version` | `String` | `"2024-10-21"` | Azure API version |
| `temperature` | `Option<f32>` | `None` | Sampling temperature |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |

### AzureOpenAiEmbeddingsConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Azure OpenAI API key |
| `resource_name` | `String` | required | Azure resource name |
| `deployment_name` | `String` | required | Embeddings deployment name |
| `api_version` | `String` | `"2024-10-21"` | Azure API version |
