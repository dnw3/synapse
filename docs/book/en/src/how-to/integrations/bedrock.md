# AWS Bedrock

This guide shows how to use AWS Bedrock as a chat model provider in Synaptic. Bedrock provides access to foundation models from Amazon, Anthropic, Meta, Mistral, and others through the AWS SDK.

## Setup

Add the `bedrock` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["bedrock"] }
```

### AWS credentials

`BedrockChatModel` uses the AWS SDK for Rust, which reads credentials from the standard AWS credential chain:

1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`)
2. Shared credentials file (`~/.aws/credentials`)
3. IAM role (when running on EC2, ECS, Lambda, etc.)

Ensure your IAM principal has `bedrock:InvokeModel` and `bedrock:InvokeModelWithResponseStream` permissions.

## Configuration

Create a `BedrockConfig` with the model ID:

```rust,ignore
use synaptic::bedrock::{BedrockConfig, BedrockChatModel};

let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
let model = BedrockChatModel::new(config).await;
```

> **Note:** The constructor is `async` because it initializes the AWS SDK client, which loads credentials and resolves the region from the environment.

### Region

By default, the region is resolved from the AWS SDK default chain (environment variable `AWS_REGION`, config file, etc.). You can override it:

```rust,ignore
let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .with_region("us-west-2");
```

### Model parameters

```rust,ignore
let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .with_temperature(0.7)
    .with_max_tokens(4096);
```

## Usage

`BedrockChatModel` implements the `ChatModel` trait:

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![
    Message::system("You are a helpful assistant."),
    Message::human("Explain AWS Bedrock in one sentence."),
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

Bedrock supports tool calling for models that expose it (e.g. Anthropic Claude models):

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

let request = ChatRequest::new(vec![Message::human("Weather in Tokyo?")])
    .with_tools(tools);

let response = model.chat(&request).await?;
```

## Using an existing AWS client

If you already have a configured `aws_sdk_bedrockruntime::Client`, pass it directly with `from_client`:

```rust,ignore
use synaptic::bedrock::{BedrockConfig, BedrockChatModel};

let aws_config = aws_config::from_env().region("eu-west-1").load().await;
let client = aws_sdk_bedrockruntime::Client::new(&aws_config);

let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
let model = BedrockChatModel::from_client(config, client);
```

> **Note:** Unlike the standard constructor, `from_client` is **not** async because it skips AWS SDK initialization.

## Architecture note

`BedrockChatModel` does **not** use the `ProviderBackend` abstraction (`HttpBackend`/`FakeBackend`). It calls the AWS SDK directly via the Bedrock Runtime `converse` and `converse_stream` APIs. This means you cannot inject a `FakeBackend` for testing. Instead, use `ScriptedChatModel` as a test double:

```rust,ignore
use synaptic::models::ScriptedChatModel;
use synaptic::core::Message;

let model = ScriptedChatModel::new(vec![
    Message::ai("Mocked Bedrock response"),
]);
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model_id` | `String` | required | Bedrock model ID (e.g. `anthropic.claude-3-5-sonnet-20241022-v2:0`) |
| `region` | `Option<String>` | `None` (auto-detect) | AWS region override |
| `temperature` | `Option<f32>` | `None` | Sampling temperature |
| `max_tokens` | `Option<u32>` | `None` | Maximum tokens to generate |
