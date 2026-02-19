# 键值存储

键值存储提供持久化的、带命名空间的结构化数据存储。与记忆（按会话存储对话消息）不同，存储以层级命名空间组织的方式保存任意键值项。它支持 CRUD 操作、命名空间列表查询，以及在配置了嵌入模型时的可选语义搜索。

## Store Trait

`Store` trait 在 `synaptic-core` 中定义，在 `synaptic-store` 中实现：

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

### 命名空间层级

命名空间是字符串数组，形成类似路径的层级结构：

```rust
// 存储用户偏好设置
store.put(&["users", "alice", "preferences"], "theme", item).await?;

// 存储项目数据
store.put(&["projects", "my-app", "config"], "settings", item).await?;

// 列出所有用户命名空间
let namespaces = store.list_namespaces(&["users"]).await?;
// [["users", "alice", "preferences"], ["users", "bob", "preferences"]]
```

不同命名空间中的项是完全隔离的。在一个命名空间中执行 `get` 或 `search` 永远不会返回其他命名空间的项。

## Item

`Item` 结构体保存存储的值：

```rust
pub struct Item {
    pub key: String,
    pub value: Value,           // serde_json::Value
    pub namespace: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub score: Option<f32>,     // 由语义搜索填充
}
```

`score` 字段在常规 CRUD 操作中为 `None`，仅在语义搜索查询返回结果时被填充。

## InMemoryStore

内置实现使用 `Arc<RwLock<HashMap>>` 提供线程安全的并发访问：

```rust
use synaptic::store::InMemoryStore;

let store = InMemoryStore::new();
```

适用于开发、测试以及不需要跨重启持久化的应用场景。对于生产环境，请使用数据库后端实现 `Store` trait。

## 语义搜索

当配置了嵌入模型时，存储支持语义搜索 -- 根据语义而非精确键匹配来查找项：

```rust
use synaptic::store::InMemoryStore;

let store = InMemoryStore::with_embeddings(embeddings_model);

// 存储时自动生成嵌入向量
store.put(&["docs"], "rust-intro", item).await?;

// 按语义相似度搜索
let results = store.search(&["docs"], &SearchQuery {
    query: Some("programming language".into()),
    limit: 5,
    ..Default::default()
}).await?;
```

每个返回的项都有一个 `score` 字段（0.0 到 1.0），表示与查询的语义相似度。

## Store 与 Memory 的对比

| 方面 | Store | Memory (`MemoryStore`) |
|------|-------|------------------------|
| **用途** | 通用键值存储 | 对话消息历史 |
| **索引方式** | 命名空间 + 键 | 会话 ID |
| **值类型** | 任意 JSON（`Value`） | `Message` |
| **操作** | CRUD + 搜索 + 列表 | 追加 + 加载 + 清除 |
| **搜索** | 语义搜索（需要嵌入模型） | 不适用 |
| **使用场景** | 智能体知识库、用户画像、配置 | 聊天历史、上下文管理 |

对话状态使用 memory。其他所有场景使用 store -- 智能体知识库、用户偏好、缓存计算结果、跨会话数据。

## 图中的 Store

在图节点中可以通过 `ToolRuntime` 访问 store：

```rust
// 在 RuntimeAwareTool 内部
async fn call_with_runtime(&self, args: Value, runtime: &ToolRuntime) -> Result<Value, SynapticError> {
    if let Some(store) = &runtime.store {
        let item = store.get(&["memory"], "context").await?;
        // 在工具执行中使用存储的数据
    }
    Ok(json!({"status": "ok"}))
}
```

这使得工具可以在图执行过程中读写持久化数据，无需通过函数参数传递 store。

## 另请参阅

- [键值存储使用指南](../how-to/store/index.md) -- 使用示例和模式
- [运行时感知工具](../how-to/tools/runtime-aware.md) -- 从工具中访问 store
- [Deep Agent 后端](../how-to/deep-agent/backends.md) -- `StoreBackend` 使用 `Store` trait
