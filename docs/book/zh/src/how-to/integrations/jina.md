# Jina AI

[Jina AI](https://jina.ai/) 提供高质量的嵌入和重排序器。`jina-embeddings-v3` 模型支持 8192 token 上下文，生成 1024 维嵌入向量。`JinaReranker` 提供交叉编码器重排序，以提高检索精度。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["jina"] }
```

在 [cloud.jina.ai](https://cloud.jina.ai/) 获取 API 密钥。

## 嵌入向量

```rust,ignore
use synaptic::jina::{JinaConfig, JinaEmbeddingModel, JinaEmbeddings};
use synaptic::core::Embeddings;

let config = JinaConfig::new("your-api-key", JinaEmbeddingModel::JinaEmbeddingsV3);
let embeddings = JinaEmbeddings::new(config);

let docs = embeddings.embed_documents(&["文档1", "文档2"]).await?;
let query_vec = embeddings.embed_query("搜索查询").await?;
```

## 重排序器

```rust,ignore
use synaptic::jina::reranker::{JinaReranker, JinaRerankerModel};
use synaptic::core::Document;

let reranker = JinaReranker::new("your-api-key")
    .with_model(JinaRerankerModel::JinaRerankerV2BaseMultilingual);

let docs = vec![
    Document::new("1", "Rust 是系统编程语言。"),
    Document::new("2", "Python 适合数据科学。"),
    Document::new("3", "Rust 保证内存安全。"),
];

let ranked = reranker.rerank("Rust 内存安全", docs, 2).await?;
for (doc, score) in &ranked {
    println!("得分 {:.3}：{}", score, doc.content);
}
```

## 可用模型

### 嵌入

| 变体 | 模型 ID | 上下文 |
|---|---|---|
| `JinaEmbeddingsV3` | `jina-embeddings-v3` | 8192 |
| `JinaEmbeddingsV2BaseEn` | `jina-embeddings-v2-base-en` | 8192 |

### 重排序器

| 变体 | 模型 ID | 语言 |
|---|---|---|
| `JinaRerankerV2BaseMultilingual` | `jina-reranker-v2-base-multilingual` | 多语言 |
| `JinaRerankerV1BaseEn` | `jina-reranker-v1-base-en` | 英语 |
