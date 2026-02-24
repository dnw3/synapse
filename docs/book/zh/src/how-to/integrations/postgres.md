# PostgreSQL 集成

本指南展示如何使用 Synaptic 的 PostgreSQL 集成。`synaptic_postgres` crate 提供四个组件：

| 组件 | Trait | 说明 |
|------|-------|------|
| `PgVectorStore` | `VectorStore` | 向量相似搜索（需要 pgvector 扩展） |
| `PgStore` | `Store` | 键值存储（纯 SQL + JSONB） |
| `PgCache` | `LlmCache` | LLM 响应缓存（纯 SQL + JSONB） |
| `PgCheckpointer` | `Checkpointer` | Graph 状态持久化（纯 SQL + JSONB） |

适合已有 PostgreSQL 基础设施的团队，无需引入额外的外部服务。

## 前置要求

- PostgreSQL >= 12（JSONB + 生成列）
- pgvector >= 0.5.0（仅 VectorStore 需要；Store/Cache 不需要）
- 安装命令：`CREATE EXTENSION IF NOT EXISTS vector;`

## Cargo.toml 配置

```toml
[dependencies]
synaptic = { version = "0.3", features = ["postgres"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## PgVectorStore（向量存储）

### 创建配置

使用 `PgVectorConfig` 指定表名和向量维度：

```rust,ignore
use synaptic::postgres::{PgVectorConfig, PgVectorStore};

let config = PgVectorConfig::new(
    "documents",   // 表名
    1536,          // 向量维度（需匹配 Embedding 模型）
);
```

### 创建连接池并初始化

`PgVectorStore` 接受一个数据库连接池。你需要使用 `sqlx` 或其他兼容库创建连接池：

```rust,ignore
use sqlx::PgPool;

let pool = PgPool::connect("postgresql://user:pass@localhost:5432/mydb").await?;
let store = PgVectorStore::new(pool, config);

// 创建必要的表和索引
store.initialize().await?;
```

`initialize()` 会创建存储文档和向量的表结构。如果表已存在，该操作是安全的。

### 添加文档

```rust,ignore
use synaptic::vectorstores::VectorStore;
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::retrieval::Document;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("pg-1", "PostgreSQL 是世界上最先进的开源关系数据库"),
    Document::new("pg-2", "pgvector 扩展为 PostgreSQL 添加了向量相似性搜索能力"),
    Document::new("pg-3", "HNSW 索引可以加速近似最近邻搜索"),
];

let ids = store.add_documents(docs, &embeddings).await?;
```

### 相似性搜索

```rust,ignore
let results = store.similarity_search("向量搜索", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### 带分数搜索

```rust,ignore
let scored = store.similarity_search_with_score("数据库", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### 删除文档

```rust,ignore
store.delete(&["pg-1", "pg-3"]).await?;
```

## PgStore（键值存储）

`PgStore` 实现了 `Store` trait，提供带命名空间层次的键值存储。不需要 pgvector 扩展，纯 SQL + JSONB 实现。

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

// 存取操作
store.put(&["users"], "alice", json!({"name": "Alice", "age": 30})).await?;
let item = store.get(&["users"], "alice").await?;

// 全文搜索
let results = store.search(&["users"], Some("Alice"), 10).await?;

// 列出命名空间
let namespaces = store.list_namespaces(&[]).await?;
```

## PgCache（LLM 缓存）

`PgCache` 实现了 `LlmCache` trait，提供持久化的 LLM 响应缓存。不需要 pgvector 扩展，纯 SQL + JSONB 实现。支持可选的 TTL 过期。

```rust,ignore
use synaptic::postgres::{PgCache, PgCacheConfig, LlmCache};

let config = PgCacheConfig::new("llm_cache").with_ttl(3600);
let cache = PgCache::new(pool, config);
cache.initialize().await?;
```

## PgCheckpointer（Graph 检查点）

`PgCheckpointer` 实现了 `Checkpointer` trait，用于 Graph 状态的持久化。

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic::postgres::PgCheckpointer;
use synaptic::graph::{create_react_agent, MessageState};
use std::sync::Arc;

// 创建连接池
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

// 创建并初始化检查点（若表不存在则自动创建）
let checkpointer = PgCheckpointer::new(pool);
checkpointer.initialize().await?;

// 构建图
let graph = create_react_agent(model, tools)?
    .with_checkpointer(Arc::new(checkpointer));
```

## 配置选项

### 向量维度

向量维度必须与 Embedding 模型的输出维度一致。常见模型的维度如下：

| Embedding 模型 | 维度 |
|----------------|------|
| `text-embedding-3-small` | 1536 |
| `text-embedding-3-large` | 3072 |
| `text-embedding-ada-002` | 1536 |

### 表名选择

`PgVectorConfig::new()` 的第一个参数是表名。建议按用途命名以便管理：

```rust,ignore
// 知识库文档
let config = PgVectorConfig::new("knowledge_base", 1536);

// 用户问答历史
let config = PgVectorConfig::new("qa_history", 1536);

// 产品描述
let config = PgVectorConfig::new("product_embeddings", 3072);
```

## 功能矩阵

| 功能 | 最低 PG 版本 | 需要扩展 | 说明 |
|------|-------------|---------|------|
| PgStore | 12+ | 无 | 纯 SQL + JSONB |
| PgCache | 12+ | 无 | 纯 SQL + JSONB |
| PgVectorStore | 12+ | pgvector >= 0.5 | 向量相似搜索 |
| PgCheckpointer | 12+ | 无 | 纯 SQL + JSONB |
| Store FTS | 12+ | 无（内置） | tsvector 全文搜索 |

## 常见模式

### 与 VectorStoreRetriever 配合

将 `PgVectorStore` 桥接到 `Retriever` trait：

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::{VectorStoreRetriever, VectorStore};
use synaptic::retrieval::Retriever;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};
use synaptic::embeddings::OpenAiEmbeddings;
use sqlx::PgPool;

let pool = PgPool::connect("postgresql://user:pass@localhost:5432/mydb").await?;
let config = PgVectorConfig::new("documents", 1536);
let store = Arc::new(PgVectorStore::new(pool, config));
store.initialize().await?;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let retriever = VectorStoreRetriever::new(store, embeddings, 5);

let results = retriever.retrieve("PostgreSQL 性能优化", 5).await?;
```

### 利用已有 PostgreSQL 数据

当应用已经使用 PostgreSQL 时，可以复用同一个连接池：

```rust,ignore
use sqlx::PgPool;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};

// 复用应用已有的连接池
let pool = app_state.db_pool.clone();
let config = PgVectorConfig::new("app_embeddings", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;
```

### 完整的 RAG 流水线

从文件加载到存储和检索的完整流程：

```rust,ignore
use synaptic::loaders::{DirectoryLoader, Loader};
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::vectorstores::VectorStore;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};
use synaptic::embeddings::OpenAiEmbeddings;
use sqlx::PgPool;

// 1. 连接数据库
let pool = PgPool::connect("postgresql://user:pass@localhost:5432/mydb").await?;
let config = PgVectorConfig::new("knowledge", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

// 2. 加载并分割文档
let loader = DirectoryLoader::new("./docs")
    .with_glob("*.md")
    .with_recursive(true);
let docs = loader.load().await?;

let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&docs)?;

// 3. 写入 PostgreSQL
let ids = store.add_documents(chunks, &embeddings).await?;
println!("已写入 {} 个文档块到 PostgreSQL", ids.len());

// 4. 搜索
let results = store.similarity_search("查询内容", 5, &embeddings).await?;
```

## 索引策略

[pgvector](https://github.com/pgvector/pgvector) 支持两种索引类型来加速近似最近邻搜索。选择哪种取决于数据集规模和性能需求。

**HNSW**（Hierarchical Navigable Small World）-- 推荐用于大多数场景。它提供更高的召回率、更快的查询速度，且不需要单独的训练步骤。代价是更高的内存占用和更慢的索引构建速度。

**IVFFlat**（Inverted File with Flat compression）-- 适合非常大的数据集且内存受限的场景。它将向量分区到多个列表中，查询时只搜索其中一部分。必须在表中已有数据后才能构建索引（需要代表性向量用于训练）。

```sql
-- HNSW 索引（推荐用于大多数场景）
CREATE INDEX ON documents USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- IVFFlat 索引（适合超大数据集）
CREATE INDEX ON documents USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 100);
```

| 特性 | HNSW | IVFFlat |
|------|------|---------|
| 召回率 | 更高 | 较低 |
| 查询速度 | 更快 | 较慢（取决于 `probes` 参数） |
| 内存占用 | 更高 | 较低 |
| 构建速度 | 较慢 | 更快 |
| 是否需要训练 | 否 | 是（需要已有数据） |

> **提示**：对于少于 10 万行的表，默认的顺序扫描通常已经足够快。当查询延迟成为瓶颈时再考虑添加索引。

## 复用已有连接池

如果你的应用已经维护了一个 `sqlx::PgPool`（例如用于主业务的关系数据），可以直接传给 `PgVectorStore`，无需创建新的连接池：

```rust,ignore
use sqlx::PgPool;
use synaptic::postgres::{PgVectorConfig, PgVectorStore};

// 复用应用状态中的连接池
let pool: PgPool = app_state.db_pool.clone();

let config = PgVectorConfig::new("app_embeddings", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;
```

这样可以避免打开重复的数据库连接，让向量操作与应用的其他数据库操作共享相同的事务边界和连接限制。

### 与 InMemoryVectorStore 的区别

| 特性 | `InMemoryVectorStore` | `PgVectorStore` |
|------|----------------------|-----------------|
| 持久化 | 否（进程退出即丢失） | 是（数据库存储） |
| 数据量 | 适合中小规模 | 适合大规模 |
| 外部依赖 | 无 | PostgreSQL + pgvector |
| 索引支持 | 无（暴力搜索） | HNSW / IVFFlat |
| 适用场景 | 开发测试、原型验证 | 生产部署 |
