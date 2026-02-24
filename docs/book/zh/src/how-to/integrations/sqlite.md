# SQLite 集成

本指南介绍如何在 Synaptic 中使用 SQLite 作为缓存、键值存储、向量搜索和图检查点的后端。所有 SQLite 功能使用内嵌引擎，无需外部服务。

## 设置

在 `Cargo.toml` 中添加 `sqlite` feature：

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "sqlite"] }
```

## SqliteCache -- LLM 响应缓存

### 配置

```rust,ignore
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};

// 基于文件的缓存
let config = SqliteCacheConfig::new("cache.db");
let cache = SqliteCache::new(config)?;

// 内存缓存（适用于测试）
let cache = SqliteCache::new(SqliteCacheConfig::in_memory())?;
```

### 设置 TTL

```rust,ignore
let config = SqliteCacheConfig::new("cache.db")
    .with_ttl(3600); // 1 小时后过期

let cache = SqliteCache::new(config)?;
```

### 配合 CachedChatModel 使用

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::cache::CachedChatModel;
use synaptic::sqlite::{SqliteCacheConfig, SqliteCache};
use synaptic::openai::OpenAiChatModel;

let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));
let cache = Arc::new(SqliteCache::new(SqliteCacheConfig::new("llm_cache.db"))?);
let cached_model = CachedChatModel::new(model, cache);

// 第一次调用 -- 请求 LLM 并缓存响应
let request = ChatRequest::new(vec![Message::human("What is Rust?")]);
let response = cached_model.chat(&request).await?;

// 相同请求的第二次调用 -- 直接返回缓存结果
let response2 = cached_model.chat(&request).await?;
```

## SqliteStore -- 带 FTS5 的键值存储

`SqliteStore` 实现了 `Store` trait，内置 FTS5 全文搜索功能。

### 配置

```rust,ignore
use synaptic::sqlite::{SqliteStoreConfig, SqliteStore};

// 基于文件的存储
let store = SqliteStore::new(SqliteStoreConfig::new("store.db"))?;

// 内存存储（适用于测试）
let store = SqliteStore::new(SqliteStoreConfig::in_memory())?;
```

### 基本 CRUD 操作

```rust,ignore
use synaptic::core::Store;
use serde_json::json;

// 写入数据
store.put(&["users"], "alice", json!({"name": "Alice", "role": "admin"})).await?;

// 读取数据
let item = store.get(&["users"], "alice").await?;

// 删除数据
store.delete(&["users"], "alice").await?;
```

### 全文搜索

`search()` 方法在提供查询时使用 FTS5 进行全文搜索：

```rust,ignore
// 使用 FTS5 全文搜索
let results = store.search(&["docs"], Some("Rust programming"), 10).await?;

// 列出命名空间中的所有条目（无查询）
let all = store.search(&["docs"], None, 100).await?;
```

### 命名空间管理

```rust,ignore
// 列出所有命名空间
let namespaces = store.list_namespaces(&[]).await?;

// 按前缀过滤命名空间
let user_ns = store.list_namespaces(&["users"]).await?;
```

## SqliteVectorStore -- 带 FTS5 混合搜索的向量存储

`SqliteVectorStore` 实现了 `VectorStore` trait。向量以 BLOB 格式存储，余弦相似度在 Rust 中计算。

### 配置

```rust,ignore
use synaptic::sqlite::{SqliteVectorStoreConfig, SqliteVectorStore};

let store = SqliteVectorStore::new(SqliteVectorStoreConfig::new("vectors.db"))?;
// 或内存模式：
let store = SqliteVectorStore::new(SqliteVectorStoreConfig::in_memory())?;
```

### 添加和搜索文档

```rust,ignore
use synaptic::core::{Document, VectorStore};
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

// 添加文档
let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
];
store.add_documents(docs, &embeddings).await?;

// 相似性搜索
let results = store.similarity_search("systems programming", 5, &embeddings).await?;

// 带分数的搜索
let scored = store.similarity_search_with_score("systems", 5, &embeddings).await?;
for (doc, score) in &scored {
    println!("{}: {:.3}", doc.id, score);
}
```

### 混合搜索（向量 + FTS5）

结合余弦相似度和 BM25 文本相关性：

```rust,ignore
// alpha 控制平衡：
//   1.0 = 纯向量相似度
//   0.0 = 纯 BM25 文本相关性
//   0.5 = 均衡（推荐）
let results = store.hybrid_search("Rust programming", 5, &embeddings, 0.5).await?;
for (doc, score) in &results {
    println!("{}: {:.3}", doc.content, score);
}
```

## 配置参考

### SqliteCacheConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `path` | `String` | 必填 | 数据库文件路径（或 `":memory:"` 表示内存模式） |
| `ttl` | `Option<u64>` | `None` | 缓存条目过期时间（秒），`None` 表示永不过期 |

### SqliteStoreConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `path` | `String` | 必填 | 数据库文件路径（或 `":memory:"` 表示内存模式） |

### SqliteVectorStoreConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `path` | `String` | 必填 | 数据库文件路径（或 `":memory:"` 表示内存模式） |
