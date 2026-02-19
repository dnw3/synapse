# Summary Memory

`ConversationSummaryMemory` 使用 LLM 将较旧的消息压缩成滚动摘要。最近的消息保持原文，而超过 `buffer_size` 阈值的内容会被摘要为一条系统消息。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationSummaryMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message, ChatModel};

// You need a ChatModel to generate summaries
let model: Arc<dyn ChatModel> = Arc::new(my_model);
let store = Arc::new(InMemoryStore::new());

// Keep the last 4 messages verbatim; summarize older ones
let memory = ConversationSummaryMemory::new(store, model, 4);

let session = "user-1";

// As messages accumulate beyond buffer_size * 2, summarization triggers
memory.append(session, Message::human("Tell me about Rust.")).await?;
memory.append(session, Message::ai("Rust is a systems programming language...")).await?;
memory.append(session, Message::human("What about ownership?")).await?;
memory.append(session, Message::ai("Ownership is Rust's core memory model...")).await?;
// ... more messages ...

let history = memory.load(session).await?;
// If summarization has occurred, history starts with a system message
// containing the summary, followed by the most recent messages.
```

## 工作原理

1. **`append()`** 将消息存储到底层 Store 中，然后检查消息总数。
2. 当消息数超过 `buffer_size * 2` 时，策略将消息分为"较旧"和"最近"两部分（最后 `buffer_size` 条消息为最近部分）。
3. 较旧的消息被发送给 `ChatModel`，附带一个要求简洁摘要的 prompt。如果之前已有摘要存在，它会作为新摘要的上下文一起提供。
4. Store 被清空，仅重新填入最近的消息。
5. **`load()`** 返回存储的消息，并在前面加上一条包含摘要文本的系统消息（如果存在摘要）：

   ```
   Summary of earlier conversation: <summary text>
   ```

6. **`clear()`** 移除该 Session 的存储消息和摘要。

## 参数

| 参数 | 类型 | 描述 |
|------|------|------|
| `store` | `Arc<dyn MemoryStore>` | 原始消息的后端存储 |
| `model` | `Arc<dyn ChatModel>` | 用于生成摘要的 LLM |
| `buffer_size` | `usize` | 保持原文的最近消息数量 |

## 适用场景

Summary Memory 适合以下情况：

- 对话非常长，你需要保留整个历史记录的上下文。
- 你可以承受摘要所需的额外 LLM 调用（仅在缓冲区溢出时触发，不是每次 append 都触发）。
- 你希望无论对话持续多长时间，Token 使用量大致保持恒定。

## 权衡

- **有损压缩** -- 摘要由 LLM 生成，因此旧消息中的具体细节可能会丢失或失真。
- **额外 LLM 成本** -- 每次摘要步骤都会进行一次单独的 ChatModel 调用。用于摘要的模型可以是比主要模型更小、更便宜的模型。
- **延迟** -- 触发摘要的 `append()` 调用由于 LLM 往返会比平时慢。

如果你想要精确的最近消息且无需 LLM 调用，请使用 [Window Memory](window.md) 或 [Token Buffer Memory](token-buffer.md)。要获得兼顾精确回忆最近消息和摘要旧历史的混合方案，请参阅 [Summary Buffer Memory](summary-buffer.md)。
