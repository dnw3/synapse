# Pinecone 向量存储

本指南展示如何使用 Synaptic 的 Pinecone 集成进行向量存储和相似性搜索。[Pinecone](https://www.pinecone.io/) 是一个全托管的向量数据库，专为大规模相似性搜索而设计。

## 设置

在 `Cargo.toml` 中添加 `pinecone` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "pinecone"] }
```

你需要在 Pinecone 控制台中创建一个索引，并获取以下信息：

- **API Key** -- 在 Pinecone 控制台的 API Keys 页面获取
- **Host** -- 索引的 URL，格式如 `https://my-index-abc1234.svc.aped-1234-ab12.pinecone.io`

```bash
export PINECONE_API_KEY="your-pinecone-api-key"
```

## 配置

使用 `PineconeConfig` 创建配置：

```rust,ignore
use synaptic::pinecone::{PineconeConfig, PineconeVectorStore};

let config = PineconeConfig::new(
    "your-api-key",
    "https://my-index-abc1234.svc.aped-1234-ab12.pinecone.io",
);

let store = PineconeVectorStore::new(config);
```

`host` 参数是 Pinecone 控制台中索引的完整 URL。每个索引都有一个唯一的 URL。

### 自定义命名空间

Pinecone 支持命名空间来隔离数据：

```rust,ignore
let config = PineconeConfig::new("api-key", "https://my-index.pinecone.io")
    .with_namespace("production");
```

## 用法

### 添加文档

`PineconeVectorStore` 实现了 `VectorStore` trait：

```rust,ignore
use synaptic::pinecone::PineconeVectorStore;
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

将 Pinecone 存储桥接到 `Retriever` trait：

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
| `api_key` | `String` | 必填 | Pinecone API 密钥 |
| `host` | `String` | 必填 | 索引的完整 URL（从 Pinecone 控制台获取） |
| `namespace` | `Option<String>` | `None` | 可选的命名空间 |
