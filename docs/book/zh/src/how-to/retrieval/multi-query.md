# Multi-Query Retriever

本指南展示如何使用 `MultiQueryRetriever` 通过 LLM 生成多个查询视角来提高检索召回率。

## 概述

单个搜索查询可能无法捕获所有相关文档，特别是当用户的措辞与文档语料库中的词汇不匹配时。`MultiQueryRetriever` 通过以下方式解决这个问题：

1. 使用 `ChatModel` 生成原始查询的替代表述。
2. 将每个查询变体通过基础 Retriever 进行检索。
3. 去重并合并结果。

该技术通过克服基于距离的相似性搜索的局限性来提高召回率。

## 基本用法

```rust
use std::sync::Arc;
use synaptic_retrieval::{MultiQueryRetriever, Retriever};

let base_retriever: Arc<dyn Retriever> = Arc::new(/* any retriever */);
let model: Arc<dyn ChatModel> = Arc::new(/* any ChatModel */);

// Default: generates 3 query variants
let retriever = MultiQueryRetriever::new(base_retriever, model);

let results = retriever.retrieve("What are the benefits of Rust?", 5).await?;
```

当你调用 `retrieve()` 时，Retriever 会：

1. 向 LLM 发送提示，要求将查询改写为 3 个不同的版本。
2. 将原始查询加上所有生成的变体通过基础 Retriever 进行检索。
3. 收集所有结果，按文档 `id` 去重，返回最多 `top_k` 个文档。

## 自定义查询变体数量

指定不同数量的生成查询：

```rust
let retriever = MultiQueryRetriever::with_num_queries(
    base_retriever,
    model,
    5,  // Generate 5 query variants
);
```

更多变体会提高召回率，但也会增加 LLM 和 Retriever 的调用次数。

## 内部工作原理

Retriever 向 LLM 发送如下提示：

> You are an AI language model assistant. Your task is to generate 3 different versions of the given user question to retrieve relevant documents from a vector database. By generating multiple perspectives on the user question, your goal is to help the user overcome some of the limitations of distance-based similarity search. Provide these alternative questions separated by newlines. Only output the questions, nothing else.
>
> Original question: What are the benefits of Rust?

LLM 可能的回复：

```
Why should I use Rust as a programming language?
What advantages does Rust offer over other languages?
What makes Rust a good choice for software development?
```

每个查询随后通过基础 Retriever 进行检索，所有结果经过去重后合并。

## 结合 VectorStore 的示例

```rust
use std::sync::Arc;
use synaptic_retrieval::{MultiQueryRetriever, Retriever};
use synaptic_vectorstores::{InMemoryVectorStore, VectorStoreRetriever, VectorStore};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::Document;

// Set up vector store
let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::new());

let docs = vec![
    Document::new("1", "Rust ensures memory safety without a garbage collector"),
    Document::new("2", "Rust's ownership system prevents data races at compile time"),
    Document::new("3", "Go uses goroutines for lightweight concurrency"),
];
store.add_documents(docs, embeddings.as_ref()).await?;

// Wrap vector store as a retriever
let base = Arc::new(VectorStoreRetriever::new(store, embeddings, 5));

// Create multi-query retriever
let model: Arc<dyn ChatModel> = Arc::new(/* your model */);
let retriever = MultiQueryRetriever::new(base, model);

let results = retriever.retrieve("Why is Rust safe?", 5).await?;
// May find documents that mention "memory safety", "ownership", "data races"
// even if the original query doesn't use those exact terms
```
