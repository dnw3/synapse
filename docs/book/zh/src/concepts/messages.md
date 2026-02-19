# 消息

消息是 Synaptic 中通信的基本单元。与 LLM 的每一次交互 -- 无论是简单的提问、多轮对话、工具调用还是流式响应 -- 都以消息序列的形式表达。本页介绍消息系统的设计、变体以及操作消息序列的工具函数。

## Message 标签枚举

`Message` 是一个 Rust 枚举，包含六种变体，使用 `#[serde(tag = "role")]` 序列化：

| 变体 | 角色字符串 | 用途 |
|---------|-------------|---------|
| `System` | `"system"` | 向模型发出关于行为和约束的指令 |
| `Human` | `"human"` | 用户输入 |
| `AI` | `"assistant"` | 模型响应，可选携带工具调用 |
| `Tool` | `"tool"` | 工具执行的结果，通过 `tool_call_id` 关联 |
| `Chat` | 自定义 | 具有用户自定义角色的消息，用于特殊协议 |
| `Remove` | `"remove"` | 从历史记录中按 ID 移除消息的信号 |

这是一个标签枚举，而非 trait 层次结构。模式匹配是穷举的，序列化是自动的，编译器确保每条代码路径都处理了每个变体。

### 为什么选择枚举？

枚举使得构造无效消息变得不可能。AI 消息始终有一个 `tool_calls` 字段（即使为空）。Tool 消息始终有一个 `tool_call_id`。System 消息永远没有工具调用。这些不变式由类型系统而非运行时检查来保证。

## 创建消息

Synaptic 提供工厂方法而非暴露结构体字面量。这使得 API 在内部字段变更时保持稳定：

```rust
use synaptic::core::Message;

// Basic messages
let sys = Message::system("You are a helpful assistant.");
let user = Message::human("What is the weather?");
let reply = Message::ai("The weather is sunny today.");

// AI message with tool calls
let with_tools = Message::ai_with_tool_calls("Let me check.", vec![tool_call]);

// Tool result linked to a specific call
let result = Message::tool("72 degrees", "call_abc123");

// Custom role
let custom = Message::chat("moderator", "This message is approved.");

// Removal signal
let remove = Message::remove("msg_id_to_remove");
```

### 构建器方法

工厂方法使用默认（空）的可选字段创建消息。构建器方法允许你设置这些字段：

```rust
let msg = Message::human("Hello")
    .with_id("msg_001")
    .with_name("Alice")
    .with_content_blocks(vec![
        ContentBlock::Text { text: "Hello".into() },
        ContentBlock::Image { url: "https://example.com/photo.jpg".into(), detail: None },
    ]);
```

可用的构建器方法：`with_id()`、`with_name()`、`with_additional_kwarg()`、`with_response_metadata_entry()`、`with_content_blocks()`、`with_usage_metadata()`（仅 AI 消息）。

## 访问消息字段

访问器方法在所有变体上统一工作：

```rust
let msg = Message::ai("Hello world");

msg.content()       // "Hello world"
msg.role()          // "assistant"
msg.is_ai()         // true
msg.is_human()      // false
msg.tool_calls()    // &[] (empty slice for non-AI messages)
msg.tool_call_id()  // None (only Some for Tool messages)
msg.id()            // None (unless set with .with_id())
msg.name()          // None (unless set with .with_name())
```

类型检查方法：`is_system()`、`is_human()`、`is_ai()`、`is_tool()`、`is_chat()`、`is_remove()`。

`Remove` 变体比较特殊：它只携带一个 `id` 字段。对其调用 `content()` 返回 `""`，`name()` 返回 `None`。`remove_id()` 方法仅对 Remove 消息返回 `Some(&str)`。

## 公共字段

每个消息变体（`Remove` 除外）都携带以下字段：

- **`content: String`** -- 文本内容
- **`id: Option<String>`** -- 可选的唯一标识符
- **`name: Option<String>`** -- 可选的发送者名称
- **`additional_kwargs: HashMap<String, Value>`** -- 可扩展的键值元数据
- **`response_metadata: HashMap<String, Value>`** -- 提供商特定的响应元数据
- **`content_blocks: Vec<ContentBlock>`** -- 多模态内容（文本、图片、音频、视频、文件、数据、推理）

AI 变体额外携带：
- **`tool_calls: Vec<ToolCall>`** -- 结构化的工具调用
- **`invalid_tool_calls: Vec<InvalidToolCall>`** -- 解析失败的工具调用
- **`usage_metadata: Option<TokenUsage>`** -- 提供商返回的 token 使用量

Tool 变体额外携带：
- **`tool_call_id: String`** -- 关联回产生此结果的 ToolCall

## 使用 AIMessageChunk 进行流式传输

从 LLM 流式接收响应时，内容以分块方式到达。`AIMessageChunk` 结构体表示单个分块：

```rust
pub struct AIMessageChunk {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
    pub id: Option<String>,
    pub tool_call_chunks: Vec<ToolCallChunk>,
    pub invalid_tool_calls: Vec<InvalidToolCall>,
}
```

分块支持 `+` 和 `+=` 运算符进行增量合并：

```rust
let mut accumulated = AIMessageChunk::default();
accumulated += chunk1;  // content is concatenated
accumulated += chunk2;  // tool_calls are extended
accumulated += chunk3;  // usage is summed

// Convert the accumulated chunk to a Message
let message = accumulated.into_message();
```

合并语义如下：
- `content` 通过 `push_str` 拼接
- `tool_calls`、`tool_call_chunks` 和 `invalid_tool_calls` 被扩展
- `id` 取第一个非 None 的值
- `usage` 按字段逐一相加（input_tokens、output_tokens、total_tokens）

## 多模态内容

`ContentBlock` 枚举支持纯文本之外的富内容类型：

| 变体 | 字段 | 用途 |
|---------|--------|---------|
| `Text` | `text` | 纯文本 |
| `Image` | `url`, `detail` | 图片引用，可选详细级别 |
| `Audio` | `url` | 音频引用 |
| `Video` | `url` | 视频引用 |
| `File` | `url`, `mime_type` | 通用文件引用 |
| `Data` | `data: Value` | 任意结构化数据 |
| `Reasoning` | `content` | 模型推理/思维链 |

内容块与 `content` 字符串字段一起携带，允许消息同时包含文本摘要和结构化的多模态数据。

## 消息工具函数

Synaptic 提供四个用于操作消息序列的工具函数：

### filter_messages

通过角色、名称或 ID 进行包含/排除过滤：

```rust
use synaptic::core::filter_messages;

let humans_only = filter_messages(
    &messages,
    Some(&["human"]),  // include_types
    None,              // exclude_types
    None, None,        // include/exclude names
    None, None,        // include/exclude ids
);
```

### trim_messages

将消息序列裁剪到 token 预算以内：

```rust
use synaptic::core::{trim_messages, TrimStrategy};

let trimmed = trim_messages(
    messages,
    4096,                       // max tokens
    |msg| msg.content().len() / 4,  // token counter function
    TrimStrategy::Last,         // keep most recent
    true,                       // always preserve system message
);
```

`TrimStrategy::First` 保留开头的消息。`TrimStrategy::Last` 保留末尾的消息，可选保留前导的 system 消息。

### merge_message_runs

合并相同角色的连续消息为单条消息：

```rust
use synaptic::core::merge_message_runs;

let merged = merge_message_runs(vec![
    Message::human("Hello"),
    Message::human("How are you?"),
    Message::ai("I'm fine"),
]);
// Result: [Human("Hello\nHow are you?"), AI("I'm fine")]
```

对于 AI 消息，工具调用和无效工具调用也会被合并。

### get_buffer_string

将消息序列转换为人类可读的字符串：

```rust
use synaptic::core::get_buffer_string;

let text = get_buffer_string(&messages, "Human", "AI");
// "System: You are helpful.\nHuman: Hello\nAI: Hi there!"
```

## 序列化

消息以 JSON 序列化，使用 `role` 作为鉴别字段：

```json
{
  "role": "assistant",
  "content": "Hello!",
  "tool_calls": [],
  "id": null,
  "name": null
}
```

AI 变体将其角色序列化为 `"assistant"`（与 OpenAI 惯例一致），运行时 `role()` 同样返回 `"assistant"`。空集合和 None 的可选值通过 `skip_serializing_if` 属性在序列化时被省略。

此序列化格式与 LangChain 的消息模式兼容，便于在 Synaptic 和基于 Python 的系统之间交换消息历史。

## 参考

- [消息类型](../how-to/messages/types.md) -- 每种消息变体的详细示例
- [过滤与裁剪](../how-to/messages/filter-trim.md) -- 消息序列的过滤和裁剪
- [合并连续消息](../how-to/messages/merge-runs.md) -- 合并相同角色的连续消息
- [记忆](memory.md) -- 消息如何跨会话存储和管理
