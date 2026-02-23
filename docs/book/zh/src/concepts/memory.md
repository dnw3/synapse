# 记忆

没有记忆，每次 LLM 调用都是无状态的——模型对之前的交互一无所知。Synaptic 中的记忆通过存储、检索和管理对话历史来解决这个问题，使后续调用能包含相关的上下文。本页解释记忆抽象、可用的策略，以及它们在完整性与成本之间的权衡。

## MemoryStore Trait

所有记忆后端实现一个统一的 trait：

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError>;
    async fn clear(&self, session_id: &str) -> Result<(), SynapticError>;
}
```

三个操作，以会话标识符为键：
- **`append`** -- 将消息添加到会话的历史记录中
- **`load`** -- 检索会话的完整历史记录
- **`clear`** -- 删除会话的所有消息

`session_id` 参数是 Synaptic 记忆设计的核心。具有不同 session ID 的两个对话是完全隔离的，即使它们共享相同的记忆存储实例。这使得多租户应用成为可能，多个用户可以通过同一个系统并发交互。

## ChatMessageHistory

`ChatMessageHistory` 是标准的 `MemoryStore` 实现。它以任意 `Store` 作为后端——存储后端是可插拔的，因此你可以在内存、文件或自定义存储之间切换，而无需更改记忆代码：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let memory = ChatMessageHistory::new(store);
memory.append("session_1", Message::human("Hello")).await?;
let history = memory.load("session_1").await?;
```

以 `InMemoryStore` 为后端的 `ChatMessageHistory` 速度快，不需要外部依赖，适用于开发、测试和短生命周期的应用。进程退出时数据会丢失。

如需持久化，可将 `InMemoryStore` 替换为 `FileStore`：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::FileStore;

let store = Arc::new(FileStore::new("./chat_history"));
let memory = ChatMessageHistory::new(store);
```

`FileStore` 将数据写入磁盘，因此对话历史在进程重启后仍然保留。你的其余代码完全不需要改动——只有 store 构造函数发生了变化。

## 记忆策略

原始的 `MemoryStore` 会永久保留每条消息。对于长对话，这会导致无限增长的 token 使用量，最终超过模型的上下文窗口。记忆策略包装一个存储，并控制哪些消息被包含在上下文中。

### ConversationBufferMemory

保留所有消息。最简单的策略——每次都将所有内容发送给 LLM。

- **优势**：不丢失任何信息。
- **劣势**：token 使用量无限增长。最终会超过上下文窗口。
- **适用场景**：你确定消息总量较少的短对话。

### ConversationWindowMemory

只保留最近的 K 对消息（人类 + AI）。更早的消息会被丢弃：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::memory::ConversationWindowMemory;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);
let memory = ConversationWindowMemory::new(history, 5); // keep last 5 exchanges
```

- **优势**：固定且可预测的 token 使用量。
- **劣势**：完全丢失旧的上下文。模型对 K 轮之前发生的事情一无所知。
- **适用场景**：聊天 UI、客服机器人，以及任何最近上下文最重要的场景。

### ConversationSummaryMemory

使用 LLM 对旧消息进行摘要，只保留摘要加上近期消息：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::memory::ConversationSummaryMemory;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);
let memory = ConversationSummaryMemory::new(history, summarizer_model);
```

每次交互后，该策略使用 LLM 生成对话的滚动摘要。摘要替代了旧消息，因此发送给主模型的上下文包含摘要加上近期消息。

- **优势**：保留整个对话的要旨。token 使用量近似恒定。
- **劣势**：摘要有成本（额外的 LLM 调用）。细节可能在压缩中丢失。摘要质量取决于模型。
- **适用场景**：历史上下文很重要的长期对话（例如，能记住过去偏好的多会话助手）。

### ConversationTokenBufferMemory

在 token 预算内保留尽可能多的近期消息：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::memory::ConversationTokenBufferMemory;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);
let memory = ConversationTokenBufferMemory::new(history, 4096); // max 4096 tokens
```

与窗口记忆（按消息数量计算）不同，token 缓冲记忆按 token 数量计算。当消息长度差异较大时，这种方式更精确。

- **优势**：直接控制上下文大小。适用于有严格上下文限制的模型。
- **劣势**：仍然会完全丢失旧消息。
- **适用场景**：对成本敏感的应用，希望高效利用上下文窗口。

### ConversationSummaryBufferMemory

一种混合策略：对旧消息进行摘要并保留近期消息，通过 token 阈值控制边界：

```rust
use std::sync::Arc;
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::memory::ConversationSummaryBufferMemory;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);
let memory = ConversationSummaryBufferMemory::new(history, model, 2000);
// Summarize when recent messages exceed 2000 tokens
```

当近期消息的总 token 数超过阈值时，最旧的消息会被摘要并替换为摘要内容。结果是一个以远期历史摘要开头、接着是逐字的近期消息的上下文。

- **优势**：两全其美——通过摘要保留旧上下文，同时保持近期消息的原始内容。
- **劣势**：更复杂。需要 LLM 进行摘要。
- **适用场景**：需要历史感知和准确近期上下文的生产聊天应用。

## 策略对比

| 策略 | 保留内容 | Token 增长 | 信息丢失 | 额外 LLM 调用 |
|----------|---------------|-------------|-----------|-----------------|
| Buffer | 所有内容 | 无限制 | 无 | 无 |
| Window | 最近 K 轮 | 固定 | 旧消息丢失 | 无 |
| Summary | 摘要 + 近期 | 近似恒定 | 细节被压缩 | 有 |
| TokenBuffer | 预算内的近期消息 | 固定 | 旧消息丢失 | 无 |
| SummaryBuffer | 摘要 + 近期缓冲 | 有界 | 旧细节被压缩 | 有 |

## RunnableWithMessageHistory

与手动在每次 LLM 调用前后加载和保存消息不同，`RunnableWithMessageHistory` 包装任意 `Runnable` 并自动处理：

```rust
use synaptic::memory::RunnableWithMessageHistory;

let chain_with_memory = RunnableWithMessageHistory::new(
    my_chain,
    store,
    |config| config.metadata.get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string(),
);
```

每次调用时：
1. 从 `RunnableConfig` 元数据中提取 session ID。
2. 从存储中加载历史消息。
3. 将历史上下文前置后调用内部 Runnable。
4. 将新消息（输入和输出）追加到存储中。

这将记忆管理与应用逻辑分离。内部 Runnable 完全不需要了解记忆。

## 会话隔离

一个关键的设计特性：记忆始终以会话为范围。`session_id` 只是一个字符串——它可以是用户 ID、对话 ID、线程 ID，或者对你的应用有意义的任何其他标识符。

共享同一个 `ChatMessageHistory`（或任何其他存储）的不同会话是完全独立的。向会话 "alice" 追加消息永远不会影响会话 "bob"。这使得在为多个用户提供服务的整个应用中使用单个存储实例是安全的。

## 参见

- [Buffer Memory](../how-to/memory/buffer.md) -- 保留所有消息
- [Window Memory](../how-to/memory/window.md) -- 保留最近 K 轮
- [Summary Memory](../how-to/memory/summary.md) -- 基于 LLM 的摘要
- [Token Buffer Memory](../how-to/memory/token-buffer.md) -- 基于 token 预算的裁剪
- [Summary Buffer Memory](../how-to/memory/summary-buffer.md) -- 混合摘要 + 近期缓冲
- [RunnableWithMessageHistory](../how-to/memory/runnable-with-history.md) -- 自动历史管理
- [消息](messages.md) -- 记忆存储的 Message 类型
