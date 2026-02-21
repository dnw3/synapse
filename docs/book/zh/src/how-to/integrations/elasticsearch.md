# Elasticsearch 向量存储

本指南展示如何使用 Synaptic 的 Elasticsearch 集成进行向量存储和 kNN 相似性搜索。[Elasticsearch](https://www.elastic.co/elasticsearch) 从 8.0 版本开始支持原生的 dense vector 字段和 kNN 搜索。

## 设置

在 `Cargo.toml` 中添加 `elasticsearch` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "elasticsearch"] }
```

启动 Elasticsearch 实例（例如通过 Docker）：

```bash
docker run -p 9200:9200 \
  -e "discovery.type=single-node" \
  -e "xpack.security.enabled=false" \
  elasticsearch:8.12.0
```

## 配置

使用 `ElasticsearchConfig` 创建配置：

```rust,ignore
use synaptic::elasticsearch::{ElasticsearchConfig, ElasticsearchVectorStore};

let config = ElasticsearchConfig::new(
    "http://localhost:9200",   // Elasticsearch URL
    "my_index",                // 索引名称
    1536,                      // 向量维度
);

let store = ElasticsearchVectorStore::new(config);
```

### 认证

如果 Elasticsearch 启用了安全认证，使用 `with_credentials()` 设置用户名和密码：

```rust,ignore
let config = ElasticsearchConfig::new("https://my-es-cluster:9200", "docs", 1536)
    .with_credentials("elastic", "your-password");
```

### 自定义字段名称

默认的内容字段为 `"content"`，嵌入字段为 `"embedding"`。如需自定义：

```rust,ignore
let config = ElasticsearchConfig::new("http://localhost:9200", "my_index", 1536)
    .with_content_field("text")
    .with_embedding_field("vector");
```

## 创建索引

调用 `ensure_index()` 创建带 kNN 映射的索引。如果索引已存在，则不会重复创建：

```rust,ignore
store.ensure_index().await?;
```

此操作会创建一个包含以下映射的索引：

- `content` 字段（`text` 类型）
- `embedding` 字段（`dense_vector` 类型，指定维度，kNN 索引）
- `metadata` 字段（`object` 类型）

## 用法

### 添加文档

`ElasticsearchVectorStore` 实现了 `VectorStore` trait：

```rust,ignore
use synaptic::elasticsearch::ElasticsearchVectorStore;
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
| `url` | `String` | 必填 | Elasticsearch URL（例如 `http://localhost:9200`） |
| `index_name` | `String` | 必填 | 索引名称 |
| `dims` | `u32` | 必填 | 向量维度 |
| `username` | `Option<String>` | `None` | 认证用户名 |
| `password` | `Option<String>` | `None` | 认证密码 |
| `content_field` | `String` | `"content"` | 文档内容字段名称 |
| `embedding_field` | `String` | `"embedding"` | 嵌入向量字段名称 |
