# 构建带记忆的聊天机器人

本教程将引导你构建一个基于会话的聊天机器人，它能够记住对话历史。你将学习如何使用 `ChatMessageHistory` 存储和检索消息，通过 session ID 隔离对话，以及选择适合你使用场景的记忆策略。

## 前置条件

在 `Cargo.toml` 中添加所需的 Synaptic crate：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["memory", "store"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 第一步：存储和加载消息

每个聊天机器人都需要记住对话内容。Synaptic 提供了 `MemoryStore` trait 来定义这个能力，`ChatMessageHistory` 是以任意 `Store` 为后端的标准实现。这里我们使用 `InMemoryStore` 作为底层存储：

```rust
use std::sync::Arc;
use synaptic::core::{MemoryStore, Message, SynapticError};
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let store = Arc::new(InMemoryStore::new());
    let memory = ChatMessageHistory::new(store);
    let session_id = "demo-session";

    // Simulate a conversation
    memory.append(session_id, Message::human("Hello, Synaptic")).await?;
    memory.append(session_id, Message::ai("Hello! How can I help you?")).await?;
    memory.append(session_id, Message::human("What can you do?")).await?;
    memory.append(session_id, Message::ai("I can help with many tasks!")).await?;

    // Load the conversation history
    let transcript = memory.load(session_id).await?;
    for message in &transcript {
        println!("{}: {}", message.role(), message.content());
    }

    // Clear memory when done
    memory.clear(session_id).await?;
    Ok(())
}
```

输出为：

```text
human: Hello, Synaptic
ai: Hello! How can I help you?
human: What can you do?
ai: I can help with many tasks!
```

`MemoryStore` trait 定义了三个方法：

- **`append(session_id, message)`** -- 将一条消息追加到某个会话的历史中。
- **`load(session_id)`** -- 返回某个会话的所有消息，类型为 `Vec<Message>`。
- **`clear(session_id)`** -- 删除某个会话的所有消息。

## 第二步：会话隔离

每个 session ID 对应一个独立的对话历史。这就是你将多个用户或对话线程分开的方式：

```rust
use std::sync::Arc;
use synaptic::core::{MemoryStore, Message, SynapticError};
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let store = Arc::new(InMemoryStore::new());
    let memory = ChatMessageHistory::new(store);

    // Alice's conversation
    memory.append("alice", Message::human("Hi, I'm Alice")).await?;
    memory.append("alice", Message::ai("Hello, Alice!")).await?;

    // Bob's conversation (completely independent)
    memory.append("bob", Message::human("Hi, I'm Bob")).await?;
    memory.append("bob", Message::ai("Hello, Bob!")).await?;

    // Each session has its own history
    let alice_history = memory.load("alice").await?;
    let bob_history = memory.load("bob").await?;

    assert_eq!(alice_history.len(), 2);
    assert_eq!(bob_history.len(), 2);
    assert_eq!(alice_history[0].content(), "Hi, I'm Alice");
    assert_eq!(bob_history[0].content(), "Hi, I'm Bob");

    Ok(())
}
```

Session ID 是任意字符串。在 Web 应用中，你通常会使用用户 ID、对话线程 ID 或两者的组合。

## 第三步：选择记忆策略

随着对话增长，将所有消息都发送给 LLM 会变得昂贵，最终会超过上下文窗口的限制。Synaptic 提供了多种记忆策略，它们包装底层的 `MemoryStore` 并控制 `load()` 返回的内容。

### ConversationBufferMemory

保留所有消息。这是最简单的策略——一个直通包装器，让"保留所有内容"的策略变得显式：

```rust
use std::sync::Arc;
use synaptic::core::MemoryStore;
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::ConversationBufferMemory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let history = Arc::new(ChatMessageHistory::new(store));
let memory = ConversationBufferMemory::new(history);
// memory.load() returns all messages
```

最适合：短对话，需要完整历史记录的场景。

### ConversationWindowMemory

只保留最近 **K** 条消息。更早的消息仍然存储，但不会被 `load()` 返回：

```rust
use std::sync::Arc;
use synaptic::core::MemoryStore;
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::ConversationWindowMemory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let history = Arc::new(ChatMessageHistory::new(store));
let memory = ConversationWindowMemory::new(history, 10); // keep last 10 messages
// memory.load() returns at most 10 messages
```

最适合：近期上下文就够用的对话，需要可预测成本的场景。

### ConversationSummaryMemory

使用 LLM 总结较早的消息。当存储的消息数量超过 `buffer_size * 2` 时，较早的部分会被压缩为一条总结，作为系统消息插入开头：

```rust
use std::sync::Arc;
use synaptic::core::{ChatModel, MemoryStore};
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::ConversationSummaryMemory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let history = Arc::new(ChatMessageHistory::new(store));
let model: Arc<dyn ChatModel> = /* your chat model */;
let memory = ConversationSummaryMemory::new(history, model, 6);
// When messages exceed 12, older ones are summarized
// memory.load() returns: [summary system message] + [recent 6 messages]
```

最适合：长时间运行的对话，需要保留较早上下文大意但不需要完整逐字记录的场景。

### ConversationTokenBufferMemory

在 **Token 预算**内保留消息。使用可配置的 token 估算器，当总量超过限制时丢弃最早的消息：

```rust
use std::sync::Arc;
use synaptic::core::MemoryStore;
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::ConversationTokenBufferMemory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let history = Arc::new(ChatMessageHistory::new(store));
let memory = ConversationTokenBufferMemory::new(history, 4000); // 4000 token budget
// memory.load() returns as many recent messages as fit within 4000 tokens
```

最适合：需要精确控制 token 数量，确保不超过模型上下文窗口的场景。

### ConversationSummaryBufferMemory

总结和缓冲策略的混合体。保留最近的消息原文，当 token 数量超过阈值时将更早的内容总结：

```rust
use std::sync::Arc;
use synaptic::core::{ChatModel, MemoryStore};
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::ConversationSummaryBufferMemory;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let history = Arc::new(ChatMessageHistory::new(store));
let model: Arc<dyn ChatModel> = /* your chat model */;
let memory = ConversationSummaryBufferMemory::new(history, model, 2000);
// Keeps recent messages verbatim; summarizes when total tokens exceed 2000
```

最适合：在成本和上下文质量之间取得平衡——你能获得近期消息的完整细节和较早消息的压缩摘要。

## 第四步：使用 RunnableWithMessageHistory 自动管理历史

在实际的聊天机器人中，你希望历史的加载和保存在每一轮对话中自动发生。`RunnableWithMessageHistory` 包装任何 `Runnable<Vec<Message>, String>` 并为你处理这一切：

1. 从 `RunnableConfig.metadata["session_id"]` 中提取 `session_id`
2. 从记忆中加载对话历史
3. 追加用户的新消息
4. 用完整的消息列表调用内部 runnable
5. 将 AI 的回复保存回记忆

```rust
use std::sync::Arc;
use std::collections::HashMap;
use synaptic::core::{MemoryStore, RunnableConfig};
use synaptic::memory::ChatMessageHistory;
use synaptic::memory::RunnableWithMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::runnables::Runnable;

// Wrap a model chain with automatic history management
let store = Arc::new(InMemoryStore::new());
let memory = Arc::new(ChatMessageHistory::new(store));
let chain = /* your model chain (BoxRunnable<Vec<Message>, String>) */;
let chatbot = RunnableWithMessageHistory::new(chain, memory);

// Each call automatically loads/saves history
let mut config = RunnableConfig::default();
config.metadata.insert(
    "session_id".to_string(),
    serde_json::Value::String("user-42".to_string()),
);

let response = chatbot.invoke("What is Rust?".to_string(), &config).await?;
// The user message and AI response are now stored in memory for session "user-42"
```

这是生产环境聊天机器人的推荐方式，因为它将记忆管理逻辑从应用代码中分离出来。

## 整体架构

以下是 Synaptic 记忆系统的心智模型：

```text
                    +-----------------------+
                    |    MemoryStore trait   |
                    |  append / load / clear |
                    +-----------+-----------+
                                |
         +----------------------+----------------------+
         |                                             |
  ChatMessageHistory                            Memory Strategies
  (backed by any Store:                        (wrap a MemoryStore)
   InMemoryStore, FileStore)                           |
                                +----------------------+----------------------+
                                |         |         |         |              |
                             Buffer    Window   Summary   TokenBuffer   SummaryBuffer
                             (all)    (last K)   (LLM)    (tokens)       (hybrid)
```

所有记忆策略本身也实现了 `MemoryStore` trait，因此它们是可组合的——你可以将 `ChatMessageHistory` 包装在 `ConversationWindowMemory` 中，下游只看到 `MemoryStore` trait。

## 总结

在本教程中你学会了：

- 使用以 `InMemoryStore` 为后端的 `ChatMessageHistory` 存储和检索对话消息
- 通过 session ID 隔离对话
- 根据对话长度和成本需求选择合适的记忆策略
- 使用 `RunnableWithMessageHistory` 自动管理历史

## 下一步

- [构建 RAG 应用](rag-application.md) -- 为聊天机器人添加文档检索能力
- [记忆操作指南](../how-to/memory/index.md) -- 每种记忆策略的详细指南
- [记忆概念](../concepts/memory.md) -- 深入理解记忆架构
