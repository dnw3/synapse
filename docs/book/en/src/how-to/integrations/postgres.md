# PostgreSQL Integration

This guide shows how to use PostgreSQL as a backend for vector storage, key-value storage, LLM response caching, and graph checkpointing in Synaptic. The `synaptic-postgres` crate provides four components that share a single `sqlx::PgPool` connection pool.

## Prerequisites

- **PostgreSQL >= 12** (JSONB + generated columns)
- **pgvector >= 0.5.0** (only required for `PgVectorStore`; `PgStore`, `PgCache`, and `PgCheckpointer` do not need it)
- Install the pgvector extension:

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

Refer to the [pgvector installation guide](https://github.com/pgvector/pgvector#installation) for platform-specific instructions.

## Setup

Add the `postgres` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "postgres"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
```

The `sqlx` dependency is needed to create the connection pool. Synaptic uses `sqlx::PgPool` for all database operations.

## PgVectorStore

### Creating a store

Connect to PostgreSQL and create the store:

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

let config = PgVectorConfig::new("documents", 1536);
let store = PgVectorStore::new(pool, config);
```

The first argument to `PgVectorConfig::new` is the table name; the second is the embedding vector dimensionality (e.g. 1536 for OpenAI `text-embedding-3-small`).

### Initializing the table

Call `initialize()` once to create the pgvector extension and the backing table. This is idempotent and safe to run on every application startup:

```rust,ignore
store.initialize().await?;
```

This creates a table with the following schema:

```sql
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    embedding vector(1536)
);
```

The `vector(N)` column type is provided by the pgvector extension, where `N` matches the `vector_dimensions` in your config.

### Adding documents

`PgVectorStore` implements the `VectorStore` trait. Pass an embeddings provider to compute vectors:

```rust,ignore
use synaptic::postgres::VectorStore;
use synaptic::retrieval::Document;
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
    Document::new("3", "Go is designed for concurrency"),
];

let ids = store.add_documents(docs, &embeddings).await?;
```

Documents with empty IDs are assigned a random UUID. Existing documents with the same ID are upserted (content, metadata, and embedding are updated).

### Similarity search

Find the `k` most similar documents using cosine distance (`<=>`):

```rust,ignore
let results = store.similarity_search("fast systems language", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

#### Search with scores

Get cosine similarity scores (higher is more similar):

```rust,ignore
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

Scores are computed as `1 - cosine_distance`, so a score of 1.0 means identical vectors.

#### Search by vector

Search using a pre-computed embedding vector:

```rust,ignore
use synaptic::embeddings::Embeddings;

let query_vec = embeddings.embed_query("systems programming").await?;
let results = store.similarity_search_by_vector(&query_vec, 3).await?;
```

### Deleting documents

Remove documents by their IDs:

```rust,ignore
store.delete(&["1", "3"]).await?;
```

### Using with a retriever

Wrap the store in a `VectorStoreRetriever` for use with Synaptic's retrieval infrastructure:

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::openai::OpenAiEmbeddings;
use synaptic::retrieval::Retriever;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(store);

let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("fast language", 5).await?;
```

### Schema-qualified table names

You can use schema-qualified names (e.g. `public.documents`) for the table:

```rust,ignore
let config = PgVectorConfig::new("myschema.embeddings", 1536);
```

Table names are validated to contain only alphanumeric characters, underscores, and dots, preventing SQL injection.

## PgStore

`PgStore` implements the `Store` trait for persistent key-value storage with namespace hierarchy and full-text search. It uses pure SQL and JSONB -- no pgvector extension required.

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic::postgres::{PgStore, PgStoreConfig, Store};
use serde_json::json;

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

let config = PgStoreConfig::new("synaptic_store");
let store = PgStore::new(pool, config);
store.initialize().await?;

// Put and get
store.put(&["users"], "alice", json!({"name": "Alice", "age": 30})).await?;
let item = store.get(&["users"], "alice").await?;

// Search with full-text search
let results = store.search(&["users"], Some("Alice"), 10).await?;

// List namespaces
let namespaces = store.list_namespaces(&[]).await?;
```

## PgCache

`PgCache` implements the `LlmCache` trait for persistent LLM response caching. It uses pure SQL and JSONB -- no pgvector extension required. Wrap any `ChatModel` with `CachedChatModel` for transparent caching.

```rust,ignore
use synaptic::postgres::{PgCache, PgCacheConfig, LlmCache};

let config = PgCacheConfig::new("llm_cache").with_ttl(3600);
let cache = PgCache::new(pool, config);
cache.initialize().await?;
```

## PgCheckpointer

`PgCheckpointer` implements the `Checkpointer` trait for persistent graph state. See the [Graph Checkpointers](graph-checkpointer.md) guide for full details.

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic::postgres::PgCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

let checkpointer = PgCheckpointer::new(pool);
checkpointer.initialize().await?;

let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));
```

### Custom table name

```rust,ignore
let checkpointer = PgCheckpointer::new(pool)
    .with_table("my_custom_checkpoints");
checkpointer.initialize().await?;
```

## Capability Matrix

| Capability | Min PG Version | Extension Required | Notes |
|-----------|---------------|-------------------|-------|
| PgStore | 12+ | None | Pure SQL + JSONB |
| PgCache | 12+ | None | Pure SQL + JSONB |
| PgVectorStore | 12+ | pgvector >= 0.5 | Vector similarity search |
| PgCheckpointer | 12+ | None | Pure SQL + JSONB |
| Store FTS | 12+ | None (built-in) | tsvector full-text search |

## Common patterns

### RAG pipeline with PgVectorStore

```rust,ignore
use synaptic::postgres::{PgVectorConfig, PgVectorStore, VectorStore};
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::openai::{OpenAiChatModel, OpenAiEmbeddings};
use synaptic::retrieval::{Document, Retriever};
use synaptic::core::{ChatModel, ChatRequest, Message};
use std::sync::Arc;

// Set up the store
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;
let config = PgVectorConfig::new("knowledge_base", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;

// Add documents
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let docs = vec![
    Document::new("doc1", "Synaptic is a Rust agent framework"),
    Document::new("doc2", "It supports RAG with vector stores"),
];
store.add_documents(docs, embeddings.as_ref()).await?;

// Retrieve and generate
let store = Arc::new(store);
let retriever = VectorStoreRetriever::new(store, embeddings, 3);
let context_docs = retriever.retrieve("What is Synaptic?", 3).await?;

let context = context_docs.iter()
    .map(|d| d.content.as_str())
    .collect::<Vec<_>>()
    .join("\n");

let model = OpenAiChatModel::new("gpt-4o-mini");
let request = ChatRequest::new(vec![
    Message::system(format!("Answer using this context:\n{context}")),
    Message::human("What is Synaptic?"),
]);
let response = model.chat(request).await?;
```

## Index Strategies

pgvector supports two index types for accelerating approximate nearest-neighbor search. Choosing the right one depends on your dataset size and performance requirements.

**HNSW** (Hierarchical Navigable Small World) -- recommended for most use cases. It provides better recall, faster queries at search time, and does not require a separate training step. The trade-off is higher memory usage and slower index build time.

**IVFFlat** (Inverted File with Flat compression) -- a good option for very large datasets where memory is a concern. It partitions vectors into lists and searches only a subset at query time. You must build the index after the table already contains data (it needs representative vectors for training).

```sql
-- HNSW index (recommended for most use cases)
CREATE INDEX ON documents USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- IVFFlat index (better for very large datasets)
CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);
```

| Property | HNSW | IVFFlat |
|----------|------|---------|
| Recall | Higher | Lower |
| Query speed | Faster | Slower (depends on `probes`) |
| Memory usage | Higher | Lower |
| Build speed | Slower | Faster |
| Training required | No | Yes (needs existing data) |

> **Tip**: For tables with fewer than 100k rows, the default sequential scan is often fast enough. Add an index when query latency becomes a concern.

## Reusing an Existing Connection Pool

If your application already maintains a `sqlx::PgPool` (e.g. for your main relational data), you can pass it directly to any of the PostgreSQL components instead of creating a new pool:

```rust,ignore
use sqlx::PgPool;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};

// Reuse the pool from your application state
let pool: PgPool = app_state.db_pool.clone();

let config = PgVectorConfig::new("app_embeddings", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;
```

This avoids opening duplicate connections and lets your vector operations share the same transaction boundaries and connection limits as the rest of your application.

## Configuration reference

### PgVectorConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `table_name` | `String` | required | PostgreSQL table name (supports schema-qualified names) |
| `vector_dimensions` | `u32` | required | Dimensionality of the embedding vectors |

### PgStoreConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `table_name` | `String` | required | PostgreSQL table name |

### PgCacheConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `table_name` | `String` | required | PostgreSQL table name |
| `ttl` | `Option<u64>` | `None` | TTL in seconds for cached entries |
