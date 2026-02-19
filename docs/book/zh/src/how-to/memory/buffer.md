# Buffer Memory

`ConversationBufferMemory` 是最简单的 Memory 策略。它保留完整的对话历史，在 `load()` 时返回所有消息，不进行任何裁剪或摘要。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationBufferMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message};

// Create a backing store and wrap it with buffer memory
let store = Arc::new(InMemoryStore::new());
let memory = ConversationBufferMemory::new(store);

let session = "user-1";

memory.append(session, Message::human("Hello")).await?;
memory.append(session, Message::ai("Hi there!")).await?;
memory.append(session, Message::human("What is Rust?")).await?;
memory.append(session, Message::ai("Rust is a systems programming language.")).await?;

let history = memory.load(session).await?;
// Returns ALL 4 messages -- the full conversation
assert_eq!(history.len(), 4);
```

## 工作原理

`ConversationBufferMemory` 是一个简单的透传包装器。它将 `append()`、`load()` 和 `clear()` 直接委托给底层的 `MemoryStore`，不做任何修改。这里的"策略"就是：保留一切。

这使得 Buffer 策略明确且可组合。通过将你的 Store 包装在 `ConversationBufferMemory` 中，你表明这个使用点有意存储完整历史，而且之后可以替换为不同的策略（例如 `ConversationWindowMemory`），而无需更改其余代码。

## 适用场景

Buffer Memory 适合以下情况：

- 对话较短（不超过约 20 轮交互），完整历史可以舒适地放入模型的上下文窗口。
- 你需要完美回忆每条消息（例如，用于审计或评估）。
- 你正在原型开发阶段，还不需要更复杂的策略。

## 权衡

- **无界增长** -- 每条消息都会被存储和返回。对于长对话，这最终会超出模型的上下文窗口或导致高 Token 成本。
- **无压缩** -- 没有摘要或裁剪，因此每次 LLM 调用你都需要为历史中的每个 Token 付费。

如果无界增长是一个问题，可以考虑使用 [Window Memory](window.md) 获得固定大小的窗口，[Token Buffer Memory](token-buffer.md) 获得 Token 预算，或 [Summary Memory](summary.md) 获得基于 LLM 的压缩。
