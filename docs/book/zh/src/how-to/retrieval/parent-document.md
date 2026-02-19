# 父文档检索器

本指南介绍如何使用 `ParentDocumentRetriever`，在小块文本上进行精确搜索，同时返回完整的父文档以提供上下文。

## 问题背景

在拆分文档用于检索时，你面临一个权衡：

- **小块文本** 更有利于搜索精度 -- 由于噪声更少，它们能更准确地匹配查询。
- **大型文档** 更有利于上下文 -- 它们为 LLM 生成答案时提供了更多信息。

`ParentDocumentRetriever` 通过同时维护两者来解决这个问题：它将父文档拆分为小的子块用于索引，但当子块匹配查询时，返回的是完整的父文档。

## 工作原理

1. 你提供父文档和拆分函数。
2. 检索器将每个父文档拆分为子块，并存储子块到父文档的映射关系。
3. 子块在子检索器中被索引（例如，基于向量存储）。
4. 在检索时，子检索器找到匹配的块，然后父文档检索器将这些块映射回其父文档，并在过程中进行去重。

## 基本用法

```rust
use std::sync::Arc;
use synaptic_retrieval::{ParentDocumentRetriever, Document, Retriever};
use synaptic_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

// 创建子检索器（任意 Retriever 实现）
let child_retriever: Arc<dyn Retriever> = Arc::new(/* vector store retriever */);

// 使用拆分函数创建父文档检索器
let splitter = RecursiveCharacterTextSplitter::new(200);
let parent_retriever = ParentDocumentRetriever::new(
    child_retriever.clone(),
    move |text: &str| splitter.split_text(text),
);
```

## 添加文档

`add_documents()` 方法将父文档拆分为子块并存储映射关系。它返回子文档，以便你在子检索器中对其建立索引。

```rust
let parent_docs = vec![
    Document::new("doc-1", "A very long document about Rust ownership..."),
    Document::new("doc-2", "A detailed guide to async programming in Rust..."),
];

// 将父文档拆分为子块并获取子文档用于索引
let child_docs = parent_retriever.add_documents(parent_docs).await;

// 在向量存储中索引子文档
// child_docs[0].id == "doc-1-child-0"
// child_docs[0].metadata["parent_id"] == "doc-1"
// child_docs[0].metadata["chunk_index"] == 0
```

每个子文档：
- ID 格式为 `"{parent_id}-child-{index}"`。
- 继承父文档的所有元数据。
- 额外包含 `parent_id` 和 `chunk_index` 元数据字段。

## 检索

当你调用 `retrieve()` 时，检索器搜索匹配的子块，然后返回对应的父文档：

```rust
let results = parent_retriever.retrieve("ownership borrowing", 3).await?;
// 返回完整的父文档，而非单个子块
```

检索器内部会获取 `top_k * 3` 个子结果，以确保在去重后能组装出足够的父文档。

## 完整示例

```rust
use std::sync::Arc;
use synaptic_retrieval::{ParentDocumentRetriever, Document, Retriever};
use synaptic_vectorstores::{InMemoryVectorStore, VectorStoreRetriever, VectorStore};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

// 为子块设置嵌入和向量存储
let embeddings = Arc::new(FakeEmbeddings::new(128));
let child_store = Arc::new(InMemoryVectorStore::new());

// 创建子检索器
let child_retriever = Arc::new(VectorStoreRetriever::new(
    child_store.clone(),
    embeddings.clone(),
    10,
));

// 使用较小的块大小为子块创建父文档检索器
let splitter = RecursiveCharacterTextSplitter::new(200);
let parent_retriever = ParentDocumentRetriever::new(
    child_retriever,
    move |text: &str| splitter.split_text(text),
);

// 添加父文档
let parents = vec![
    Document::new("rust-guide", "A comprehensive guide to Rust. \
        Rust is a systems programming language focused on safety, speed, and concurrency. \
        It achieves memory safety without garbage collection through its ownership system. \
        The borrow checker enforces ownership rules at compile time..."),
    Document::new("go-guide", "A comprehensive guide to Go. \
        Go is a statically typed language designed at Google. \
        It features goroutines for lightweight concurrency. \
        Go's garbage collector manages memory automatically..."),
];

let children = parent_retriever.add_documents(parents).await;

// 在向量存储中索引子文档
child_store.add_documents(children, embeddings.as_ref()).await?;

// 搜索子块，返回完整的父文档
let results = parent_retriever.retrieve("memory safety ownership", 2).await?;
// 返回完整的 "rust-guide" 父文档，即使只有
// 关于 ownership 的小块匹配了查询
```

## 何时使用

`ParentDocumentRetriever` 在以下场景最为有用：

- 你的文档很长且涵盖多个主题，但你需要精确的检索。
- 你需要 LLM 看到完整的文档上下文以生成高质量的答案。
- 仅使用小块文本会丢失重要的周围上下文。

对于块本身已足够独立的简单用例，标准的 `VectorStoreRetriever` 可能就足够了。
