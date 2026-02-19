# RunnableWithMessageHistory

`RunnableWithMessageHistory` 包装任意 `Runnable<Vec<Message>, String>`，在每次调用前自动加载对话历史，并在调用后自动保存结果。这消除了在每次链式调用前后手动调用 `memory.load()` 和 `memory.append()` 的模板代码。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{RunnableWithMessageHistory, InMemoryStore};
use synaptic_core::{MemoryStore, Message, RunnableConfig};
use synaptic_runnables::Runnable;

let store = Arc::new(InMemoryStore::new());

// `chain` 是任意 Runnable<Vec<Message>, String>，例如 ChatModel 管道
let with_history = RunnableWithMessageHistory::new(
    chain.boxed(),
    store,
);

// session_id 通过 config 的 metadata 传递
let mut config = RunnableConfig::default();
config.metadata.insert(
    "session_id".to_string(),
    serde_json::Value::String("user-42".to_string()),
);

// 第一次调用
let response = with_history.invoke("Hello!".to_string(), &config).await?;
// 内部流程：
// 1. 加载 session "user-42" 的已有消息（首次调用时为空）
// 2. 将 Message::human("Hello!") 追加到 store 和消息列表中
// 3. 将完整的 Vec<Message> 传递给内部 runnable
// 4. 将 Message::ai(response) 保存到 store

// 第二次调用 -- 历史记录自动延续
let response = with_history.invoke("Tell me more.".to_string(), &config).await?;
// 内部 runnable 现在接收到全部 4 条消息：
// [Human("Hello!"), AI(first_response), Human("Tell me more."), ...]
```

## 工作原理

`RunnableWithMessageHistory` 实现了 `Runnable<String, String>`。每次 `invoke()` 调用时：

1. **提取 session ID** -- 从 `config.metadata` 中读取 `session_id`。如果不存在，默认使用 `"default"`。
2. **加载历史** -- 调用 `memory.load(session_id)` 获取已有消息。
3. **追加用户消息** -- 创建 `Message::human(input)`，同时追加到内存列表和 store 中。
4. **调用内部 runnable** -- 将完整的 `Vec<Message>`（历史 + 新消息）传递给被包装的 runnable。
5. **保存 AI 回复** -- 创建 `Message::ai(output)` 并追加到 store 中。
6. **返回** -- 返回输出字符串。

## 会话隔离

不同的 session ID 产生完全隔离的对话历史：

```rust
let mut config_a = RunnableConfig::default();
config_a.metadata.insert(
    "session_id".to_string(),
    serde_json::Value::String("alice".to_string()),
);

let mut config_b = RunnableConfig::default();
config_b.metadata.insert(
    "session_id".to_string(),
    serde_json::Value::String("bob".to_string()),
);

// Alice 和 Bob 拥有独立的对话历史
with_history.invoke("Hi, I'm Alice.".to_string(), &config_a).await?;
with_history.invoke("Hi, I'm Bob.".to_string(), &config_b).await?;
```

## 与记忆策略结合使用

由于 `RunnableWithMessageHistory` 接受任意 `Arc<dyn MemoryStore>`，你可以传入记忆策略来控制历史的管理方式：

```rust
use synaptic_memory::{ConversationWindowMemory, InMemoryStore, RunnableWithMessageHistory};
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let windowed = Arc::new(ConversationWindowMemory::new(store, 10));

let with_history = RunnableWithMessageHistory::new(
    chain.boxed(),
    windowed,  // 只加载最近 10 条消息
);
```

这样你可以将自动的历史管理与任意裁剪或摘要策略结合使用。

## 何时使用

在以下场景中使用 `RunnableWithMessageHistory`：

- 你有一个接收消息并返回字符串的 `Runnable` 链（聊天管道的常见模式）。
- 你希望避免在每次调用前后手动加载和保存消息。
- 你需要基于会话的对话管理，同时尽量减少模板代码。

## 清除历史

使用底层 store 的 `MemoryStore::clear()` 方法重置某个会话的历史：

```rust
let store = Arc::new(InMemoryStore::new());
let with_history = RunnableWithMessageHistory::new(chain.boxed(), store.clone());

// 经过一些对话之后...
store.clear("user-42").await?;

// 下一次调用将从头开始 -- 不会加载之前的消息
```

如果需要更底层地控制消息的加载和保存时机，请直接使用 `MemoryStore` trait。
