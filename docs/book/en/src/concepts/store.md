# Key-Value Store

The key-value store provides persistent, namespaced storage for structured data. Unlike memory (which stores conversation messages by session), the store holds arbitrary key-value items organized into hierarchical namespaces. It supports CRUD operations, namespace listing, and optional semantic search when an embeddings model is configured.

## The Store Trait

The `Store` trait is defined in `synaptic-core` and implemented in `synaptic-store`:

```rust
#[async_trait]
pub trait Store: Send + Sync {
    async fn put(&self, namespace: &[&str], key: &str, value: Item) -> Result<(), SynapticError>;
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError>;
    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError>;
    async fn search(&self, namespace: &[&str], query: &SearchQuery) -> Result<Vec<Item>, SynapticError>;
    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError>;
}
```

### Namespace Hierarchy

Namespaces are arrays of strings, forming a path-like hierarchy:

```rust
// Store user preferences
store.put(&["users", "alice", "preferences"], "theme", item).await?;

// Store project data
store.put(&["projects", "my-app", "config"], "settings", item).await?;

// List all user namespaces
let namespaces = store.list_namespaces(&["users"]).await?;
// [["users", "alice", "preferences"], ["users", "bob", "preferences"]]
```

Items in different namespaces are completely isolated. A `get` or `search` in one namespace never returns items from another.

## Item

The `Item` struct holds the stored value:

```rust
pub struct Item {
    pub key: String,
    pub value: Value,           // serde_json::Value
    pub namespace: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub score: Option<f32>,     // populated by semantic search
}
```

The `score` field is `None` for regular CRUD operations and is populated only when items are returned from a semantic search query.

## InMemoryStore

The built-in implementation uses `Arc<RwLock<HashMap>>` for thread-safe concurrent access:

```rust
use synaptic::store::InMemoryStore;

let store = InMemoryStore::new();
```

Suitable for development, testing, and applications that don't need persistence across restarts. For production use, implement the `Store` trait with a database backend.

## Semantic Search

When an embeddings model is configured, the store supports semantic search -- finding items by meaning rather than exact key match:

```rust
use synaptic::store::InMemoryStore;

let store = InMemoryStore::with_embeddings(embeddings_model);

// Items are automatically embedded when stored
store.put(&["docs"], "rust-intro", item).await?;

// Search by semantic similarity
let results = store.search(&["docs"], &SearchQuery {
    query: Some("programming language".into()),
    limit: 5,
    ..Default::default()
}).await?;
```

Each returned item has a `score` field (0.0 to 1.0) indicating semantic similarity to the query.

## Store vs. Memory

| Aspect | Store | Memory (MemoryStore) |
|--------|-------|---------------------|
| **Purpose** | General key-value storage | Conversation message history |
| **Keyed by** | Namespace + key | Session ID |
| **Value type** | Arbitrary JSON (`Value`) | `Message` |
| **Operations** | CRUD + search + list | Append + load + clear |
| **Search** | Semantic (with embeddings) | Not applicable |
| **Use case** | Agent knowledge, user profiles, configuration | Chat history, context management |

Use memory for conversation state. Use the store for everything else -- agent knowledge bases, user preferences, cached computations, cross-session data.

## Store in the Graph

The store is accessible within graph nodes through the `ToolRuntime`:

```rust
// Inside a RuntimeAwareTool
async fn call_with_runtime(&self, args: Value, runtime: &ToolRuntime) -> Result<Value, SynapticError> {
    if let Some(store) = &runtime.store {
        let item = store.get(&["memory"], "context").await?;
        // Use stored data in tool execution
    }
    Ok(json!({"status": "ok"}))
}
```

This enables tools to read and write persistent data during graph execution without passing the store through function arguments.

## See Also

- [Key-Value Store How-to](../how-to/store/index.md) -- usage examples and patterns
- [Runtime-Aware Tools](../how-to/tools/runtime-aware.md) -- accessing the store from tools
- [Deep Agent Backends](../how-to/deep-agent/backends.md) -- StoreBackend uses the Store trait
