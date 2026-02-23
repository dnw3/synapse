# Memory

Synaptic 通过 `MemoryStore` trait 和一系列 Memory 策略提供基于 Session 的对话记忆功能，这些策略控制对话历史的存储、裁剪和摘要方式。

## `MemoryStore` Trait

所有 Memory 策略都实现了 `MemoryStore` trait，该 trait 定义了三个异步操作：

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError>;
    async fn clear(&self, session_id: &str) -> Result<(), SynapticError>;
}
```

- **`append`** -- 向 Session 的历史记录中添加一条消息。
- **`load`** -- 检索某个 Session 的对话历史。
- **`clear`** -- 移除某个 Session 的所有消息。

每个操作都通过 `session_id` 字符串进行键控，将不同对话彼此隔离。你可以自行选择 Session 键（用户 ID、线程 ID、UUID——任何适合你应用的标识符）。

## `ChatMessageHistory`

最简单的 `MemoryStore` 实现是 `ChatMessageHistory`，它基于统一的 `Store` 后端存储消息：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::core::{MemoryStore, Message};

let store = ChatMessageHistory::new(Arc::new(InMemoryStore::new()));

store.append("session-1", Message::human("Hello")).await?;
store.append("session-1", Message::ai("Hi there!")).await?;

let history = store.load("session-1").await?;
assert_eq!(history.len(), 2);

// Different sessions are completely isolated
let other = store.load("session-2").await?;
assert!(other.is_empty());
```

`ChatMessageHistory` 通常用作下面描述的高级 Memory 策略的后端存储。通过传入不同的 `Store` 实现（如 `InMemoryStore`、`FileStore`），可以灵活选择存储方式。

## Memory 策略

每种 Memory 策略都包装了一个底层的 `MemoryStore`，并在加载消息时应用不同的策略。所有策略本身也实现了 `MemoryStore`，因此在任何需要 `MemoryStore` 的地方都可以互换使用。

| 策略 | 行为 | 适用场景 |
|------|------|----------|
| [Buffer Memory](buffer.md) | 保留完整的对话历史 | 完整上下文很重要的短对话 |
| [Window Memory](window.md) | 仅保留最近的 K 条消息 | 较旧上下文不太相关的聊天界面 |
| [Summary Memory](summary.md) | 使用 LLM 对较旧消息进行摘要 | 需要紧凑历史的超长对话 |
| [Token Buffer Memory](token-buffer.md) | 在 Token 预算内保留最近的消息 | 成本控制和 prompt 大小限制 |
| [Summary Buffer Memory](summary-buffer.md) | 混合模式——对旧消息摘要，保留最近消息原文 | 上下文与效率的最佳平衡 |

## 自动管理历史记录

对于在链式调用前加载历史记录、调用后保存结果这一常见模式，Synaptic 提供了 [RunnableWithMessageHistory](runnable-with-history.md)。它包装任何 `Runnable<Vec<Message>, String>`，并通过 `RunnableConfig` 元数据中的 Session ID 自动处理加载/保存的生命周期。

## 选择策略

- 如果对话较短（少于 20 条消息），**Buffer Memory** 是最简单的选择。
- 如果你希望可预测的内存使用且无需 LLM 调用，请使用 **Window Memory** 或 **Token Buffer Memory**。
- 如果对话很长，且你需要以压缩形式保留完整上下文，请使用 **Summary Memory**。
- 如果你想兼得两者的优势——精确的最近消息加上旧历史的压缩摘要——请使用 **Summary Buffer Memory**。
