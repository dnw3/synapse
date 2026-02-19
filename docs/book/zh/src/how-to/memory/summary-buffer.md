# 摘要缓冲记忆

`ConversationSummaryBufferMemory` 是一种混合策略，结合了[摘要记忆](summary.md)和[令牌缓冲记忆](token-buffer.md)的优势。近期消息保持原文，而当估算的总令牌数超过可配置的阈值时，较早的消息会被压缩为 LLM 生成的滚动摘要。

## 用法

```rust
use std::sync::Arc;
use synaptic_memory::{ConversationSummaryBufferMemory, InMemoryStore};
use synaptic_core::{MemoryStore, Message, ChatModel};

let model: Arc<dyn ChatModel> = Arc::new(my_model);
let store = Arc::new(InMemoryStore::new());

// 当总令牌数超过 500 时，对较早的消息进行摘要
let memory = ConversationSummaryBufferMemory::new(store, model, 500);

let session = "user-1";

memory.append(session, Message::human("What is Rust?")).await?;
memory.append(session, Message::ai("Rust is a systems programming language...")).await?;
memory.append(session, Message::human("How does ownership work?")).await?;
memory.append(session, Message::ai("Ownership is a set of rules...")).await?;
// ... 随着对话增长并超过 500 估算令牌，
// 较早的消息会被自动摘要 ...

let history = memory.load(session).await?;
// history = [System("Summary of earlier conversation: ..."), 近期消息...]
```

## 工作原理

1. **`append()`** 存储新消息，然后估算所有已存储消息的总令牌数。
2. 当总数超过 `max_token_limit` 且消息数量超过一条时：
   - 计算分割点：在令牌限制一半以内的近期消息保持原文。
   - 分割点之前的所有消息由 `ChatModel` 进行摘要。如果已有先前的摘要，则将其作为上下文。
   - 清空 store 并仅重新填充近期消息。
3. **`load()`** 返回已存储的消息，如果存在摘要，则在前面插入一条包含摘要的系统消息：

   ```
   Summary of earlier conversation: <摘要文本>
   ```

4. **`clear()`** 移除该会话的所有已存储消息和摘要。

## 参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `store` | `Arc<dyn MemoryStore>` | 存储原始消息的后端 store |
| `model` | `Arc<dyn ChatModel>` | 用于生成摘要的 LLM |
| `max_token_limit` | `usize` | 触发摘要的令牌阈值 |

## 令牌估算

与 `ConversationTokenBufferMemory` 相同，该策略以大约每 4 个字符对应 1 个令牌来估算（最小值为 1）。同样需要注意：实际令牌数因模型而异。

## 何时使用

摘要缓冲记忆是以下场景的推荐策略：

- 对话较长，你既需要精确的近期上下文，又需要压缩的历史上下文。
- 你希望在令牌预算内尽可能保留更多信息。
- 偶尔的 LLM 摘要调用所带来的额外成本可以接受。

这是与 LangChain 的 `ConversationSummaryBufferMemory` 最接近的等价实现，通常是生产环境聊天机器人的最佳默认选择。

## 权衡

- **溢出时的 LLM 成本** -- 摘要仅在令牌限制被超过时触发，但每次摘要调用都会增加延迟和成本。
- **旧消息有损** -- 较早消息的细节可能在摘要中丢失，但近期消息始终保持原文。
- **启发式令牌计数** -- 分割点基于估算的令牌数，而非精确计数。

## 使用 ScriptedChatModel 进行离线测试

使用 `ScriptedChatModel` 可以在没有 API 密钥的情况下测试摘要功能：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, MemoryStore, Message};
use synaptic_models::ScriptedChatModel;
use synaptic_memory::{ConversationSummaryBufferMemory, InMemoryStore};

// 预设模型在被调用时返回摘要
let summarizer = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("The user asked about Rust and ownership."),
        usage: None,
    },
]));

let store = Arc::new(InMemoryStore::new());
let memory = ConversationSummaryBufferMemory::new(store, summarizer, 50);

let session = "test";

// 添加足够多的消息以超过 50 令牌的阈值
memory.append(session, Message::human("What is Rust?")).await?;
memory.append(session, Message::ai("Rust is a systems programming language focused on safety, speed, and concurrency.")).await?;
memory.append(session, Message::human("How does ownership work?")).await?;
memory.append(session, Message::ai("Ownership is a set of rules the compiler checks at compile time. Each value has a single owner.")).await?;

// 加载 -- 较早的消息现在已被摘要
let history = memory.load(session).await?;
// history[0] 是包含摘要的 System 消息
// 其余消息是保持原文的最近消息
```

如需更简单的替代方案，请参阅 [缓冲记忆](buffer.md)（保留所有消息）、[窗口记忆](window.md)（固定消息数量）或 [令牌缓冲记忆](token-buffer.md)（无摘要的令牌预算）。
