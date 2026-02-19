# 消息类型

本指南涵盖 Synaptic 中所有消息变体、如何创建它们，以及如何检查其内容。

## `Message` 枚举

`Message` 是一个标签枚举（`#[serde(tag = "role")]`），包含六个变体：

| 变体 | 工厂方法 | 角色字符串 | 用途 |
|---------|---------------|-------------|---------|
| `System` | `Message::system()` | `"system"` | 模型的系统指令 |
| `Human` | `Message::human()` | `"human"` | 用户输入 |
| `AI` | `Message::ai()` | `"assistant"` | 模型响应（纯文本） |
| `AI`（带工具调用） | `Message::ai_with_tool_calls()` | `"assistant"` | 带工具调用的模型响应 |
| `Tool` | `Message::tool()` | `"tool"` | 工具执行结果 |
| `Chat` | `Message::chat()` | 自定义 | 自定义角色消息 |
| `Remove` | `Message::remove()` | `"remove"` | 按 ID 移除消息的信号 |

## 创建消息

始终使用工厂方法，而不是直接构造枚举变体：

```rust
use synaptic_core::{Message, ToolCall};
use serde_json::json;

// System message -- sets the model's behavior
let system = Message::system("You are a helpful assistant.");

// Human message -- user input
let human = Message::human("Hello, how are you?");

// AI message -- plain text response
let ai = Message::ai("I'm doing well, thanks for asking!");

// AI message with tool calls
let ai_tools = Message::ai_with_tool_calls(
    "Let me look that up for you.",
    vec![
        ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: json!({"query": "Rust programming"}),
        },
    ],
);

// Tool message -- result of a tool execution
// Second argument is the tool_call_id, which must match the ToolCall's id
let tool = Message::tool("Found 42 results for 'Rust programming'", "call_1");

// Chat message -- custom role
let chat = Message::chat("moderator", "This conversation is on topic.");

// Remove message -- used in message history management
let remove = Message::remove("msg-id-to-remove");
```

## 访问器方法

所有消息变体共享一组通用的访问器方法：

```rust
use synaptic_core::Message;

let msg = Message::human("Hello!");

// Get the role as a string
assert_eq!(msg.role(), "human");

// Get the text content
assert_eq!(msg.content(), "Hello!");

// Type-checking predicates
assert!(msg.is_human());
assert!(!msg.is_ai());
assert!(!msg.is_system());
assert!(!msg.is_tool());
assert!(!msg.is_chat());
assert!(!msg.is_remove());

// Tool-related accessors (empty/None for non-AI/non-Tool messages)
assert!(msg.tool_calls().is_empty());
assert!(msg.tool_call_id().is_none());

// Optional fields
assert!(msg.id().is_none());
assert!(msg.name().is_none());
```

### 工具调用访问器

```rust
use synaptic_core::{Message, ToolCall};
use serde_json::json;

let ai = Message::ai_with_tool_calls("", vec![
    ToolCall {
        id: "call_1".into(),
        name: "search".into(),
        arguments: json!({"q": "rust"}),
    },
]);

// Get all tool calls (only meaningful for AI messages)
let calls = ai.tool_calls();
assert_eq!(calls.len(), 1);
assert_eq!(calls[0].name, "search");

let tool_msg = Message::tool("result", "call_1");

// Get the tool_call_id (only meaningful for Tool messages)
assert_eq!(tool_msg.tool_call_id(), Some("call_1"));
```

## 构建器方法

消息支持构建器模式来设置可选字段：

```rust
use synaptic_core::Message;
use serde_json::json;

let msg = Message::human("Hello!")
    .with_id("msg-001")
    .with_name("Alice")
    .with_additional_kwarg("source", json!("web"))
    .with_response_metadata_entry("model", json!("gpt-4o"));

assert_eq!(msg.id(), Some("msg-001"));
assert_eq!(msg.name(), Some("Alice"));
```

可用的构建器方法：

| 方法 | 描述 |
|--------|-------------|
| `.with_id(id)` | 设置消息 ID |
| `.with_name(name)` | 设置发送者名称 |
| `.with_additional_kwarg(key, value)` | 添加任意键值对 |
| `.with_response_metadata_entry(key, value)` | 添加响应元数据 |
| `.with_content_blocks(blocks)` | 设置多模态内容块 |
| `.with_usage_metadata(usage)` | 设置 token 用量（仅限 AI 消息） |

## 序列化

消息序列化为带有 `"role"` 标签的 JSON：

```rust
use synaptic_core::Message;

let msg = Message::human("Hello!");
let json = serde_json::to_string_pretty(&msg).unwrap();
// {
//   "role": "human",
//   "content": "Hello!"
// }
```

注意 AI 变体序列化时使用 `"role": "assistant"`（而非 `"ai"`），与大多数 LLM 提供商使用的约定保持一致。
