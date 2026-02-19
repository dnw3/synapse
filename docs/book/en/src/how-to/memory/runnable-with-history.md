# RunnableWithMessageHistory

`RunnableWithMessageHistory` wraps any `Runnable<Vec<Message>, String>` to automatically load conversation history before invocation and save the result afterward. This eliminates the boilerplate of manually calling `memory.load()` and `memory.append()` around every chain invocation.

## Usage

```rust
use std::sync::Arc;
use synaptic_memory::{RunnableWithMessageHistory, InMemoryStore};
use synaptic_core::{MemoryStore, Message, RunnableConfig};
use synaptic_runnables::Runnable;

let store = Arc::new(InMemoryStore::new());

// `chain` is any Runnable<Vec<Message>, String>, e.g. a ChatModel pipeline
let with_history = RunnableWithMessageHistory::new(
    chain.boxed(),
    store,
);

// The session_id is passed via config metadata
let mut config = RunnableConfig::default();
config.metadata.insert(
    "session_id".to_string(),
    serde_json::Value::String("user-42".to_string()),
);

// First invocation
let response = with_history.invoke("Hello!".to_string(), &config).await?;
// Internally:
// 1. Loads existing messages for session "user-42" (empty on first call)
// 2. Appends Message::human("Hello!") to the store and to the message list
// 3. Passes the full Vec<Message> to the inner runnable
// 4. Saves Message::ai(response) to the store

// Second invocation -- history is automatically carried forward
let response = with_history.invoke("Tell me more.".to_string(), &config).await?;
// The inner runnable now receives all 4 messages:
// [Human("Hello!"), AI(first_response), Human("Tell me more."), ...]
```

## How It Works

`RunnableWithMessageHistory` implements `Runnable<String, String>`. On each `invoke()` call:

1. **Extract session ID** -- reads `session_id` from `config.metadata`. If not present, defaults to `"default"`.
2. **Load history** -- calls `memory.load(session_id)` to retrieve existing messages.
3. **Append human message** -- creates `Message::human(input)`, appends it to both the in-memory list and the store.
4. **Invoke inner runnable** -- passes the full `Vec<Message>` (history + new message) to the wrapped runnable.
5. **Save AI response** -- creates `Message::ai(output)` and appends it to the store.
6. **Return** -- returns the output string.

## Session Isolation

Different session IDs produce completely isolated conversation histories:

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

// Alice and Bob have independent conversation histories
with_history.invoke("Hi, I'm Alice.".to_string(), &config_a).await?;
with_history.invoke("Hi, I'm Bob.".to_string(), &config_b).await?;
```

## Combining with Memory Strategies

Because `RunnableWithMessageHistory` takes any `Arc<dyn MemoryStore>`, you can pass in a memory strategy to control how history is managed:

```rust
use synaptic_memory::{ConversationWindowMemory, InMemoryStore, RunnableWithMessageHistory};
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let windowed = Arc::new(ConversationWindowMemory::new(store, 10));

let with_history = RunnableWithMessageHistory::new(
    chain.boxed(),
    windowed,  // Only the last 10 messages will be loaded
);
```

This lets you combine automatic history management with any trimming or summarization strategy.

## When to Use

Use `RunnableWithMessageHistory` when:

- You have a `Runnable` chain that takes messages and returns a string (the common pattern for chat pipelines).
- You want to avoid manually loading and saving messages around every invocation.
- You need session-based conversation management with minimal boilerplate.

## Clearing History

Use `MemoryStore::clear()` on the underlying store to reset a session's history:

```rust
let store = Arc::new(InMemoryStore::new());
let with_history = RunnableWithMessageHistory::new(chain.boxed(), store.clone());

// After some conversation...
store.clear("user-42").await?;

// Next invocation starts fresh -- no previous messages are loaded
```

For lower-level control over when messages are loaded and saved, use the `MemoryStore` trait directly.
