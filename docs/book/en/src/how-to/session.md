# Session Management

The `synaptic::session` module provides Store-backed session lifecycle management. All session data -- metadata, messages, and graph checkpoints -- lives in a single `Store`, making it easy to swap backends (in-memory, filesystem, or any custom implementation).

## Setup

Add the `session` feature (which pulls in `graph`, `memory`, and `store`):

```toml
[dependencies]
synaptic = { version = "0.3", features = ["session"] }
```

For filesystem persistence, also enable `store-filesystem`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["session", "store-filesystem"] }
```

## SessionManager

`SessionManager` is the central entry point. Construct it with any `Arc<dyn Store>`:

```rust,ignore
use std::sync::Arc;
use synaptic::session::SessionManager;
use synaptic::store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let manager = SessionManager::new(store);
```

### Creating a Session

Each session is assigned a unique UUID. The metadata is persisted in the store under the `["sessions"]` namespace.

```rust,ignore
let session_id = manager.create_session().await?;
println!("Session ID: {session_id}");
```

### Listing Sessions

Returns all sessions sorted by creation time.

```rust,ignore
let sessions = manager.list_sessions().await?;
for info in &sessions {
    println!("{} (created: {})", info.id, info.created_at);
}
```

### Getting a Session

Retrieve metadata for a single session by ID:

```rust,ignore
if let Some(info) = manager.get_session(&session_id).await? {
    println!("Found session: {}", info.id);
}
```

### Deleting a Session

`delete_session` removes **all** data associated with the session: metadata, messages, summaries, and checkpoints.

```rust,ignore
manager.delete_session(&session_id).await?;
```

## SessionInfo

`SessionInfo` is the metadata struct stored for each session:

| Field        | Type     | Description                      |
|------------- |--------- |--------------------------------- |
| `id`         | `String` | Unique session identifier (UUID) |
| `created_at` | `String` | ISO timestamp of creation        |

## Shared Store Access

The key design principle is that `SessionManager`, `ChatMessageHistory`, and `StoreCheckpointer` all share the **same** underlying store. This means a single store handles everything for a session.

### Memory Interface

Call `.memory()` to get a `ChatMessageHistory` backed by the same store. Use it to append and load messages for any session:

```rust,ignore
use synaptic::core::Message;

let memory = manager.memory();

// Append messages
memory.append(&session_id, Message::human("Hello")).await?;
memory.append(&session_id, Message::ai("Hi there!")).await?;

// Load conversation history
let messages = memory.load(&session_id).await?;
assert_eq!(messages.len(), 2);
```

### Checkpointer Interface

Call `.checkpointer()` to get a `StoreCheckpointer` for use with `CompiledGraph`:

```rust,ignore
use std::sync::Arc;

let checkpointer = manager.checkpointer();

// Pass to graph compilation
let graph = builder.compile_with_checkpointer(Arc::new(checkpointer))?;
```

### Underlying Store

Access the raw store reference when needed:

```rust,ignore
let store = manager.store();
```

## Full Example

```rust,ignore
use std::sync::Arc;
use synaptic::core::{Message, Store};
use synaptic::session::SessionManager;
use synaptic::store::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a store-backed session manager
    let store = Arc::new(InMemoryStore::new());
    let manager = SessionManager::new(store);

    // Create a session
    let session_id = manager.create_session().await?;

    // Use the memory interface to store messages
    let memory = manager.memory();
    memory.append(&session_id, Message::human("What is Rust?")).await?;
    memory.append(&session_id, Message::ai("Rust is a systems programming language.")).await?;

    // Load messages back
    let messages = memory.load(&session_id).await?;
    println!("Messages: {}", messages.len());

    // Use checkpointer for graph state persistence
    let _checkpointer = manager.checkpointer();

    // List all sessions
    let sessions = manager.list_sessions().await?;
    println!("Total sessions: {}", sessions.len());

    // Clean up -- deletes metadata, messages, and checkpoints
    manager.delete_session(&session_id).await?;

    Ok(())
}
```

## Using FileStore for Persistence

For durable sessions that survive process restarts, use `FileStore` instead of `InMemoryStore`:

```rust,ignore
use std::sync::Arc;
use synaptic::session::SessionManager;
use synaptic::store::FileStore;

let store = Arc::new(FileStore::new(".sessions").await?);
let manager = SessionManager::new(store);

// Everything works the same -- data is persisted to disk
let session_id = manager.create_session().await?;
let memory = manager.memory();
memory.append(&session_id, Message::human("Hello")).await?;
```

## Data Layout

All session data is organized by store namespaces:

| Namespace               | Key          | Content              |
|------------------------ |------------- |--------------------- |
| `["sessions"]`          | `session_id` | `SessionInfo` JSON   |
| `["memory", session_id]`| `"messages"` | Message history      |
| `["memory", session_id]`| `"summary"`  | Conversation summary |
| `["checkpoints", session_id]` | checkpoint keys | Graph state   |

When `delete_session` is called, all entries across these namespaces are removed for the given session ID.
