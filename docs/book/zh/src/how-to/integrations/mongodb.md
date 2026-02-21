# MongoDB Atlas 向量搜索

本指南展示如何使用 Synaptic 的 MongoDB 集成，利用 [MongoDB Atlas](https://www.mongodb.com/atlas) 的向量搜索功能进行相似性检索。MongoDB Atlas 提供原生的向量搜索索引，可以在现有 MongoDB 部署上启用向量检索能力。

## 设置

在 `Cargo.toml` 中添加 `mongodb` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "mongodb"] }
```

### 前置条件

1. 一个 MongoDB Atlas 集群（M10 或以上，向量搜索需要 Atlas 专用集群）
2. 预先创建 Atlas Search 索引（带向量字段映射）

在 Atlas 控制台中为你的集合创建搜索索引，索引定义示例：

```json
{
  "fields": [
    {
      "type": "vector",
      "path": "embedding",
      "numDimensions": 1536,
      "similarity": "cosine"
    }
  ]
}
```

## 配置

使用 `MongoVectorConfig` 创建配置：

```rust,ignore
use synaptic::mongodb::{MongoVectorConfig, MongoVectorStore};

let config = MongoVectorConfig::new(
    "my_database",       // 数据库名称
    "my_collection",     // 集合名称
    "vector_index",      // Atlas Search 索引名称
    1536,                // 向量维度
);
```

### 从 URI 创建

使用 MongoDB 连接字符串创建存储实例：

```rust,ignore
let store = MongoVectorStore::from_uri(
    "mongodb+srv://user:pass@cluster.mongodb.net",
    config,
).await?;
```

### 自定义字段名称

默认的内容字段为 `"content"`，嵌入字段为 `"embedding"`。如需自定义：

```rust,ignore
let config = MongoVectorConfig::new("db", "collection", "index", 1536)
    .with_content_field("text")
    .with_embedding_field("vector");
```

## 用法

### 添加文档

`MongoVectorStore` 实现了 `VectorStore` trait：

```rust,ignore
use synaptic::mongodb::MongoVectorStore;
use synaptic::core::{VectorStore, Document, Embeddings};
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("1", "Rust 是一门系统编程语言"),
    Document::new("2", "Python 适合数据科学"),
    Document::new("3", "Go 擅长并发编程"),
];

let ids = store.add_documents(docs, &embeddings).await?;
```

### 相似性搜索

```rust,ignore
let results = store.similarity_search("系统编程", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### 带分数搜索

```rust,ignore
let scored = store.similarity_search_with_score("并发", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### 删除文档

```rust,ignore
store.delete(&["1", "3"]).await?;
```

## 与 Retriever 配合使用

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::core::Retriever;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(store);

let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("查询内容", 5).await?;
```

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `database` | `String` | 必填 | MongoDB 数据库名称 |
| `collection` | `String` | 必填 | MongoDB 集合名称 |
| `index_name` | `String` | 必填 | Atlas Search 索引名称 |
| `dims` | `u32` | 必填 | 向量维度 |
| `content_field` | `String` | `"content"` | 文档内容字段名称 |
| `embedding_field` | `String` | `"embedding"` | 嵌入向量字段名称 |
