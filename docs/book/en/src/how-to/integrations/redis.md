# Redis Store & Cache

This guide shows how to use Redis for persistent key-value storage and LLM response caching in Synaptic. The `redis` integration provides two components:

- **`RedisStore`** -- implements the `Store` trait for namespace-scoped key-value storage.
- **`RedisCache`** -- implements the `LlmCache` trait for caching LLM responses with optional TTL.

## Setup

Add the `redis` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "redis"] }
```

Ensure you have a Redis server running:

```bash
docker run -p 6379:6379 redis:7
```

## RedisStore

### Creating a store

The simplest way to create a store is from a Redis URL:

```rust,ignore
use synaptic::redis::RedisStore;

let store = RedisStore::from_url("redis://127.0.0.1/")?;
```

### Custom key prefix

By default, all keys are prefixed with `"synaptic:store:"`. You can customize this:

```rust,ignore
use synaptic::redis::{RedisStore, RedisStoreConfig};

let config = RedisStoreConfig {
    prefix: "myapp:store:".to_string(),
};
let store = RedisStore::from_url_with_config("redis://127.0.0.1/", config)?;
```

### Storing and retrieving data

`RedisStore` implements the `Store` trait with full namespace support:

```rust,ignore
use synaptic::redis::Store;
use serde_json::json;

// Put a value under a namespace
store.put(&["users", "prefs"], "theme", json!("dark")).await?;

// Retrieve the value
let item = store.get(&["users", "prefs"], "theme").await?;
if let Some(item) = item {
    println!("Theme: {}", item.value); // "dark"
}
```

### Searching within a namespace

Search for items using substring matching on keys and values:

```rust,ignore
store.put(&["docs"], "rust", json!("Rust is fast")).await?;
store.put(&["docs"], "python", json!("Python is flexible")).await?;

// Search with a query string (substring match)
let results = store.search(&["docs"], Some("fast"), 10).await?;
assert_eq!(results.len(), 1);

// Search without a query (list all items in namespace)
let all = store.search(&["docs"], None, 10).await?;
assert_eq!(all.len(), 2);
```

### Deleting data

```rust,ignore
store.delete(&["users", "prefs"], "theme").await?;
```

### Listing namespaces

List all known namespace paths, optionally filtered by prefix:

```rust,ignore
store.put(&["app", "settings"], "key1", json!("v1")).await?;
store.put(&["app", "cache"], "key2", json!("v2")).await?;
store.put(&["logs"], "key3", json!("v3")).await?;

// List all namespaces
let all_ns = store.list_namespaces(&[]).await?;
// [["app", "settings"], ["app", "cache"], ["logs"]]

// List namespaces under "app"
let app_ns = store.list_namespaces(&["app"]).await?;
// [["app", "settings"], ["app", "cache"]]
```

### Using with agents

Pass the store to `create_agent` so that `RuntimeAwareTool` implementations receive it via `ToolRuntime`:

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{create_agent, AgentOptions};
use synaptic::redis::RedisStore;

let store = Arc::new(RedisStore::from_url("redis://127.0.0.1/")?);
let options = AgentOptions {
    store: Some(store),
    ..Default::default()
};
let graph = create_agent(model, tools, options)?;
```

## RedisCache

### Creating a cache

Create a cache from a Redis URL:

```rust,ignore
use synaptic::redis::RedisCache;

let cache = RedisCache::from_url("redis://127.0.0.1/")?;
```

### Cache with TTL

Set a TTL (in seconds) so entries expire automatically:

```rust,ignore
use synaptic::redis::{RedisCache, RedisCacheConfig};

let config = RedisCacheConfig {
    ttl: Some(3600), // 1 hour
    ..Default::default()
};
let cache = RedisCache::from_url_with_config("redis://127.0.0.1/", config)?;
```

Without a TTL, cached entries persist indefinitely until explicitly cleared.

### Custom key prefix

The default cache prefix is `"synaptic:cache:"`. Customize it to avoid collisions:

```rust,ignore
let config = RedisCacheConfig {
    prefix: "myapp:llm_cache:".to_string(),
    ttl: Some(1800), // 30 minutes
};
let cache = RedisCache::from_url_with_config("redis://127.0.0.1/", config)?;
```

### Wrapping a ChatModel

Use `CachedChatModel` to cache responses from any `ChatModel`:

```rust,ignore
use std::sync::Arc;
use synaptic::core::ChatModel;
use synaptic::cache::CachedChatModel;
use synaptic::redis::RedisCache;
use synaptic::openai::OpenAiChatModel;

let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));
let cache = Arc::new(RedisCache::from_url("redis://127.0.0.1/")?);

let cached_model = CachedChatModel::new(model, cache);
// First call hits the LLM; identical requests return the cached response
```

### Clearing the cache

Remove all cached entries:

```rust,ignore
use synaptic::redis::LlmCache;

cache.clear().await?;
```

This deletes all Redis keys matching the cache prefix.

## Redis Cluster

Synaptic supports Redis Cluster for production deployments that require horizontal scaling and high availability.

### Setup

Enable the `redis-cluster` feature in your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["redis-cluster"] }
```

### Creating a cluster store

```rust,ignore
use synaptic::redis::RedisStore;

let store = RedisStore::from_cluster_nodes(&[
    "redis://127.0.0.1:7000/",
    "redis://127.0.0.1:7001/",
    "redis://127.0.0.1:7002/",
])?;
```

With custom config:

```rust,ignore
use synaptic::redis::{RedisStore, RedisStoreConfig};

let config = RedisStoreConfig {
    prefix: "myapp:store:".to_string(),
};
let store = RedisStore::from_cluster_nodes_with_config(
    &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
    config,
)?;
```

### Creating a cluster cache

```rust,ignore
use synaptic::redis::{RedisCache, RedisCacheConfig};

let config = RedisCacheConfig {
    ttl: Some(3600),
    ..Default::default()
};
let cache = RedisCache::from_cluster_nodes_with_config(
    &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
    config,
)?;
```

### Notes

- All `Store`, `LlmCache`, and `Checkpointer` operations work identically on standalone and cluster backends. The API surface is the same -- only the constructor changes.
- Key enumeration (`search`, `clear`) uses `KEYS` on clusters (redis-rs scatters across nodes automatically) instead of `SCAN` on standalone. These operations are not on the hot path.
- The `redis-cluster` feature pulls in the `cluster-async` feature from the `redis` crate.

## Configuration reference

### RedisStoreConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prefix` | `String` | `"synaptic:store:"` | Key prefix for all store entries |

### RedisCacheConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prefix` | `String` | `"synaptic:cache:"` | Key prefix for all cache entries |
| `ttl` | `Option<u64>` | `None` | TTL in seconds; `None` means entries never expire |

## Key format

- **Store keys**: `{prefix}{namespace_joined_by_colon}:{key}` (e.g. `synaptic:store:users:prefs:theme`)
- **Cache keys**: `{prefix}{key}` (e.g. `synaptic:cache:abc123`)
- **Namespace index**: `{prefix}__namespaces__` (a Redis SET tracking all namespace paths)
