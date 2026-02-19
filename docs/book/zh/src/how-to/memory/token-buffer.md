# 令牌缓冲记忆

`ConversationTokenBufferMemory` 保留符合令牌预算的最近消息。在 `load()` 时，最早的消息会被丢弃，直到估算的总令牌数达到或低于 `max_tokens`。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationTokenBufferMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message};

let store = Arc::new(InMemoryStore::new());

// 将消息保持在 200 令牌的预算内
let memory = ConversationTokenBufferMemory::new(store, 200);

let session = "user-1";

memory.append(session, Message::human("Hello!")).await?;
memory.append(session, Message::ai("Hi! How can I help?")).await?;
memory.append(session, Message::human("Tell me a long story about Rust.")).await?;
memory.append(session, Message::ai("Rust began as a personal project...")).await?;

let history = memory.load(session).await?;
// 只返回符合 200 估算令牌的消息。
// 最早的消息优先被丢弃。
```

## 工作原理

- **`append()`** 将每条消息不做修改地存储到底层 `MemoryStore` 中。
- **`load()`** 获取所有消息，估算总令牌数，然后逐条移除最早的消息，直到总数符合 `max_tokens`。
- **`clear()`** 移除该会话在底层 store 中的所有消息。

### 令牌估算

Synaptic 使用大约每 4 个字符对应 1 个令牌的简单启发式方法，每条消息最少计为 1 个令牌：

```rust
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4 + 1
}
```

这是一个粗略的近似值。实际令牌数因模型和分词器而异。该启发式方法有意偏向保守（略微高估），以避免超出实际的令牌限制。

## 参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `store` | `Arc<dyn MemoryStore>` | 存储原始消息的后端 store |
| `max_tokens` | `usize` | `load()` 返回的最大估算令牌数 |

## 何时使用

令牌缓冲记忆适用于以下场景：

- 你需要以令牌数而非消息数来控制提示大小。
- 你希望在不手动计数消息的情况下，保持在模型的上下文窗口范围内。
- 你偏好不调用 LLM 的简单策略来管理记忆大小。

## 权衡

- **近似** -- 令牌估算是启发式的，并非精确计数。要实现精确的令牌预算控制，需要使用特定模型的分词器。
- **硬截断** -- 被丢弃的消息完全丢失，不会有摘要或压缩的历史表示。
- **按整条消息丢弃** -- 如果某条消息非常长，它可能会独占大部分预算。

如需按固定消息数而非令牌预算来管理，请参阅[窗口记忆](window.md)。如需通过摘要保留较早上下文的策略，请参阅[摘要记忆](summary.md)或[摘要缓冲记忆](summary-buffer.md)。
