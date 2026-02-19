# 为模型绑定 Tool

本指南展示如何在聊天请求中包含 Tool（函数）定义，以便模型决定是否调用它们。

## 定义 Tool

`ToolDefinition` 描述了模型可以调用的 Tool。它包含名称、描述和参数的 JSON Schema：

```rust
use synaptic_core::ToolDefinition;
use serde_json::json;

let weather_tool = ToolDefinition {
    name: "get_weather".to_string(),
    description: "Get the current weather for a location".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "City name, e.g. 'Tokyo'"
            }
        },
        "required": ["location"]
    }),
};
```

## 随请求发送 Tool

使用 `ChatRequest::with_tools()` 将 Tool 定义附加到单个请求：

```rust
use synaptic_core::{ChatModel, ChatRequest, Message, ToolDefinition};
use serde_json::json;

async fn call_with_tools(model: &dyn ChatModel) -> Result<(), Box<dyn std::error::Error>> {
    let tool_def = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["location"]
        }),
    };

    let request = ChatRequest::new(vec![
        Message::human("What's the weather in Tokyo?"),
    ]).with_tools(vec![tool_def]);

    let response = model.chat(request).await?;

    // Check if the model decided to call any tools
    for tc in response.message.tool_calls() {
        println!("Tool: {}, Args: {}", tc.name, tc.arguments);
    }

    Ok(())
}
```

## 处理 Tool 调用

当模型返回 Tool 调用时，每个 `ToolCall` 包含：

- `id` -- 本次调用的唯一标识符（用于将 Tool 结果匹配回去）
- `name` -- 要调用的 Tool 名称
- `arguments` -- 包含参数的 `serde_json::Value`

执行 Tool 后，将结果作为 `Tool` 消息发回：

```rust
use synaptic_core::{ChatRequest, Message, ToolCall};
use serde_json::json;

// Suppose the model returned a tool call
let tool_call = ToolCall {
    id: "call_123".to_string(),
    name: "get_weather".to_string(),
    arguments: json!({"location": "Tokyo"}),
};

// Execute your tool logic...
let result = "Sunny, 22C";

// Send the result back in a follow-up request
let messages = vec![
    Message::human("What's the weather in Tokyo?"),
    Message::ai_with_tool_calls("", vec![tool_call]),
    Message::tool(result, "call_123"),  // tool_call_id must match
];

let follow_up = ChatRequest::new(messages);
// let final_response = model.chat(follow_up).await?;
```

## 使用 `BoundToolsChatModel` 永久绑定 Tool

如果希望通过模型发送的每个请求都自动包含特定的 Tool 定义，可以使用 `BoundToolsChatModel`：

```rust
use std::sync::Arc;
use synaptic_core::{ChatModel, ChatRequest, Message, ToolDefinition};
use synaptic_models::BoundToolsChatModel;
use serde_json::json;

let tools = vec![
    ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get weather for a city".to_string(),
        parameters: json!({"type": "object", "properties": {"city": {"type": "string"}}}),
    },
    ToolDefinition {
        name: "search".to_string(),
        description: "Search the web".to_string(),
        parameters: json!({"type": "object", "properties": {"query": {"type": "string"}}}),
    },
];

let base_model: Arc<dyn ChatModel> = Arc::new(base_model);
let bound = BoundToolsChatModel::new(base_model, tools);

// Now every call to bound.chat() will include both tools automatically
let request = ChatRequest::new(vec![Message::human("Look up Rust news")]);
// let response = bound.chat(request).await?;
```

## 多个 Tool

您可以提供任意数量的 Tool。模型会根据对话上下文选择调用哪些（如果需要的话）：

```rust
let request = ChatRequest::new(vec![
    Message::human("Search for Rust news and tell me the weather in Berlin"),
]).with_tools(vec![search_tool, weather_tool, calculator_tool]);
```

另请参阅：[控制 Tool Choice](tool-choice.md)，了解如何精细控制模型使用哪些 Tool。
