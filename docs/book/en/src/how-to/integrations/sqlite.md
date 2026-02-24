# SQLite Integration

This guide covers using SQLite as a backend for caching, key-value storage, vector search, and graph checkpointing in Synaptic. All SQLite features use a bundled engine -- no external service required.

## Setup

Add the `sqlite` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "sqlite"] }
```

## SqliteCache -- LLM Response Cache

### Configuration

```rust,ignore
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};

// File-based cache
let config = SqliteCacheConfig::new("cache.db");
let cache = SqliteCache::new(config)?;

// In-memory cache (for testing)
let cache = SqliteCache::new(SqliteCacheConfig::in_memory())?;
```

### TTL (time-to-live)

```rust,ignore
let config = SqliteCacheConfig::new("cache.db")
    .with_ttl(3600); // 1 hour

let cache = SqliteCache::new(config)?;
```

### Wrapping a ChatModel

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::cache::CachedChatModel;
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};
use synaptic::openai::OpenAiChatModel;

let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));
let cache = Arc::new(SqliteCache::new(SqliteCacheConfig::new("llm_cache.db"))?);
let cached_model = CachedChatModel::new(model, cache);

// First call hits the LLM
let request = ChatRequest::new(vec![Message::human("What is Rust?")]);
let response = cached_model.chat(&request).await?;

// Second identical call returns the cached response instantly
let response2 = cached_model.chat(&request).await?;
```

## SqliteStore -- Key-Value Store with FTS5

`SqliteStore` implements the `Store` trait with built-in FTS5 full-text search.

### Configuration

```rust,ignore
use synaptic::sqlite::{SqliteStoreConfig, SqliteStore};

// File-based store
let store = SqliteStore::new(SqliteStoreConfig::new("store.db"))?;

// In-memory store (for testing)
let store = SqliteStore::new(SqliteStoreConfig::in_memory())?;
```

### Basic CRUD

```rust,ignore
use synaptic::core::Store;
use serde_json::json;

// Put a value
store.put(&["users"], "alice", json!({"name": "Alice", "role": "admin"})).await?;

// Get a value
let item = store.get(&["users"], "alice").await?;

// Delete a value
store.delete(&["users"], "alice").await?;
```

### Full-Text Search

The `search()` method uses FTS5 for full-text search when a query is provided:

```rust,ignore
// Search with FTS5 full-text query
let results = store.search(&["docs"], Some("Rust programming"), 10).await?;

// List all items in a namespace (no query)
let all = store.search(&["docs"], None, 100).await?;
```

### Namespace Management

```rust,ignore
// List all namespaces
let namespaces = store.list_namespaces(&[]).await?;

// List namespaces with a prefix
let user_ns = store.list_namespaces(&["users"]).await?;
```

## SqliteVectorStore -- Vector Search with FTS5 Hybrid

`SqliteVectorStore` implements the `VectorStore` trait. Embeddings are stored as BLOBs and cosine similarity is computed in Rust.

### Configuration

```rust,ignore
use synaptic::sqlite::{SqliteVectorStoreConfig, SqliteVectorStore};

let store = SqliteVectorStore::new(SqliteVectorStoreConfig::new("vectors.db"))?;
// or in-memory:
let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory())?;
```

### Adding and Searching Documents

```rust,ignore
use synaptic::core::{Document, VectorStore};
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

// Add documents
let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
];
store.add_documents(docs, &embeddings).await?;

// Similarity search
let results = store.similarity_search("systems programming", 5, &embeddings).await?;

// Search with scores
let scored = store.similarity_search_with_score("systems", 5, &embeddings).await?;
for (doc, score) in &scored {
    println!("{}: {:.3}", doc.id, score);
}
```

### Hybrid Search (Vector + FTS5)

Combine cosine similarity with BM25 text relevance:

```rust,ignore
// alpha controls the balance:
//   1.0 = pure vector similarity
//   0.0 = pure BM25 text relevance
//   0.5 = balanced (recommended)
let results = store.hybrid_search("Rust programming", 5, &embeddings, 0.5).await?;
for (doc, score) in &results {
    println!("{}: {:.3}", doc.content, score);
}
```

## Configuration Reference

### SqliteCacheConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `String` | required | Path to the SQLite database file (or `":memory:"`) |
| `ttl` | `Option<u64>` | `None` | TTL in seconds; `None` means entries never expire |

### SqliteStoreConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `String` | required | Path to the SQLite database file (or `":memory:"`) |

### SqliteVectorStoreConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `String` | required | Path to the SQLite database file (or `":memory:"`) |
