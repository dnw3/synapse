# Graph Checkpointers

By default, `synaptic-graph` uses [`MemorySaver`], which stores graph state only in-process memory. This means state is lost when the process restarts — not suitable for production.

Synaptic provides four persistent checkpointer backends:

| Backend | Crate | Best For |
|---------|-------|----------|
| Redis | `synaptic-redis` | Low-latency, optional TTL expiry |
| PostgreSQL | `synaptic-pgvector` | Relational workloads, ACID guarantees |
| SQLite | `synaptic-sqlite` | Single-machine, no external service |
| MongoDB | `synaptic-mongodb` | Distributed, document-oriented |

## Setup

Add the relevant crate to `Cargo.toml`:

```toml
# Redis checkpointer
[dependencies]
synaptic = { version = "0.2", features = ["agent", "redis"] }
synaptic-redis = { version = "0.2" }

# PostgreSQL checkpointer
synaptic = { version = "0.2", features = ["agent", "pgvector"] }
synaptic-pgvector = { version = "0.2" }

# SQLite checkpointer (no external service required)
synaptic = { version = "0.2", features = ["agent", "sqlite"] }
synaptic-sqlite = { version = "0.2" }

# MongoDB checkpointer
synaptic = { version = "0.2", features = ["agent", "mongodb"] }
synaptic-mongodb = { version = "0.2" }
```

## Redis Checkpointer

### Quick start

```rust,ignore
use synaptic_redis::{RedisCheckpointer, RedisCheckpointerConfig};
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// Connect to Redis
let checkpointer = RedisCheckpointer::from_url("redis://127.0.0.1/").await?;

// Build the graph with the persistent checkpointer
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

// Run with a thread ID for persistence
let state = MessageState { messages: vec![Message::human("Hello")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### Configuration

```rust,ignore
use synaptic_redis::RedisCheckpointerConfig;

let config = RedisCheckpointerConfig::new("redis://127.0.0.1/")
    .with_ttl(86400)          // Expire checkpoints after 24 hours
    .with_prefix("myapp");    // Custom key prefix (default: "synaptic")

let checkpointer = RedisCheckpointer::new(config).await?;
```

### Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | `String` | required | Redis connection URL |
| `ttl` | `Option<u64>` | `None` | TTL in seconds for checkpoint keys |
| `prefix` | `String` | `"synaptic"` | Key prefix for all checkpoint keys |

### Key scheme

Redis stores checkpoints using the following keys:

- **Checkpoint data**: `{prefix}:checkpoint:{thread_id}:{checkpoint_id}` — JSON-serialized `Checkpoint`
- **Thread index**: `{prefix}:idx:{thread_id}` — Redis LIST of checkpoint IDs in chronological order

## PostgreSQL Checkpointer

### Quick start

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic_pgvector::PgCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// Create a connection pool
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

// Create and initialize the checkpointer (creates table if not exists)
let checkpointer = PgCheckpointer::new(pool);
checkpointer.initialize().await?;

// Build the graph
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));
```

### Schema

`initialize()` creates the following table if it does not exist:

```sql
CREATE TABLE IF NOT EXISTS synaptic_checkpoints (
    thread_id     TEXT        NOT NULL,
    checkpoint_id TEXT        NOT NULL,
    state         JSONB       NOT NULL,
    next_node     TEXT,
    parent_id     TEXT,
    metadata      JSONB       NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (thread_id, checkpoint_id)
);
```

### Custom table name

```rust,ignore
let checkpointer = PgCheckpointer::new(pool)
    .with_table("my_custom_checkpoints");
checkpointer.initialize().await?;
```

## SQLite Checkpointer

The `SqliteCheckpointer` stores checkpoints in a local SQLite database. It requires no external service and is ideal for single-machine deployments, CLI tools, and development.

### Quick start

```rust,ignore
use synaptic_sqlite::SqliteCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// File-based (persists across restarts)
let checkpointer = SqliteCheckpointer::new("/var/lib/myapp/checkpoints.db")?;

// Build the graph
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

let state = MessageState { messages: vec![Message::human("Hello")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### In-memory mode (for testing)

```rust,ignore
use synaptic_sqlite::SqliteCheckpointer;

let checkpointer = SqliteCheckpointer::in_memory()?;
```

### Schema

`SqliteCheckpointer::new()` automatically creates two tables:

```sql
-- Checkpoint state storage
CREATE TABLE IF NOT EXISTS synaptic_checkpoints (
    thread_id     TEXT    NOT NULL,
    checkpoint_id TEXT    NOT NULL,
    state         TEXT    NOT NULL,  -- JSON-serialized Checkpoint
    created_at    INTEGER NOT NULL,  -- Unix timestamp
    PRIMARY KEY (thread_id, checkpoint_id)
);

-- Ordered index for latest/list queries
CREATE TABLE IF NOT EXISTS synaptic_checkpoint_idx (
    thread_id     TEXT    NOT NULL,
    checkpoint_id TEXT    NOT NULL,
    seq           INTEGER NOT NULL,  -- Monotonically increasing per thread
    PRIMARY KEY (thread_id, checkpoint_id)
);
```

### Notes

- Uses `rusqlite` with the `bundled` feature — no external `libsqlite3` required.
- Async operations use `tokio::task::spawn_blocking` to avoid blocking the runtime.
- `PUT` is idempotent: re-inserting the same `checkpoint_id` replaces data but does not add a duplicate index entry.

## MongoDB Checkpointer

The `MongoCheckpointer` stores checkpoints in MongoDB, suitable for distributed deployments where multiple processes share state.

### Quick start

```rust,ignore
use synaptic_mongodb::MongoCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

let client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await?;
let db = client.database("myapp");
let checkpointer = MongoCheckpointer::new(&db, "graph_checkpoints").await?;

let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));

let state = MessageState { messages: vec![Message::human("Hello")] };
let config = RunnableConfig::default().with_metadata("thread_id", "user-123");
let result = graph.invoke_with_config(state, config).await?;
```

### Document schema

Each checkpoint is stored as a MongoDB document:

```json
{
  "thread_id":     "user-123",
  "checkpoint_id": "18f4a2b1-0001",
  "seq":           0,
  "state":         "{...serialized Checkpoint JSON...}",
  "created_at":    { "$date": "2026-02-22T00:00:00Z" }
}
```

Two indexes are created automatically:

- **Unique index** on `(thread_id, checkpoint_id)` — ensures idempotent puts.
- **Compound index** on `(thread_id, seq)` — used for ordered `list()` and latest `get()`.

### Notes

- Compatible with MongoDB Atlas and self-hosted MongoDB 5.0+.
- `put()` uses upsert semantics: re-inserting the same checkpoint ID is safe.
- `get()` without a `checkpoint_id` returns the document with the highest `seq`.

## Human-in-the-loop with persistence

Persistent checkpointers enable stateful human-in-the-loop workflows:

```rust,ignore
use synaptic::graph::{StateGraph, MessageState, StreamMode};
use synaptic_sqlite::SqliteCheckpointer;
use std::sync::Arc;

let checkpointer = Arc::new(SqliteCheckpointer::new("/var/lib/myapp/checkpoints.db")?);

// Compile graph with interrupt before "human_review" node
let graph = builder
    .interrupt_before(vec!["human_review"])
    .compile_with_checkpointer(checkpointer)?;

// First invocation — graph pauses before "human_review"
let config = RunnableConfig::default().with_metadata("thread_id", "session-42");
let result = graph.invoke_with_config(initial_state, config.clone()).await?;

// Inject human feedback and resume
let updated = graph.update_state(config.clone(), feedback_state).await?;
let final_result = graph.invoke_with_config(updated, config).await?;
```

## Time-travel debugging

Retrieve any historical checkpoint by ID for debugging or replaying:

```rust,ignore
use synaptic_graph::{CheckpointConfig, Checkpointer};

let config = CheckpointConfig::with_checkpoint_id("thread-123", "specific-checkpoint-id");
if let Some(checkpoint) = checkpointer.get(&config).await? {
    println!("State at checkpoint: {:?}", checkpoint.state);
}

// List all checkpoints for a thread
let all = checkpointer.list(&CheckpointConfig::new("thread-123")).await?;
println!("Total checkpoints: {}", all.len());
```

## Comparison

| Checkpointer | Persistence | External Dep | TTL | Distributed |
|---|---|---|---|---|
| `MemorySaver` | No (in-process) | None | No | No |
| `SqliteCheckpointer` | Yes (file) | None | No | No |
| `RedisCheckpointer` | Yes | Redis | Yes | Yes |
| `PgCheckpointer` | Yes | PostgreSQL | No | Yes |
| `MongoCheckpointer` | Yes | MongoDB | No | Yes |

## Error handling

```rust,ignore
use synaptic::core::SynapticError;

match checkpointer.get(&config).await {
    Ok(Some(cp)) => println!("Loaded checkpoint: {}", cp.id),
    Ok(None) => println!("No checkpoint found"),
    Err(SynapticError::Store(msg)) => eprintln!("Storage error: {msg}"),
    Err(e) => return Err(e.into()),
}
```
