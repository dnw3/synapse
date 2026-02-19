# Model Profiles

`ModelProfile` exposes a model's capabilities and limits so that calling code can inspect provider support flags at runtime without hard-coding provider-specific knowledge.

## The `ModelProfile` Struct

```rust,ignore
pub struct ModelProfile {
    pub name: String,
    pub provider: String,
    pub supports_tool_calling: bool,
    pub supports_structured_output: bool,
    pub supports_streaming: bool,
    pub max_input_tokens: Option<usize>,
    pub max_output_tokens: Option<usize>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Model identifier (e.g. `"gpt-4o"`, `"claude-3-opus"`) |
| `provider` | `String` | Provider name (e.g. `"openai"`, `"anthropic"`) |
| `supports_tool_calling` | `bool` | Whether the model can handle `ToolDefinition` in requests |
| `supports_structured_output` | `bool` | Whether the model supports JSON schema enforcement |
| `supports_streaming` | `bool` | Whether `stream_chat()` produces real token-level chunks |
| `max_input_tokens` | `Option<usize>` | Maximum context window size, if known |
| `max_output_tokens` | `Option<usize>` | Maximum generation length, if known |

## Querying a Model's Profile

Every `ChatModel` implementation exposes a `profile()` method that returns `Option<ModelProfile>`. The default implementation returns `None`, so providers opt in by overriding it:

```rust,ignore
use synaptic_core::ChatModel;

let model = my_chat_model();

if let Some(profile) = model.profile() {
    println!("Provider: {}", profile.provider);
    println!("Supports tools: {}", profile.supports_tool_calling);

    if let Some(max) = profile.max_input_tokens {
        println!("Context window: {} tokens", max);
    }
} else {
    println!("No profile available for this model");
}
```

## Using Profiles for Capability Checks

Profiles are useful when writing generic code that works across multiple providers. For example, you can guard tool-calling or structured-output logic behind a capability check:

```rust,ignore
use synaptic_core::{ChatModel, ChatRequest, ToolChoice};

async fn maybe_call_with_tools(
    model: &dyn ChatModel,
    request: ChatRequest,
) -> Result<ChatResponse, SynapticError> {
    let supports_tools = model
        .profile()
        .map(|p| p.supports_tool_calling)
        .unwrap_or(false);

    if supports_tools {
        let request = request.with_tool_choice(ToolChoice::Auto);
        model.chat(request).await
    } else {
        // Fall back to plain chat without tools
        model.chat(ChatRequest::new(request.messages)).await
    }
}
```

## Implementing `profile()` for a Custom Model

If you implement your own `ChatModel`, override `profile()` to advertise capabilities:

```rust,ignore
use synaptic_core::{ChatModel, ModelProfile};

impl ChatModel for MyCustomModel {
    // ... chat() and stream_chat() ...

    fn profile(&self) -> Option<ModelProfile> {
        Some(ModelProfile {
            name: "my-model-v1".to_string(),
            provider: "custom".to_string(),
            supports_tool_calling: true,
            supports_structured_output: false,
            supports_streaming: true,
            max_input_tokens: Some(128_000),
            max_output_tokens: Some(4_096),
        })
    }
}
```
