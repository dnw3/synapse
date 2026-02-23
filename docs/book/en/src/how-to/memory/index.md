# Memory

Synaptic provides session-keyed conversation memory through the `MemoryStore` trait and a family of memory strategies that control how conversation history is stored, trimmed, and summarized.

## The `MemoryStore` Trait

All memory strategies implement the `MemoryStore` trait, which defines three async operations:

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError>;
    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError>;
    async fn clear(&self, session_id: &str) -> Result<(), SynapticError>;
}
```

- **`append`** -- adds a message to the session's history.
- **`load`** -- retrieves the conversation history for a session.
- **`clear`** -- removes all messages for a session.

Every operation is keyed by a `session_id` string, which isolates conversations from one another. You choose the session key (a user ID, a thread ID, a UUID -- whatever makes sense for your application).

## `ChatMessageHistory`

The simplest `MemoryStore` implementation is `ChatMessageHistory`, which wraps any `Store` backend and stores messages per session:

```rust
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use synaptic::core::{MemoryStore, Message};
use std::sync::Arc;

let store = ChatMessageHistory::new(Arc::new(InMemoryStore::new()));

store.append("session-1", Message::human("Hello")).await?;
store.append("session-1", Message::ai("Hi there!")).await?;

let history = store.load("session-1").await?;
assert_eq!(history.len(), 2);

// Different sessions are completely isolated
let other = store.load("session-2").await?;
assert!(other.is_empty());
```

`ChatMessageHistory` is often used as the backing store for the higher-level memory strategies described below.

## Memory Strategies

Each memory strategy wraps an underlying `MemoryStore` and applies a different policy when loading messages. All strategies implement `MemoryStore` themselves, so they are interchangeable wherever a `MemoryStore` is expected.

| Strategy | Behavior | When to Use |
|----------|----------|-------------|
| [Buffer Memory](buffer.md) | Keeps the entire conversation history | Short conversations where full context matters |
| [Window Memory](window.md) | Keeps only the last K messages | Chat UIs where older context is less relevant |
| [Summary Memory](summary.md) | Summarizes older messages with an LLM | Very long conversations requiring compact history |
| [Token Buffer Memory](token-buffer.md) | Keeps recent messages within a token budget | Cost control and prompt size limits |
| [Summary Buffer Memory](summary-buffer.md) | Hybrid -- summarizes old messages, keeps recent ones verbatim | Best balance of context and efficiency |

## Auto-Managing History

For the common pattern of loading history before a chain call and saving the result afterward, Synaptic provides [RunnableWithMessageHistory](runnable-with-history.md). It wraps any `Runnable<Vec<Message>, String>` and handles the load/save lifecycle automatically, keyed by a session ID in the `RunnableConfig` metadata.

## Choosing a Strategy

- If your conversations are short (under 20 messages), **Buffer Memory** is the simplest choice.
- If you want predictable memory usage without an LLM call, use **Window Memory** or **Token Buffer Memory**.
- If conversations are long and you need the full context preserved in compressed form, use **Summary Memory**.
- If you want the best of both worlds -- exact recent messages plus a compressed summary of older history -- use **Summary Buffer Memory**.
