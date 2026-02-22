# Voyage AI

[Voyage AI](https://www.voyageai.com/) 提供专为检索和 RAG 流水线优化的顶级文本嵌入。`voyage-3-large` 模型在 MTEB 排行榜中始终位居前列。Voyage 还提供面向代码和金融领域的专用模型。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["voyage"] }
```

在 [dash.voyageai.com](https://dash.voyageai.com/) 获取 API 密钥。

## 使用示例

```rust,ignore
use synaptic::voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};
use synaptic::core::Embeddings;

let config = VoyageConfig::new("your-api-key", VoyageModel::Voyage3Large);
let embeddings = VoyageEmbeddings::new(config);

// 嵌入文档用于 RAG
let docs = embeddings.embed_documents(&["Rust 很快。", "内存安全很重要。"]).await?;

// 嵌入查询
let query_vec = embeddings.embed_query("什么是 Rust？").await?;
```

## 可用模型

| 枚举变体 | API 模型 ID | 维度 | 适用场景 |
|---|---|---|---|
| `Voyage3Large` | `voyage-3-large` | 1024 | 最高质量（推荐） |
| `Voyage3` | `voyage-3` | 1024 | 均衡质量/速度 |
| `Voyage3Lite` | `voyage-3-lite` | 512 | 最快、最便宜 |
| `VoyageCode3` | `voyage-code-3` | 1024 | 代码检索 |
| `VoyageFinance2` | `voyage-finance-2` | 1024 | 金融文档 |

## 配合向量数据库使用

```rust,ignore
use synaptic::voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};
use synaptic::vectorstores::InMemoryVectorStore;
use synaptic::core::{Document, VectorStore};

let config = VoyageConfig::new("your-api-key", VoyageModel::Voyage3);
let embeddings = VoyageEmbeddings::new(config);
let store = InMemoryVectorStore::new();

let docs = vec![
    Document::new("doc-1", "Rust 在没有垃圾回收的情况下提供内存安全。"),
    Document::new("doc-2", "零成本抽象实现高性能。"),
];

store.add_documents(docs, &embeddings).await?;
let results = store.similarity_search("内存安全", 2, &embeddings).await?;
```
