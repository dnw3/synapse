# Qdrant 向量存储

本指南展示如何使用 Synaptic 的 Qdrant 集成将文档 Embedding 存储到 [Qdrant](https://qdrant.tech/) 向量数据库中，并进行相似性搜索。

## 概述

`synaptic_qdrant` crate 提供了 `QdrantVectorStore`，它实现了 `VectorStore` trait，将 Qdrant 作为后端向量数据库。Qdrant 是一个高性能的开源向量数据库，支持分布式部署、多种距离度量和丰富的过滤条件。

## Cargo.toml 配置

```toml
[dependencies]
synaptic = { version = "0.3", features = ["qdrant"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 基础使用

### 创建配置

使用 `QdrantConfig` 配置 Qdrant 连接参数：

```rust,ignore
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};

let config = QdrantConfig::new(
    "http://localhost:6334",   // Qdrant gRPC 地址
    "my_collection",           // Collection 名称
    1536,                      // 向量维度（需匹配 Embedding 模型）
);
```

### 创建 VectorStore 并初始化 Collection

```rust,ignore
let store = QdrantVectorStore::new(config);

// 确保 Collection 存在，不存在时自动创建
store.ensure_collection().await?;
```

`ensure_collection()` 是幂等的 -- 如果 Collection 已存在，不会重复创建。

### 添加文档

```rust,ignore
use synaptic::vectorstores::VectorStore;
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::retrieval::Document;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("doc-1", "Rust 是一门注重安全和性能的系统编程语言"),
    Document::new("doc-2", "Python 广泛用于数据科学和机器学习"),
    Document::new("doc-3", "Go 以其简洁的并发模型著称"),
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
store.delete(&["doc-1", "doc-3"]).await?;
```

## 配置选项

### API Key 认证

连接需要认证的 Qdrant Cloud 实例：

```rust,ignore
let config = QdrantConfig::new("https://my-cluster.qdrant.io:6334", "documents", 1536)
    .with_api_key("your-api-key");
```

### 距离度量

Qdrant 支持多种距离度量方式。默认使用 Cosine：

```rust,ignore
use synaptic::qdrant::QdrantConfig;

// 使用欧氏距离
let config = QdrantConfig::new(url, collection, 1536)
    .with_distance("euclid");

// 使用点积
let config = QdrantConfig::new(url, collection, 1536)
    .with_distance("dot");

// 使用余弦相似度（默认）
let config = QdrantConfig::new(url, collection, 1536)
    .with_distance("cosine");
```

### 向量维度

向量维度（`vector_size`）必须与所使用的 Embedding 模型输出维度一致：

| Embedding 模型 | 维度 |
|----------------|------|
| `text-embedding-3-small` | 1536 |
| `text-embedding-3-large` | 3072 |
| `text-embedding-ada-002` | 1536 |

## 常见模式

### 与 VectorStoreRetriever 配合

将 `QdrantVectorStore` 桥接到 `Retriever` trait，使其融入 RAG 流水线：

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::{VectorStoreRetriever, VectorStore};
use synaptic::retrieval::Retriever;
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};
use synaptic::embeddings::OpenAiEmbeddings;

let config = QdrantConfig::new("http://localhost:6334", "knowledge_base", 1536);
let store = Arc::new(QdrantVectorStore::new(config));
store.ensure_collection().await?;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let retriever = VectorStoreRetriever::new(store, embeddings, 5);

let results = retriever.retrieve("什么是所有权？", 5).await?;
```

### 与 Agent 配合

在 Agent 的工具中使用 Qdrant 进行知识检索：

```rust,ignore
use std::sync::Arc;
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::embeddings::OpenAiEmbeddings;

let config = QdrantConfig::new("http://localhost:6334", "docs", 1536)
    .with_api_key("your-key");
let store = Arc::new(QdrantVectorStore::new(config));
store.ensure_collection().await?;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let retriever = VectorStoreRetriever::new(store, embeddings, 5);

// 在 Agent 工具中使用 retriever 进行知识检索
```

### 批量导入文档

加载文档后批量写入 Qdrant：

```rust,ignore
use synaptic::loaders::{DirectoryLoader, Loader};
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::vectorstores::VectorStore;

// 1. 加载文档
let loader = DirectoryLoader::new("./knowledge")
    .with_glob("*.md")
    .with_recursive(true);
let docs = loader.load().await?;

// 2. 分割为小块
let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&docs)?;

// 3. 写入 Qdrant
let ids = store.add_documents(chunks, &embeddings).await?;
println!("已导入 {} 个文档块", ids.len());
```
