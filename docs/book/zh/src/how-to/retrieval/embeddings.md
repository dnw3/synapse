# Embeddings

本指南展示如何使用 Synaptic 的 `Embeddings` trait 及其内置提供商将文本转换为向量表示。

## 概述

所有 Embeddings 提供商都实现了 `synaptic_embeddings` 中的 `Embeddings` trait：

```rust
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError>;
}
```

- `embed_documents()` 在单次批处理中嵌入多个文本 -- 用于索引。
- `embed_query()` 嵌入单个查询文本 -- 用于检索时。

## FakeEmbeddings

基于输入文本的简单哈希生成确定性向量。适用于测试和开发，无需 API 调用。

```rust
use synaptic::embeddings::FakeEmbeddings;
use synaptic::embeddings::Embeddings;

// Specify the number of dimensions (default is 4)
let embeddings = FakeEmbeddings::new(4);

let doc_vectors = embeddings.embed_documents(&["doc one", "doc two"]).await?;
let query_vector = embeddings.embed_query("search query").await?;

// Vectors are normalized to unit length
// Similar texts produce similar vectors
```

## OpenAiEmbeddings

使用 OpenAI Embeddings API。需要 API 密钥和 `ProviderBackend`。

```rust
use std::sync::Arc;
use synaptic::embeddings::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};
use synaptic::embeddings::Embeddings;
use synaptic::models::backend::HttpBackend;

let config = OpenAiEmbeddingsConfig::new("sk-...")
    .with_model("text-embedding-3-small");  // default model

let backend = Arc::new(HttpBackend::new());
let embeddings = OpenAiEmbeddings::new(config, backend);

let vectors = embeddings.embed_documents(&["hello world"]).await?;
```

你可以自定义基础 URL 以使用兼容的 API：

```rust
let config = OpenAiEmbeddingsConfig::new("sk-...")
    .with_base_url("https://my-proxy.example.com/v1");
```

## OllamaEmbeddings

使用本地 Ollama 实例进行嵌入。无需 API 密钥 -- 只需指定模型名称。

```rust
use std::sync::Arc;
use synaptic::embeddings::{OllamaEmbeddings, OllamaEmbeddingsConfig};
use synaptic::embeddings::Embeddings;
use synaptic::models::backend::HttpBackend;

let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
// Default base_url: http://localhost:11434

let backend = Arc::new(HttpBackend::new());
let embeddings = OllamaEmbeddings::new(config, backend);

let vector = embeddings.embed_query("search query").await?;
```

自定义 Ollama 端点：

```rust
let config = OllamaEmbeddingsConfig::new("nomic-embed-text")
    .with_base_url("http://my-ollama:11434");
```

## CacheBackedEmbeddings

用 Store 后端缓存封装任意 `Embeddings` 提供商。之前计算过的 Embeddings 从缓存返回；只有未缓存的文本才会发送到底层提供商。

构造函数接受三个参数：底层 Embeddings 提供商、Store 后端、命名空间前缀。

```rust
use std::sync::Arc;
use synaptic::embeddings::{CacheBackedEmbeddings, FakeEmbeddings, Embeddings};
use synaptic::store::InMemoryStore;

let inner = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryStore::new());
let cached = CacheBackedEmbeddings::new(inner, store, "fake");

// First call computes the embedding
let v1 = cached.embed_query("hello").await?;

// Second call returns the cached result -- no recomputation
let v2 = cached.embed_query("hello").await?;

assert_eq!(v1, v2);
```

使用 OpenAI Embeddings 的完整示例：

```rust
use synaptic::embeddings::CacheBackedEmbeddings;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let embeddings = Arc::new(OpenAiEmbeddings::new());
let store = Arc::new(InMemoryStore::new());
let cached = CacheBackedEmbeddings::new(embeddings, store, "openai");
```

当向 VectorStore 添加文档然后查询时，这特别有用，因为相同的文本可能会在多次操作中被重复嵌入。

> **持久化后端**：`InMemoryStore` 适合开发和测试。在生产环境中，可以使用 `SqliteStore`、`PgStore` 或 `RedisStore` 作为持久化后端，这样缓存在进程重启后仍然有效。

## 将 Embeddings 与 VectorStore 配合使用

Embeddings 作为参数传递给 VectorStore 方法，而不是存储在 VectorStore 内部。这种设计让你可以在不重建 Store 的情况下更换 Embeddings 提供商。

```rust
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::retrieval::Document;

let embeddings = FakeEmbeddings::new(128);
let store = InMemoryVectorStore::new();

let docs = vec![Document::new("1", "Rust is fast")];
store.add_documents(docs, &embeddings).await?;

let results = store.similarity_search("fast language", 5, &embeddings).await?;
```
