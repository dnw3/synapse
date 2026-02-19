# Ensemble Retriever

本指南展示如何使用 `EnsembleRetriever` 和 Reciprocal Rank Fusion (RRF) 组合多个 Retriever。

## 概述

不同的检索策略各有优势。基于关键词的方法（如 BM25）擅长精确的词语匹配，而基于向量的方法能捕捉语义相似性。`EnsembleRetriever` 将多个 Retriever 的结果合并为一个排序列表，让你同时获得两种方法的优势。

它使用 **Reciprocal Rank Fusion (RRF)** 来合并排名。每个 Retriever 根据文档在其结果中的排名贡献一个加权 RRF 分数。然后文档按总 RRF 分数排序。

## 基本用法

```rust
use std::sync::Arc;
use synaptic_retrieval::{EnsembleRetriever, Retriever};

let retriever_a: Arc<dyn Retriever> = Arc::new(/* vector retriever */);
let retriever_b: Arc<dyn Retriever> = Arc::new(/* BM25 retriever */);

let ensemble = EnsembleRetriever::new(vec![
    (retriever_a, 0.5),  // weight 0.5
    (retriever_b, 0.5),  // weight 0.5
]);

let results = ensemble.retrieve("query", 5).await?;
```

每个元组包含一个 Retriever 和它的权重。权重缩放该 Retriever 的 RRF 分数贡献。

## 组合向量搜索与 BM25

最常见的使用场景是将语义（向量）搜索与关键词（BM25）搜索组合：

```rust
use std::sync::Arc;
use synaptic_retrieval::{BM25Retriever, EnsembleRetriever, Document, Retriever};
use synaptic_vectorstores::{InMemoryVectorStore, VectorStoreRetriever, VectorStore};
use synaptic_embeddings::FakeEmbeddings;

let docs = vec![
    Document::new("1", "Rust provides memory safety through ownership"),
    Document::new("2", "Python has a large ecosystem for machine learning"),
    Document::new("3", "Rust's borrow checker prevents data races"),
    Document::new("4", "Go is designed for building scalable services"),
];

// BM25 retriever (keyword-based)
let bm25 = Arc::new(BM25Retriever::new(docs.clone()));

// Vector retriever (semantic)
let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::from_documents(docs, embeddings.as_ref()).await?);
let vector = Arc::new(VectorStoreRetriever::new(store, embeddings, 5));

// Combine with equal weights
let ensemble = EnsembleRetriever::new(vec![
    (vector as Arc<dyn Retriever>, 0.5),
    (bm25 as Arc<dyn Retriever>, 0.5),
]);

let results = ensemble.retrieve("Rust safety", 3).await?;
```

## 调整权重

权重控制每个 Retriever 对最终排名的贡献程度。权重越高，影响越大。

```rust
// Favor semantic search
let ensemble = EnsembleRetriever::new(vec![
    (vector_retriever, 0.7),
    (bm25_retriever, 0.3),
]);

// Favor keyword search
let ensemble = EnsembleRetriever::new(vec![
    (vector_retriever, 0.3),
    (bm25_retriever, 0.7),
]);
```

## Reciprocal Rank Fusion 的工作原理

对于 Retriever 返回的每个文档，RRF 计算一个分数：

```
rrf_score = weight / (k + rank)
```

其中：
- `weight` 是 Retriever 配置的权重。
- `k` 是一个常数（60，标准 RRF 常数），防止排名靠前的文档过于主导。
- `rank` 是文档在 Retriever 结果中基于 1 的位置。

如果一个文档出现在多个 Retriever 的结果中，其 RRF 分数会被累加。最终结果按总 RRF 分数降序排列。

## 组合两个以上的 Retriever

你可以组合任意数量的 Retriever：

```rust
let ensemble = EnsembleRetriever::new(vec![
    (vector_retriever, 0.4),
    (bm25_retriever, 0.3),
    (multi_query_retriever, 0.3),
]);

let results = ensemble.retrieve("query", 10).await?;
```
