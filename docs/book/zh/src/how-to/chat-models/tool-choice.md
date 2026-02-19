# 控制 Tool Choice

本指南展示如何控制模型在响应请求时是否使用以及使用哪些 Tool。

## 概述

当您将 Tool 附加到 `ChatRequest` 时，模型默认自行决定是否调用其中的 Tool。`ToolChoice` 枚举允许您覆盖此行为，强制模型使用 Tool、禁止使用 Tool 或指定使用某个特定 Tool。

## `ToolChoice` 枚举

```rust
use synaptic_core::ToolChoice;

// Auto -- 模型自行决定是否使用 Tool（这是默认行为）
ToolChoice::Auto

// Required -- 模型必须调用至少一个 Tool
ToolChoice::Required

// None -- 模型不得调用任何 Tool，即使提供了 Tool
ToolChoice::None

// Specific -- 模型必须调用指定的 Tool
ToolChoice::Specific("get_weather".to_string())
```

## 在请求中设置 Tool Choice

使用 `ChatRequest::with_tool_choice()`：

```rust
use synaptic_core::{ChatRequest, Message, ToolChoice, ToolDefinition};
use serde_json::json;

let tools = vec![
    ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get weather for a city".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"]
        }),
    },
    ToolDefinition {
        name: "search".to_string(),
        description: "Search the web".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        }),
    },
];

let messages = vec![Message::human("What's the weather in London?")];
```

### Auto（默认）

模型自由选择是否调用 Tool：

```rust
let request = ChatRequest::new(messages.clone())
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::Auto);
```

这等同于完全不调用 `with_tool_choice()`。

### Required

强制模型调用至少一个 Tool。当您确定用户意图对应某个 Tool 调用时非常有用：

```rust
let request = ChatRequest::new(messages.clone())
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::Required);
```

### None

禁止模型调用 Tool，即使提供了 Tool 定义。当您希望临时禁用 Tool 使用但不移除定义时，这很有用：

```rust
let request = ChatRequest::new(messages.clone())
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::None);
```

### Specific

强制模型按名称调用某个特定 Tool。无论对话上下文如何，模型都会调用该 Tool：

```rust
let request = ChatRequest::new(messages.clone())
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::Specific("get_weather".to_string()));
```

## 实用模式

### 使用 Specific Tool Choice 进行路由

在构建多步骤 Agent 时，您可以通过要求使用特定的"路由器" Tool 来强制执行分类步骤：

```rust
let router_tool = ToolDefinition {
    name: "route".to_string(),
    description: "Classify the user's intent".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "intent": {
                "type": "string",
                "enum": ["weather", "search", "calculator"]
            }
        },
        "required": ["intent"]
    }),
};

let request = ChatRequest::new(vec![Message::human("What is 2 + 2?")])
    .with_tools(vec![router_tool])
    .with_tool_choice(ToolChoice::Specific("route".to_string()));
```

### 两阶段生成

第一次调用使用 `Required` 提取结构化数据，然后使用 `None` 生成自然语言响应：

```rust
// Phase 1: extract data
let extract_request = ChatRequest::new(messages.clone())
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::Required);

// Phase 2: generate response (no tools)
let respond_request = ChatRequest::new(full_conversation)
    .with_tools(tools.clone())
    .with_tool_choice(ToolChoice::None);
```
