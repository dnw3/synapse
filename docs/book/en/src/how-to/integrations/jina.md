# Jina AI

[Jina AI](https://jina.ai/) provides high-quality embeddings and rerankers. The `jina-embeddings-v3` model supports 8192-token contexts and produces 1024-dimensional embeddings. The `JinaReranker` provides cross-encoder reranking to improve retrieval precision.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["jina"] }
```

Get an API key from [cloud.jina.ai](https://cloud.jina.ai/).

## Embeddings

```rust,ignore
use synaptic::jina::{JinaConfig, JinaEmbeddingModel, JinaEmbeddings};
use synaptic::core::Embeddings;

let config = JinaConfig::new("your-api-key", JinaEmbeddingModel::JinaEmbeddingsV3);
let embeddings = JinaEmbeddings::new(config);

let docs = embeddings.embed_documents(&["Document 1", "Document 2"]).await?;
let query_vec = embeddings.embed_query("search query").await?;
```

## Reranker

```rust,ignore
use synaptic::jina::reranker::{JinaReranker, JinaRerankerModel};
use synaptic::core::Document;

let reranker = JinaReranker::new("your-api-key")
    .with_model(JinaRerankerModel::JinaRerankerV2BaseMultilingual);

let docs = vec![
    Document::new("1", "Rust is a systems programming language."),
    Document::new("2", "Python is great for data science."),
    Document::new("3", "Rust ensures memory safety."),
];

let ranked = reranker.rerank("Rust memory safety", docs, 2).await?;
for (doc, score) in &ranked {
    println!("Score {:.3}: {}", score, doc.content);
}
```

## Available Models

### Embeddings

| Variant | Model ID | Context |
|---|---|---|
| `JinaEmbeddingsV3` | `jina-embeddings-v3` | 8192 |
| `JinaEmbeddingsV2BaseEn` | `jina-embeddings-v2-base-en` | 8192 |

### Reranker

| Variant | Model ID | Language |
|---|---|---|
| `JinaRerankerV2BaseMultilingual` | `jina-reranker-v2-base-multilingual` | Multilingual |
| `JinaRerankerV1BaseEn` | `jina-reranker-v1-base-en` | English |
