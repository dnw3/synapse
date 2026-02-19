# Window Memory

`ConversationWindowMemory` 仅保留最近的 K 条消息。所有消息都存储在底层 Store 中，但 `load()` 只返回最后 `window_size` 条消息的滑动窗口。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationWindowMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message};

let store = Arc::new(InMemoryStore::new());

// Keep only the last 4 messages visible
let memory = ConversationWindowMemory::new(store, 4);

let session = "user-1";

memory.append(session, Message::human("Message 1")).await?;
memory.append(session, Message::ai("Reply 1")).await?;
memory.append(session, Message::human("Message 2")).await?;
memory.append(session, Message::ai("Reply 2")).await?;
memory.append(session, Message::human("Message 3")).await?;
memory.append(session, Message::ai("Reply 3")).await?;

let history = memory.load(session).await?;
// Only the last 4 messages are returned
assert_eq!(history.len(), 4);
assert_eq!(history[0].content(), "Message 2");
assert_eq!(history[3].content(), "Reply 3");
```

## 工作原理

- **`append()`** 将每条消息存储到底层 `MemoryStore` 中——写入时不会丢弃任何内容。
- **`load()`** 从 Store 中检索所有消息，然后仅返回最后 `window_size` 条记录。如果消息总数小于或等于 `window_size`，则返回所有消息。
- **`clear()`** 从底层 Store 中移除给定 Session 的所有消息。

窗口是在加载时应用的，而非写入时。这意味着完整的历史记录保留在后端 Store 中，如果需要可以直接访问。

## 选择 `window_size`

`window_size` 参数以单条消息为单位计数，而非消息对。一次典型的人机交互产生 2 条消息，因此 `window_size` 为 10 大约保留 5 轮对话。

选择大小时请考虑模型的上下文窗口。对于大多数模型，20 条消息的窗口通常是安全的，而 4-6 条消息的窗口适合只需最近上下文的轻量级聊天界面。

## 适用场景

Window Memory 适合以下情况：

- 你希望固定、可预测的内存使用，且无需 LLM 摘要调用。
- 较旧的上下文确实不太相关（例如，休闲聊天机器人或客服流程）。
- 你需要一个简单且易于理解的策略。

## 权衡

- **硬截断** -- 窗口外的消息对模型不可见。没有摘要或对旧历史的压缩表示。
- **无 Token 感知** -- 窗口以消息数量衡量，而非 Token 数量。少数较长的消息仍可能超出模型的上下文窗口。如果你需要 Token 级别的控制，请参阅 [Token Buffer Memory](token-buffer.md)。

对于通过摘要保留旧上下文的策略，请参阅 [Summary Memory](summary.md) 或 [Summary Buffer Memory](summary-buffer.md)。
