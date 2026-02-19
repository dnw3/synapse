# Contextual Compression

本指南展示如何使用 `ContextualCompressionRetriever` 和 `EmbeddingsFilter` 对检索到的文档进行后过滤。

## 概述

基础 Retriever 可能会返回与查询仅松散相关的文档。Contextual Compression 添加了第二步过滤：检索之后，`DocumentCompressor` 根据查询评估每个文档，并移除不满足相关性阈值的文档。

当你的基础 Retriever 检索范围较广（高召回率）而你希望收紧结果（高精确率）时，这特别有用。

## DocumentCompressor trait

过滤逻辑由 `DocumentCompressor` trait 定义：

```rust
#[async_trait]
pub trait DocumentCompressor: Send + Sync {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
    ) -> Result<Vec<Document>, SynapticError>;
}
```

Synaptic 提供了 `EmbeddingsFilter` 作为内置的压缩器。

## EmbeddingsFilter

通过计算查询 Embeddings 与每个文档内容 Embeddings 之间的 cosine similarity 来过滤文档。只有达到或超过相似度阈值的文档才会被保留。

```rust
use std::sync::Arc;
use synaptic_retrieval::EmbeddingsFilter;
use synaptic_embeddings::FakeEmbeddings;

let embeddings = Arc::new(FakeEmbeddings::new(128));

// Only keep documents with similarity >= 0.7
let filter = EmbeddingsFilter::new(embeddings, 0.7);
```

便捷构造函数使用默认阈值 `0.75`：

```rust
let filter = EmbeddingsFilter::with_default_threshold(embeddings);
```

## ContextualCompressionRetriever

封装基础 Retriever 并对结果应用 `DocumentCompressor`：

```rust
use std::sync::Arc;
use synaptic_retrieval::{
    ContextualCompressionRetriever,
    EmbeddingsFilter,
    Retriever,
};
use synaptic_embeddings::FakeEmbeddings;

let embeddings = Arc::new(FakeEmbeddings::new(128));
let base_retriever: Arc<dyn Retriever> = Arc::new(/* any retriever */);

// Create the filter
let filter = Arc::new(EmbeddingsFilter::new(embeddings, 0.7));

// Wrap the base retriever with compression
let retriever = ContextualCompressionRetriever::new(base_retriever, filter);

let results = retriever.retrieve("query", 5).await?;
// Only documents with cosine similarity >= 0.7 to the query are returned
```

## 完整示例

```rust
use std::sync::Arc;
use synaptic_retrieval::{
    BM25Retriever,
    ContextualCompressionRetriever,
    EmbeddingsFilter,
    Document,
    Retriever,
};
use synaptic_embeddings::FakeEmbeddings;

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "The weather today is sunny and warm"),
    Document::new("3", "Rust provides memory safety guarantees"),
    Document::new("4", "Cooking pasta requires boiling water"),
];

// BM25 might return loosely relevant results
let base = Arc::new(BM25Retriever::new(docs));

// Use embedding similarity to filter out irrelevant documents
let embeddings = Arc::new(FakeEmbeddings::new(128));
let filter = Arc::new(EmbeddingsFilter::new(embeddings, 0.6));
let retriever = ContextualCompressionRetriever::new(base, filter);

let results = retriever.retrieve("Rust programming", 5).await?;
// Documents about weather and cooking are filtered out
```

## 工作原理

1. `ContextualCompressionRetriever` 调用 `base.retrieve(query, top_k)` 获取候选文档。
2. 将这些候选文档传递给 `DocumentCompressor`（例如 `EmbeddingsFilter`）。
3. 压缩器对查询和所有候选文档计算 Embeddings，计算 cosine similarity，并移除低于阈值的文档。
4. 返回过滤后的结果。

## 自定义压缩器

你可以实现自己的 `DocumentCompressor` 来使用其他过滤策略 -- 例如，使用 LLM 判断相关性，或从每个文档中提取最相关的段落。

```rust
use async_trait::async_trait;
use synaptic_retrieval::{DocumentCompressor, Document};
use synaptic_core::SynapticError;

struct MyCompressor;

#[async_trait]
impl DocumentCompressor for MyCompressor {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
    ) -> Result<Vec<Document>, SynapticError> {
        // Your filtering logic here
        Ok(documents)
    }
}
```
