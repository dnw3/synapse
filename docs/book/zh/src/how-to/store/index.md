# 键值存储

`Store` trait 为 Agent 提供持久化的键值存储，支持跨调用的状态管理。

## Store Trait

```rust,ignore
use synaptic_store::Store;

#[async_trait]
pub trait Store: Send + Sync {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError>;
    async fn search(&self, namespace: &[&str], query: Option<&str>, limit: usize) -> Result<Vec<Item>, SynapticError>;
    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError>;
    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError>;
    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError>;
}
```

`get()` 或 `search()` 返回的每个 `Item` 包含：

```rust,ignore
pub struct Item {
    pub namespace: Vec<String>,
    pub key: String,
    pub value: Value,
    pub created_at: String,
    pub updated_at: String,
    pub score: Option<f64>,  // populated by semantic search
}
```

## InMemoryStore

```rust,ignore
use synaptic_store::InMemoryStore;

let store = InMemoryStore::new();
store.put(&["users", "prefs"], "theme", json!("dark")).await?;

let item = store.get(&["users", "prefs"], "theme").await?;
```

### 语义搜索

当配置了嵌入模型后，`InMemoryStore` 在 `search()` 查询时使用余弦相似度而非子字符串匹配。搜索结果按相关性排序，`Item::score` 会被填充。

```rust,ignore
use synaptic_store::InMemoryStore;
use synaptic_embeddings::OpenAiEmbeddings;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = InMemoryStore::new().with_embeddings(embeddings);

// Put documents
store.put(&["docs"], "rust", json!("Rust is a systems programming language")).await?;
store.put(&["docs"], "python", json!("Python is an interpreted language")).await?;

// Semantic search — results ranked by similarity
let results = store.search(&["docs"], Some("systems programming"), 10).await?;
// results[0] will be the "rust" item with highest similarity score
assert!(results[0].score.unwrap() > results[1].score.unwrap());
```

未配置嵌入模型时，`search()` 回退到对键和值的子字符串匹配。

## 与 Agent 配合使用

```rust,ignore
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_store::InMemoryStore;

let store = Arc::new(InMemoryStore::new());
let options = AgentOptions {
    store: Some(store),
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

当向 `create_agent` 提供 Store 时，它会自动连接到 `ToolNode`。任何注册到 Agent 的 `RuntimeAwareTool` 都会通过 `ToolRuntime` 接收到该 Store。
