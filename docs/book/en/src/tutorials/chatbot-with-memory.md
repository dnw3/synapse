# Build a Chatbot with Memory

This tutorial walks you through building a session-based chatbot that remembers conversation history. You will learn how to store and retrieve messages with `ChatMessageHistory`, isolate conversations by session ID, and choose the right memory strategy for your use case.

## Prerequisites

Add the required Synaptic crates to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["memory", "store"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Step 1: Store and Load Messages

Every chatbot needs to remember what was said. Synaptic provides the `MemoryStore` trait for this purpose, and `ChatMessageHistory` as the standard implementation backed by any `Store`. Here we use `InMemoryStore` as the underlying storage:

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

The output will be:

```text
human: Hello, Synaptic
ai: Hello! How can I help you?
human: What can you do?
ai: I can help with many tasks!
```

The `MemoryStore` trait defines three methods:

- **`append(session_id, message)`** -- adds a message to a session's history.
- **`load(session_id)`** -- returns all messages for a session as a `Vec<Message>`.
- **`clear(session_id)`** -- removes all messages for a session.

## Step 2: Session Isolation

Each session ID maps to an independent conversation history. This is how you keep multiple users or threads separate:

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

Session IDs are arbitrary strings. In a web application you would typically use a user ID, a conversation thread ID, or a combination of both.

## Step 3: Choose a Memory Strategy

As conversations grow long, sending every message to the LLM becomes expensive and eventually exceeds the context window. Synaptic provides several memory strategies that wrap an underlying `MemoryStore` and control what gets returned by `load()`.

### ConversationBufferMemory

Keeps all messages. This is the simplest strategy -- a passthrough wrapper that makes the "keep everything" policy explicit:

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

Best for: short conversations where you want the full history available.

### ConversationWindowMemory

Keeps only the last **K** messages. Older messages are still stored but are not returned by `load()`:

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

Best for: conversations where recent context is sufficient and you want predictable costs.

### ConversationSummaryMemory

Uses an LLM to summarize older messages. When the stored message count exceeds `buffer_size * 2`, the older portion is compressed into a summary that is prepended as a system message:

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

Best for: long-running conversations where you need to retain the gist of older context without the full verbatim history.

### ConversationTokenBufferMemory

Keeps messages within a **token budget**. Uses a configurable token estimator to drop the oldest messages once the total exceeds the limit:

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

Best for: staying within a model's context window by directly managing token count.

### ConversationSummaryBufferMemory

A hybrid of summary and buffer strategies. Keeps the most recent messages verbatim, and summarizes everything older when the token count exceeds a threshold:

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

Best for: balancing cost with context quality -- you get the detail of recent messages and the compressed gist of older ones.

## Step 4: Auto-Manage History with RunnableWithMessageHistory

In a real chatbot, you want the history load/save to happen automatically on each turn. `RunnableWithMessageHistory` wraps any `Runnable<Vec<Message>, String>` and handles this for you:

1. Extracts the `session_id` from `RunnableConfig.metadata["session_id"]`
2. Loads conversation history from memory
3. Appends the user's new message
4. Calls the inner runnable with the full message list
5. Saves the AI response back to memory

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

This is the recommended approach for production chatbots because it keeps the memory management out of your application logic.

## How It All Fits Together

Here is the mental model for Synaptic memory:

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

All memory strategies implement `MemoryStore` themselves, so they are composable -- you could wrap a `ChatMessageHistory` in a `ConversationWindowMemory`, and everything downstream only sees the `MemoryStore` trait.

## Summary

In this tutorial you learned how to:

- Use `ChatMessageHistory` backed by `InMemoryStore` to store and retrieve conversation messages
- Isolate conversations with session IDs
- Choose a memory strategy based on your conversation length and cost requirements
- Automate history management with `RunnableWithMessageHistory`

## Next Steps

- [Build a RAG Application](rag-application.md) -- add document retrieval to your chatbot
- [Memory How-to Guides](../how-to/memory/index.md) -- detailed guides for each memory strategy
- [Memory Concepts](../concepts/memory.md) -- deeper understanding of memory architecture
