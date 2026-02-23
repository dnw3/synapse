# Session, Memory & Store

Synaptic has three crates that deal with "remembering things": `synaptic-store`, `synaptic-memory`, and `synaptic-session`. They operate at different abstraction layers and serve different purposes. The key design principle is that **all three layers share a single `Store` backend** -- one storage engine, many views. This page explains each layer, how they relate, and when to use which.

## Three-Layer Architecture

```text
┌─────────────────────────────────────────────────────┐
│  synaptic-session            (Session lifecycle)     │
│  SessionManager · .memory() · .checkpointer()       │
│  "Which conversation? Can I resume it?"              │
├─────────────────────────────────────────────────────┤
│  synaptic-memory             (Memory strategies)     │
│  ChatMessageHistory · Buffer · Window · Summary      │
│  "How many turns to keep? How to trim?"              │
├─────────────────────────────────────────────────────┤
│  synaptic-store              (Key-value storage)     │
│  InMemoryStore · FileStore                           │
│  "Where is the data? How to read/write?"             │
└─────────────────────────────────────────────────────┘
       ▲ all three layers share a single Store instance
```

Each layer builds on the one below it. The lower you go, the more generic the abstraction. Unlike earlier versions where each layer had its own persistence backend, the new architecture funnels all data through the `Store` trait using namespace conventions.

## Layer 1: Store (Data Persistence)

**Crate:** `synaptic-store`

The store is a general-purpose key-value storage layer. It knows nothing about conversations or AI -- it just stores and retrieves JSON values organized by namespace and key.

```rust
use synaptic::store::{InMemoryStore, FileStore};

// In-memory (development/testing)
let store = InMemoryStore::new();

// File-backed (persistence across restarts)
let store = FileStore::new("/data/myapp");

// Store anything: user profiles, cached results, agent knowledge
store.put(&["users", "alice"], "preferences", item).await?;
let prefs = store.get(&["users", "alice"], "preferences").await?;
```

**Key characteristics:**
- Namespace + key addressing (like a filesystem path)
- Arbitrary JSON values
- CRUD + search + list_namespaces
- Optional semantic search (with embeddings)

**Use when:** You need to persist arbitrary data -- user profiles, cached computations, agent knowledge bases, cross-session state.

### Namespace Design: `&[&str]`

The `Store` trait uses `namespace: &[&str]` -- a multi-level path similar to Python LangChain's `tuple[str, ...]`. It works like a filesystem directory path but uses a borrowed slice of string slices, meaning zero allocation at the call site.

```rust
// Two-level namespace: category + session ID
store.put(&["memory", "session_abc"], "messages", value).await?;

// One-level namespace: flat collection
store.put(&["sessions"], "session_abc", metadata).await?;

// Three-level namespace: deeper hierarchy
store.put(&["agents", "weather-bot", "cache"], "forecast", data).await?;

// List all namespaces under a prefix
let ns = store.list_namespaces(&["memory"]).await?;
// Returns: [["memory", "session_abc"], ["memory", "session_xyz"], ...]
```

This design gives each subsystem its own namespace prefix while sharing a single store instance. The convention used by Synaptic's built-in types:

| Namespace | Key | Data |
|-----------|-----|------|
| `["memory", "{session_id}"]` | `"messages"` | Conversation messages (JSON array) |
| `["memory", "{session_id}"]` | `"summary"` | Summary text (for summary strategies) |
| `["checkpoints", "{thread_id}"]` | `"{checkpoint_id}"` | Graph checkpoint snapshot |
| `["sessions"]` | `"{session_id}"` | Session metadata (id, created_at) |

Because all data goes through the `Store` trait, swapping `InMemoryStore` for `FileStore` (or a future `RedisStore`) changes the persistence backend for memory, checkpoints, and sessions in one line.

## Layer 2: Memory (Conversation Context Management)

**Crate:** `synaptic-memory`

Memory is specialized for conversation history. `ChatMessageHistory` implements the `MemoryStore` trait backed by any `Store`. Messages are serialized as full serde JSON, preserving `tool_calls`, `tool_call_id`, and all message metadata.

```rust
use synaptic::memory::ChatMessageHistory;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let history = ChatMessageHistory::new(store);

// Append messages as conversation progresses
history.append("session_1", Message::human("Hello")).await?;

// Load full history
let messages = history.load("session_1").await?;
```

Memory **strategies** wrap `ChatMessageHistory` and control what gets sent to the LLM:

```rust
use synaptic::memory::{ChatMessageHistory, ConversationWindowMemory};

let history = ChatMessageHistory::new(store.clone());
let memory = ConversationWindowMemory::new(history, 10); // keep last 10 turns
let context = memory.load("session_1").await?;
```

**Key characteristics:**
- Session-scoped message storage with full-fidelity JSON serialization
- Strategies control what gets sent to the LLM (Buffer, Window, Summary, TokenBuffer, SummaryBuffer)
- Backed by any `Store` via `ChatMessageHistory`

**Use when:** You need to manage conversation context for an LLM -- deciding how many messages to keep, whether to summarize old messages, or how to fit within a token budget.

### Memory vs. Store

| Aspect | Store | Memory |
|--------|-------|--------|
| **Purpose** | General key-value storage | Conversation context management |
| **Keyed by** | Namespace + key | Session ID |
| **Value type** | Arbitrary JSON (`Value`) | `Message` |
| **Operations** | CRUD + search + list | Append + load + clear |
| **Strategies** | None (raw storage) | Buffer, Window, Summary, TokenBuffer, SummaryBuffer |
| **Use case** | Agent knowledge, user profiles | Chat history, LLM context |

## Layer 3: Session (Conversation Lifecycle)

**Crate:** `synaptic-session`

Session manages the lifecycle of entire conversations -- creating, listing, resuming, and deleting sessions. It acts as a coordinator that hands out `ChatMessageHistory` and `StoreCheckpointer` instances all backed by the same store.

```rust
use synaptic::session::SessionManager;
use synaptic::store::FileStore;
use std::sync::Arc;

let store = Arc::new(FileStore::new("/data/myapp"));
let manager = SessionManager::new(store);

// Create a new session (returns session ID)
let session_id = manager.create_session().await?;

// Get memory and checkpointer — both use the same store
let memory = manager.memory();
let checkpointer = manager.checkpointer();

// Append messages via memory
memory.append(&session_id, Message::human("Hello")).await?;
memory.append(&session_id, Message::ai("Hi there!")).await?;

// Later: list and resume
let sessions = manager.list_sessions().await?;
let history = memory.load(&session_id).await?;

// Delete session and all associated data
manager.delete_session(&session_id).await?;
```

**Key characteristics:**
- Session CRUD (create/list/get/delete)
- `.memory()` returns a `ChatMessageHistory` sharing the same store
- `.checkpointer()` returns a `StoreCheckpointer` sharing the same store
- `delete_session()` cleans up messages, summaries, and checkpoints in one call
- Generates unique session IDs (UUID v4)

**Use when:** You are building a CLI, chatbot, or multi-session application where users need to resume previous conversations.

## How They Work Together

In the unified architecture, all three layers share a single `Arc<dyn Store>`:

```text
                  Arc<dyn Store>
                  (one instance)
                  ┌─────────┐
                  │ FileStore│
                  └────┬────┘
           ┌──────────┼──────────┐
           ▼          ▼          ▼
    ChatMessageHistory  StoreCheckpointer  SessionManager
    ["memory", sid]     ["checkpoints", t] ["sessions"]
```

```text
User starts conversation
    │
    ▼
SessionManager.create_session()              ← Session layer: lifecycle
    │
    ▼
manager.memory().load(session_id)            ← Memory layer: context strategy
    │
    ▼
Store.get(&["memory", sid], "messages")      ← Store layer: persistence
    │
    ▼
LLM.chat(context_messages)                  ← AI model
    │
    ▼
manager.memory().append(session_id, msg)     ← Memory layer: save new message
    (store.put automatically called)         ← Store layer: persists it
```

### Example: Building a Persistent Chat Agent

```rust,ignore
use synaptic::session::SessionManager;
use synaptic::memory::ConversationWindowMemory;
use synaptic::store::FileStore;
use std::sync::Arc;

// One store for everything
let store = Arc::new(FileStore::new("/data/myapp"));

// Session manager coordinates lifecycle
let sessions = SessionManager::new(store.clone());
let session_id = sessions.create_session().await?;

// Memory strategy wraps the store-backed history
let memory = ConversationWindowMemory::new(sessions.memory(), 20);

// Chat loop
loop {
    let user_input = read_input();

    // Load context using memory strategy
    let mut context = memory.load(&session_id).await?;
    context.push(Message::human(&user_input));

    // Call LLM
    let response = model.chat(ChatRequest::new(context)).await?;

    // Save to memory — strategy decides what to keep,
    // store persists it as full-fidelity JSON
    memory.append(&session_id, Message::human(&user_input)).await?;
    memory.append(&session_id, response.message.clone()).await?;
}
```

Because memory and session share the same store, there is no data duplication. The memory strategy controls what the LLM sees, while the store preserves the complete message history as full-fidelity JSON (including `tool_calls`, `tool_call_id`, and all metadata).

## When to Use What

| Scenario | Use |
|----------|-----|
| Store user preferences across sessions | **Store** (`FileStore`) |
| Keep last 10 messages for LLM context | **Memory** (`ConversationWindowMemory`) |
| Resume a conversation after restart | **Session** (`SessionManager`) |
| Cache tool execution results | **Store** (`InMemoryStore`) |
| Summarize old messages to save tokens | **Memory** (`ConversationSummaryMemory`) |
| List all past conversations | **Session** (`SessionManager::list_sessions`) |
| Store embeddings for semantic search | **Store** (with embeddings) |
| Persist graph checkpoints per session | **Graph** (`StoreCheckpointer`) |

## Related Concepts

- **Condenser** (`synaptic-condenser`) -- operates at the memory layer, providing additional context compression strategies (rolling, token budget, LLM summarization, pipeline). Think of condensers as "memory strategies on steroids" that can be composed via middleware.

- **Graph Checkpointing** (`synaptic-graph::StoreCheckpointer`) -- persists graph execution state (which node was last executed, the full state snapshot) into the shared store under namespace `["checkpoints", "{thread_id}"]`.

## See Also

- [Memory](memory.md) -- memory strategies in detail
- [Key-Value Store](store.md) -- store trait and operations
- [Session Management](../how-to/session.md) -- how-to guide
- [File Persistence](../how-to/persistence.md) -- FileStore usage
- [Context Condensation](../how-to/condenser.md) -- condenser strategies
