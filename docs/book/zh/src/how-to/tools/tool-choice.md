# Tool Choice

`ToolChoice` 控制 Chat Model 在响应时是否以及如何选择工具。它定义在 `synaptic-core` 中，通过 `with_tool_choice()` 构建器方法附加到 `ChatRequest` 上。

## ToolChoice 变体

| 变体 | 行为 |
|------|------|
| `ToolChoice::Auto` | 由模型决定是调用工具还是以文本形式响应（提供工具时的默认行为） |
| `ToolChoice::Required` | 模型必须调用至少一个工具——不能以纯文本形式响应 |
| `ToolChoice::None` | 模型不得调用任何工具，即使请求中提供了工具 |
| `ToolChoice::Specific(name)` | 模型必须调用指定名称的工具 |

## 基本用法

将 `ToolChoice` 与工具定义一起附加到 `ChatRequest`：

```rust
use serde_json::json;
use synaptic_core::{ChatRequest, Message, ToolChoice, ToolDefinition};

let weather_tool = ToolDefinition {
    name: "get_weather".to_string(),
    description: "Get the current weather for a location".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "location": { "type": "string" }
        },
        "required": ["location"]
    }),
};

// Force the model to use tools
let request = ChatRequest::new(vec![
    Message::human("What is the weather in Tokyo?"),
])
.with_tools(vec![weather_tool])
.with_tool_choice(ToolChoice::Required);
```

## 各变体的使用场景

### Auto（默认）

让模型自行决定。这是通用 Agent 的最佳选择，当不需要工具时应以文本形式响应：

```rust
use synaptic_core::{ChatRequest, Message, ToolChoice};

let request = ChatRequest::new(vec![
    Message::human("Hello, how are you?"),
])
.with_tools(tool_defs)
.with_tool_choice(ToolChoice::Auto);
```

### Required

强制使用工具。适用于 Agent 循环中下一步必须是工具调用的场景，或者当你确定用户的请求需要调用工具时：

```rust
use synaptic_core::{ChatRequest, Message, ToolChoice};

let request = ChatRequest::new(vec![
    Message::human("Look up the weather in Paris and Tokyo."),
])
.with_tools(tool_defs)
.with_tool_choice(ToolChoice::Required);
// The model MUST respond with one or more tool calls
```

### None

禁止工具调用。适用于你想在不从请求中移除工具的情况下临时禁用工具，或者在最终总结步骤中使用：

```rust
use synaptic_core::{ChatRequest, Message, ToolChoice};

let request = ChatRequest::new(vec![
    Message::system("Summarize the tool results for the user."),
    Message::human("What is the weather?"),
    // ... tool result messages ...
])
.with_tools(tool_defs)
.with_tool_choice(ToolChoice::None);
// The model MUST respond with text, not tool calls
```

### Specific

强制使用特定工具。适用于你明确知道应该调用哪个工具的场景：

```rust
use synaptic_core::{ChatRequest, Message, ToolChoice};

let request = ChatRequest::new(vec![
    Message::human("Check the weather in London."),
])
.with_tools(tool_defs)
.with_tool_choice(ToolChoice::Specific("get_weather".to_string()));
// The model MUST call the "get_weather" tool specifically
```

## 完整示例

以下是一个完整示例，创建工具、强制调用特定工具并处理结果：

```rust
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{
    ChatModel, ChatRequest, Message, SynapticError, Tool,
    ToolChoice, ToolDefinition,
};
use synaptic_tools::{ToolRegistry, SerialToolExecutor};

// Define the tool
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str { "calculator" }
    fn description(&self) -> &'static str { "Perform arithmetic calculations" }
    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let expr = args["expression"].as_str().unwrap_or("");
        // Simplified: in production, parse and evaluate the expression
        Ok(json!({"result": expr}))
    }
}

// Register tools
let registry = ToolRegistry::new();
registry.register(Arc::new(CalculatorTool))?;

// Build the tool definition for the model
let calc_def = ToolDefinition {
    name: "calculator".to_string(),
    description: "Perform arithmetic calculations".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "The arithmetic expression to evaluate"
            }
        },
        "required": ["expression"]
    }),
};

// Build a request that forces the calculator tool
let request = ChatRequest::new(vec![
    Message::human("What is 42 * 17?"),
])
.with_tools(vec![calc_def])
.with_tool_choice(ToolChoice::Specific("calculator".to_string()));

// Send to the model, then execute the returned tool calls
let response = model.chat(request).await?;
for tc in response.message.tool_calls() {
    let executor = SerialToolExecutor::new(registry.clone());
    let result = executor.execute(&tc.name, tc.arguments.clone()).await?;
    println!("Tool {} returned: {}", tc.name, result);
}
```

## 提供商支持

所有 Synaptic 提供商适配器（`OpenAiChatModel`、`AnthropicChatModel`、`GeminiChatModel`、`OllamaChatModel`）都支持 `ToolChoice`。适配器会自动将 Synaptic 的 `ToolChoice` 枚举转换为提供商特定的格式。

另请参见：[绑定工具](../chat-models/bind-tools.md)了解如何将工具永久附加到模型，以及 [ReAct Agent 教程](../../tutorials/react-agent.md)获取完整的 Agent 循环示例。
