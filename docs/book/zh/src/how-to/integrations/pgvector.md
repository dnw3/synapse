# PostgreSQL pgvector

本指南展示如何使用 Synaptic 的 pgvector 集成，将 PostgreSQL 作为向量存储后端进行文档 Embedding 的存储和相似性搜索。

## 概述

`synaptic_pgvector` crate 提供了 `PgVectorStore`，它实现了 `VectorStore` trait，利用 PostgreSQL 的 [pgvector](https://github.com/pgvector/pgvector) 扩展来存储和检索向量数据。适合已有 PostgreSQL 基础设施的团队，无需引入额外的向量数据库服务。

## 前置条件

1. PostgreSQL 15 或更高版本
2. 安装 pgvector 扩展：

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

## Cargo.toml 配置

```toml
[dependencies]
synaptic = { version = "0.3", features = ["pgvector"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 基础使用

### 创建配置

使用 `PgVectorConfig` 指定表名和向量维度：

```rust,ignore
use synaptic::pgvector::{PgVectorConfig, PgVectorStore};

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

## 常见模式

### 与 VectorStoreRetriever 配合

将 `PgVectorStore` 桥接到 `Retriever` trait：

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::{VectorStoreRetriever, VectorStore};
use synaptic::retrieval::Retriever;
use synaptic::pgvector::{PgVectorConfig, PgVectorStore};
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
use synaptic::pgvector::{PgVectorConfig, PgVectorStore};

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
use synaptic::pgvector::{PgVectorConfig, PgVectorStore};
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

// 3. 写入 pgvector
let ids = store.add_documents(chunks, &embeddings).await?;
println!("已写入 {} 个文档块到 PostgreSQL", ids.len());

// 4. 搜索
let results = store.similarity_search("查询内容", 5, &embeddings).await?;
```

### 与 InMemoryVectorStore 的区别

| 特性 | `InMemoryVectorStore` | `PgVectorStore` |
|------|----------------------|-----------------|
| 持久化 | 否（进程退出即丢失） | 是（数据库存储） |
| 数据量 | 适合中小规模 | 适合大规模 |
| 外部依赖 | 无 | PostgreSQL + pgvector |
| 索引支持 | 无（暴力搜索） | HNSW / IVFFlat |
| 适用场景 | 开发测试、原型验证 | 生产部署 |
