# Model Profiles

`ModelProfile` 公开模型的能力和限制，使调用代码可以在运行时检查提供商的支持标志，而无需硬编码特定于提供商的知识。

## `ModelProfile` 结构体

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

| 字段 | 类型 | 描述 |
|-------|------|-------------|
| `name` | `String` | 模型标识符（例如 `"gpt-4o"`、`"claude-3-opus"`） |
| `provider` | `String` | 提供商名称（例如 `"openai"`、`"anthropic"`） |
| `supports_tool_calling` | `bool` | 模型是否能处理请求中的 `ToolDefinition` |
| `supports_structured_output` | `bool` | 模型是否支持 JSON Schema 约束 |
| `supports_streaming` | `bool` | `stream_chat()` 是否能产生真正的 token 级别 chunk |
| `max_input_tokens` | `Option<usize>` | 最大上下文窗口大小（如已知） |
| `max_output_tokens` | `Option<usize>` | 最大生成长度（如已知） |

## 查询模型的 Profile

每个 `ChatModel` 实现都公开了 `profile()` 方法，返回 `Option<ModelProfile>`。默认实现返回 `None`，因此提供商通过覆盖该方法来选择启用：

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

## 使用 Profile 进行能力检查

在编写跨多个提供商的通用代码时，Profile 非常有用。例如，您可以在能力检查后有条件地执行 Tool 调用或 Structured Output 逻辑：

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

## 为自定义模型实现 `profile()`

如果您实现了自己的 `ChatModel`，可以覆盖 `profile()` 来声明模型能力：

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
