# Checkpointing

Checkpointing persists graph state between invocations, enabling resumable execution, multi-turn conversations over a graph, and human-in-the-loop workflows. The `Checkpointer` trait abstracts the storage backend, and `StoreCheckpointer` provides an implementation backed by any `Store` for development and testing.

## The `Checkpointer` Trait

```rust
#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn put(&self, config: &CheckpointConfig, checkpoint: &Checkpoint) -> Result<(), SynapticError>;
    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError>;
    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError>;
}
```

A `Checkpoint` stores the serialized state and the name of the next node to execute:

```rust
pub struct Checkpoint {
    pub state: serde_json::Value,
    pub next_node: Option<String>,
}
```

## `StoreCheckpointer`

`StoreCheckpointer` with an `InMemoryStore` backend is the simplest in-memory checkpointer. It stores checkpoints in any `Store` backend, organized by namespace `["checkpoints", thread_id]`:

```rust
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let checkpointer = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
```

For production use, you can swap in any `Store` implementation (Redis, database, file system, etc.) or implement the `Checkpointer` trait directly.

## Attaching a Checkpointer

After compiling a graph, attach a checkpointer with `.with_checkpointer()`:

```rust
use synaptic::graph::{StateGraph, FnNode, MessageState, StoreCheckpointer, END};
use synaptic::store::InMemoryStore;
use synaptic::core::Message;
use std::sync::Arc;

let node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Processed"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("process", node)
    .set_entry_point("process")
    .add_edge("process", END)
    .compile()?
    .with_checkpointer(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));
```

## `CheckpointConfig`

A `CheckpointConfig` identifies a thread (conversation) for checkpointing:

```rust
use synaptic::graph::CheckpointConfig;

let config = CheckpointConfig::new("thread-1");
```

The `thread_id` string isolates different conversations. Each thread maintains its own checkpoint history.

## Invoking with Checkpoints

Use `invoke_with_config()` to run the graph with checkpointing enabled:

```rust
let config = CheckpointConfig::new("thread-1");
let initial = MessageState::with_messages(vec![Message::human("Hello")]);

let result = graph.invoke_with_config(initial, Some(config.clone())).await?;
```

After each node executes, the current state and next node are saved to the checkpointer. On subsequent invocations with the same `CheckpointConfig`, the graph resumes from the last checkpoint.

## Retrieving State

You can inspect the current state saved for a thread:

```rust
// Get the latest state for a thread
if let Some(state) = graph.get_state(&config).await? {
    println!("Messages: {}", state.messages.len());
}

// Get the full checkpoint history (oldest to newest)
let history = graph.get_state_history(&config).await?;
for (state, next_node) in &history {
    println!(
        "State with {} messages, next node: {:?}",
        state.messages.len(),
        next_node
    );
}
```

## State Serialization

Checkpointing requires your state type to implement `Serialize` and `Deserialize` (from `serde`). The built-in `MessageState` already has these derives. For custom state types, add the derives:

```rust
use serde::{Serialize, Deserialize};
use synaptic::graph::State;

#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    data: Vec<String>,
}

impl State for MyState {
    fn merge(&mut self, other: Self) {
        self.data.extend(other.data);
    }
}
```
