# File Persistence

Synaptic provides file-system backed persistence through `FileStore` (the `Store` trait) and `StoreCheckpointer` (the `Checkpointer` trait backed by any `Store`).

## FileStore

`FileStore` implements the `Store` trait with a directory-based layout. Each item is stored as a JSON file.

**Feature flag**: `store-filesystem`

### Layout

```
{root}/{namespace...}/{key}.json
```

For example, `store.put(&["users", "prefs"], "theme", json!("dark"))` writes to `{root}/users/prefs/theme.json`.

### Basic Usage

```rust,ignore
use synaptic::store::FileStore;

let store = FileStore::new("/tmp/my-store");

// Write
store.put(&["app", "settings"], "theme", json!("dark")).await?;

// Read
let item = store.get(&["app", "settings"], "theme").await?;

// Search (substring matching on key and value)
let results = store.search(&["app"], Some("theme"), 10).await?;

// Delete
store.delete(&["app", "settings"], "theme").await?;

// List namespaces
let namespaces = store.list_namespaces(&["app"]).await?;
```

### With Embeddings

`FileStore` supports optional embeddings for semantic search, just like `InMemoryStore`.

```rust,ignore
use synaptic::store::FileStore;
use synaptic::openai::OpenAiEmbeddings;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = FileStore::new("/tmp/my-store").with_embeddings(embeddings);
```

## StoreCheckpointer

`StoreCheckpointer` implements the `Checkpointer` trait backed by any `Store`. This replaces the old `FileSaver` with a unified, backend-agnostic approach -- the same checkpointer works with `InMemoryStore`, `FileStore`, `RedisStore`, or any other `Store` implementation.

**Feature flag**: `store-filesystem` (when using `FileStore` as the backing store)

### How It Works

Checkpoints are stored at namespace `["checkpoints", "{thread_id}"]`, with the checkpoint ID as the key. Checkpoint IDs are timestamp-hex based, so alphabetical order corresponds to chronological order.

When backed by `FileStore`, the on-disk layout is:

```
{root}/checkpoints/{thread_id}/{checkpoint_id}.json
```

### Usage

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{StoreCheckpointer, CheckpointConfig};
use synaptic::store::FileStore;

let store = Arc::new(FileStore::new("/tmp/my-data"));
let checkpointer = StoreCheckpointer::new(store);

// Use with a compiled graph
let graph = builder.compile_with_checkpointer(Arc::new(checkpointer))?;

let config = CheckpointConfig::new("thread-1");
let result = graph.invoke_with_config(state, config).await?;
```

### Manual Checkpoint Operations

```rust,ignore
use synaptic::graph::{Checkpointer, CheckpointConfig};

let config = CheckpointConfig::new("thread-1");

// Get the latest checkpoint
let latest = checkpointer.get(&config).await?;

// List all checkpoints for a thread
let all = checkpointer.list(&config).await?;
```

### Unified Namespace

Because `StoreCheckpointer` is backed by a regular `Store`, the same `FileStore` instance can handle memory, checkpoints, and sessions simultaneously. Each subsystem uses a distinct namespace prefix:

```rust,ignore
use std::sync::Arc;
use synaptic::store::FileStore;
use synaptic::graph::StoreCheckpointer;

let store = Arc::new(FileStore::new("/tmp/my-data"));

// Checkpoints go to {root}/checkpoints/{thread_id}/{id}.json
let checkpointer = StoreCheckpointer::new(store.clone());

// Application data goes to {root}/app/settings/{key}.json
store.put(&["app", "settings"], "theme", json!("dark")).await?;
```

This single-store approach eliminates the need for separate directory configurations and keeps all persistent state in one place.

## Cargo.toml

```toml
[dependencies]
synaptic = { version = "0.3", features = ["store-filesystem", "graph"] }
```

## Migrating from FileSaver

If you previously used `FileSaver` from the `graph-filesystem` feature, switch to `StoreCheckpointer` backed by `FileStore`:

| Before | After |
|--------|-------|
| `use synaptic::graph::FileSaver` | `use synaptic::graph::StoreCheckpointer` |
| `FileSaver::new("/tmp/checkpoints")` | `StoreCheckpointer::new(Arc::new(FileStore::new("/tmp/data")))` |
| Feature: `graph-filesystem` | Feature: `store-filesystem` + `graph` |

The checkpoint data format is the same, so existing checkpoint files remain compatible.

`FileStore` and `StoreCheckpointer` are suitable for single-process deployments. For distributed systems, use database-backed `Store` implementations such as `RedisStore`.
