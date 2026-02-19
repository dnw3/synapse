# Vector Stores

本指南展示如何使用 Synaptic 的 `VectorStore` trait 和内置的 `InMemoryVectorStore` 来存储和搜索文档 Embeddings。

## 概述

`synaptic_vectorstores` 中的 `VectorStore` trait 提供了添加、搜索和删除文档的方法：

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add_documents(
        &self, docs: Vec<Document>, embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError>;

    async fn similarity_search(
        &self, query: &str, k: usize, embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapticError>;

    async fn similarity_search_with_score(
        &self, query: &str, k: usize, embeddings: &dyn Embeddings,
    ) -> Result<Vec<(Document, f32)>, SynapticError>;

    async fn similarity_search_by_vector(
        &self, embedding: &[f32], k: usize,
    ) -> Result<Vec<Document>, SynapticError>;

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapticError>;
}
```

`embeddings` 参数传递给每个方法，而不是存储在 VectorStore 内部。这种设计让你可以在不重建 Store 的情况下更换 Embeddings 提供商。

## InMemoryVectorStore

使用 cosine similarity 进行搜索的内存向量存储。底层由 `RwLock<HashMap>` 支持。

### 创建 Store

```rust
use synaptic_vectorstores::InMemoryVectorStore;

let store = InMemoryVectorStore::new();
```

### 添加文档

```rust
use synaptic_vectorstores::{InMemoryVectorStore, VectorStore};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::Document;

let store = InMemoryVectorStore::new();
let embeddings = FakeEmbeddings::new(128);

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
    Document::new("3", "Go is designed for concurrency"),
];

let ids = store.add_documents(docs, &embeddings).await?;
// ids == ["1", "2", "3"]
```

### 相似性搜索

查找与查询最相似的 `k` 个文档：

```rust
let results = store.similarity_search("fast systems language", 2, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### 带分数搜索

获取结果及其相似度分数（分数越高越相似）：

```rust
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### 按向量搜索

使用预计算的 Embeddings 向量而非文本查询进行搜索：

```rust
use synaptic_embeddings::Embeddings;

let query_vec = embeddings.embed_query("systems programming").await?;
let results = store.similarity_search_by_vector(&query_vec, 3).await?;
```

### 删除文档

```rust
store.delete(&["1", "3"]).await?;
```

## 便捷构造函数

创建预填充文档的 Store：

```rust
use synaptic_vectorstores::InMemoryVectorStore;
use synaptic_embeddings::FakeEmbeddings;

let embeddings = FakeEmbeddings::new(128);

// From (id, content) tuples
let store = InMemoryVectorStore::from_texts(
    vec![("1", "Rust is fast"), ("2", "Python is flexible")],
    &embeddings,
).await?;

// From Document values
let store = InMemoryVectorStore::from_documents(docs, &embeddings).await?;
```

## 最大边际相关性搜索 (MMR)

MMR 搜索在相关性和多样性之间取得平衡。`lambda_mult` 参数控制两者的权衡：

- `1.0` -- 纯相关性（等同于标准相似性搜索）
- `0.0` -- 最大多样性
- `0.5` -- 平衡（典型默认值）

```rust
let results = store.max_marginal_relevance_search(
    "programming language",
    3,        // k: number of results
    10,       // fetch_k: initial candidates to consider
    0.5,      // lambda_mult: relevance vs. diversity
    &embeddings,
).await?;
```

## VectorStoreRetriever

`VectorStoreRetriever` 将任意 `VectorStore` 桥接到 `Retriever` trait，使其与 Synaptic 其余的检索基础设施兼容。

```rust
use std::sync::Arc;
use synaptic_vectorstores::{InMemoryVectorStore, VectorStoreRetriever};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::Retriever;

let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::new());
// ... add documents to store ...

let retriever = VectorStoreRetriever::new(store, embeddings, 5);

let results = retriever.retrieve("query", 5).await?;
```

## MultiVectorRetriever

`MultiVectorRetriever` 在 VectorStore 中存储小的子块以实现精确检索，但返回它们所属的较大父文档。这让你兼得两方面的优势：小块用于精确的 Embeddings 搜索，完整文档用于 LLM 上下文。

```rust
use std::sync::Arc;
use synaptic_vectorstores::{InMemoryVectorStore, MultiVectorRetriever};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::{Document, Retriever};

let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::new());

let retriever = MultiVectorRetriever::new(store, embeddings, 3);

// Add parent documents with their child chunks
let parent = Document::new("parent-1", "Full article about Rust ownership...");
let children = vec![
    Document::new("child-1", "Ownership rules in Rust"),
    Document::new("child-2", "Borrowing and references"),
];

retriever.add_documents(parent, children).await?;

// Search finds child chunks but returns the parent
let results = retriever.retrieve("ownership", 1).await?;
assert_eq!(results[0].id, Some("parent-1".to_string()));
```

`id_key` 元数据字段将子文档链接到其父文档。默认值为 `"doc_id"`。

### 分数阈值过滤

设置最低相似度分数。只有达到阈值的文档才会被返回：

```rust
let retriever = VectorStoreRetriever::new(store, embeddings, 10)
    .with_score_threshold(0.7);

let results = retriever.retrieve("query", 10).await?;
// Only documents with cosine similarity >= 0.7 are included
```
