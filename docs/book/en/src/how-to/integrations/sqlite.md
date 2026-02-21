# SQLite Cache

This guide shows how to use SQLite as a persistent LLM response cache in Synaptic. `SqliteCache` stores chat model responses locally so identical requests are served from disk without calling the LLM again.

## Setup

Add the `sqlite` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "sqlite"] }
```

No external service is required. The cache uses a local SQLite file (or an in-memory database for testing).

## Configuration

### File-based cache

Create a `SqliteCacheConfig` pointing to a database file:

```rust,ignore
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};

let config = SqliteCacheConfig::new("cache.db");
let cache = SqliteCache::new(config).await?;
```

The database file is created automatically if it does not exist. The constructor is async because it initializes the database schema.

### In-memory cache

For testing or ephemeral use, create an in-memory SQLite cache:

```rust,ignore
let config = SqliteCacheConfig::in_memory();
let cache = SqliteCache::new(config).await?;
```

### TTL (time-to-live)

Set an optional TTL so cached entries expire automatically:

```rust,ignore
use std::time::Duration;

let config = SqliteCacheConfig::new("cache.db")
    .with_ttl(Duration::from_secs(3600)); // 1 hour

let cache = SqliteCache::new(config).await?;
```

Without a TTL, cached entries persist indefinitely.

## Usage

### Wrapping a ChatModel

Use `CachedChatModel` from `synaptic-cache` to wrap any `ChatModel`:

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::cache::CachedChatModel;
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};
use synaptic::openai::OpenAiChatModel;

let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

let config = SqliteCacheConfig::new("llm_cache.db");
let cache = Arc::new(SqliteCache::new(config).await?);

let cached_model = CachedChatModel::new(model, cache);

// First call hits the LLM
let request = ChatRequest::new(vec![Message::human("What is Rust?")]);
let response = cached_model.chat(&request).await?;

// Second identical call returns the cached response instantly
let response2 = cached_model.chat(&request).await?;
```

### Direct cache access

`SqliteCache` implements the `LlmCache` trait, so you can use it directly:

```rust,ignore
use synaptic::core::LlmCache;

// Look up a cached response by key
let cached = cache.lookup("some-cache-key").await?;

// Store a response
cache.update("some-cache-key", &response).await?;

// Clear all entries
cache.clear().await?;
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `String` | required | Path to the SQLite database file (or `":memory:"` for in-memory) |
| `ttl` | `Option<Duration>` | `None` | Time-to-live for cache entries; `None` means entries never expire |
