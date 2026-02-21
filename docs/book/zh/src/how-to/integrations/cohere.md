# Cohere Reranker

本指南展示如何使用 Synaptic 的 Cohere 集成进行文档重排序。Cohere Reranker 使用专门的重排序模型对检索结果进行精细排序，提升检索质量。

## 设置

在 `Cargo.toml` 中添加 `cohere` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["cohere"] }
```

设置 API 密钥环境变量：

```bash
export CO_API_KEY="your-cohere-api-key"
```

> **注意：** Cohere 的 chat 和 embeddings 功能请使用 [OpenAI 兼容构造器](openai-compatible.md)（`cohere_chat_model` 和 `cohere_embeddings`），本 crate 专注于 Reranker 功能。

## 配置

使用 `CohereRerankerConfig` 创建配置：

```rust,ignore
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};

let config = CohereRerankerConfig::new("your-api-key");
let reranker = CohereReranker::new(config);
```

### 自定义模型

默认使用 `rerank-v3.5` 模型。可以通过 `with_model()` 指定其他模型：

```rust,ignore
let config = CohereRerankerConfig::new("your-api-key")
    .with_model("rerank-english-v3.0");
```

### 自定义 Base URL

如果使用代理或自定义端点：

```rust,ignore
let config = CohereRerankerConfig::new("your-api-key")
    .with_base_url("https://my-proxy.example.com");
```

## 用法

### 基础重排序

对文档列表按照与查询的相关性进行重排序：

```rust,ignore
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};
use synaptic::core::Document;

let config = CohereRerankerConfig::new("your-api-key");
let reranker = CohereReranker::new(config);

let docs = vec![
    Document::new("1", "Python 是一门解释型编程语言"),
    Document::new("2", "Rust 以内存安全和高性能著称"),
    Document::new("3", "JavaScript 广泛用于 Web 开发"),
    Document::new("4", "Rust 的所有权系统可以在编译时防止数据竞争"),
];

let results = reranker.rerank("Rust 编程语言的优势", &docs, 2).await?;

for (doc, score) in &results {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### 作为 DocumentCompressor 使用

启用 `retrieval` feature 后，`CohereReranker` 实现了 `DocumentCompressor` trait，可以与 `ContextualCompressionRetriever` 配合使用：

```rust,ignore
use std::sync::Arc;
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};
use synaptic::retrieval::{ContextualCompressionRetriever, Retriever};

let reranker = Arc::new(CohereReranker::new(
    CohereRerankerConfig::new("your-api-key"),
));

// base_retriever 是任何实现了 Retriever trait 的检索器
let compression_retriever = ContextualCompressionRetriever::new(
    base_retriever,
    reranker,
);

let results = compression_retriever.retrieve("查询内容", 5).await?;
```

### 典型 RAG 流水线

在 RAG 流水线中使用 Cohere Reranker 对初始检索结果进行精排：

```rust,ignore
use std::sync::Arc;
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};
use synaptic::retrieval::{ContextualCompressionRetriever, Retriever};
use synaptic::vectorstores::{InMemoryVectorStore, VectorStoreRetriever, VectorStore};
use synaptic::openai::OpenAiEmbeddings;

// 1. 设置向量存储和 base retriever
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(InMemoryVectorStore::new());
store.add_documents(docs, embeddings.as_ref()).await?;

let base_retriever = Arc::new(
    VectorStoreRetriever::new(store, embeddings, 20),  // 初始检索 20 个
);

// 2. 用 Cohere Reranker 进行精排
let reranker = Arc::new(CohereReranker::new(
    CohereRerankerConfig::new("your-api-key"),
));

let retriever = ContextualCompressionRetriever::new(base_retriever, reranker);

// 3. 获取精排后的结果
let results = retriever.retrieve("查询", 5).await?;  // 返回 top 5
```

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | `String` | 必填 | Cohere API 密钥 |
| `model` | `String` | `"rerank-v3.5"` | Reranker 模型名称 |
| `base_url` | `String` | `"https://api.cohere.com/v2"` | API Base URL |
