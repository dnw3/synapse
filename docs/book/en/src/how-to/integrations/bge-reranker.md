# BGE Reranker (HuggingFace)

BAAI's BGE reranker models are state-of-the-art cross-encoder rerankers available via the HuggingFace Inference API. They significantly outperform bi-encoder embedding similarity for document ranking, making them ideal for the final reranking stage in RAG pipelines.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["huggingface"] }
```

Sign up at [huggingface.co](https://huggingface.co/) and create an access token under Settings → Access Tokens.

## Available Models

| Variant | HF Model ID | Context | Best For |
|---------|-------------|---------|----------|
| `BgeRerankerV2M3` | `BAAI/bge-reranker-v2-m3` | 512 tokens | Multilingual (recommended) |
| `BgeRerankerLarge` | `BAAI/bge-reranker-large` | 512 tokens | Highest quality (English) |
| `BgeRerankerBase` | `BAAI/bge-reranker-base` | 512 tokens | Fast, good quality (English) |
| `Custom(String)` | _(any)_ | — | Unlisted models |

## Usage

```rust,ignore
use synaptic::huggingface::reranker::{BgeRerankerModel, HuggingFaceReranker};
use synaptic::core::Document;

let reranker = HuggingFaceReranker::new("hf_your_access_token")
    .with_model(BgeRerankerModel::BgeRerankerV2M3);

let docs = vec![
    Document::new("doc1", "Paris is the capital of France."),
    Document::new("doc2", "The Eiffel Tower is in Paris."),
    Document::new("doc3", "Berlin is the capital of Germany."),
    Document::new("doc4", "France is a country in Western Europe."),
];

let results = reranker
    .rerank("What is the capital of France?", docs, 2)
    .await?;

for (doc, score) in &results {
    println!("{:.4}: {}", score, doc.content);
}
// Output (example):
// 0.9876: Paris is the capital of France.
// 0.7543: France is a country in Western Europe.
```

## RAG Pipeline Integration

Use the BGE reranker to improve retrieval quality by reranking a large candidate set to a small high-precision set:

```rust,ignore
use synaptic::huggingface::reranker::HuggingFaceReranker;
use synaptic::vectorstores::InMemoryVectorStore;
use synaptic::core::Document;

// Retrieve 20 candidates with fast vector search
let candidates = vector_store
    .similarity_search("capital of France", 20, &embeddings)
    .await?;

// Rerank to top 5 with cross-encoder
let reranker = HuggingFaceReranker::new("hf_token");
let top5 = reranker
    .rerank("capital of France", candidates, 5)
    .await?;

// Use top5 as context for the LLM
```

## Error Handling

```rust,ignore
use synaptic::core::SynapticError;

match reranker.rerank(query, docs, k).await {
    Ok(results) => {
        for (doc, score) in results {
            println!("{:.4}: {}", score, doc.content);
        }
    }
    Err(SynapticError::Retriever(msg)) => eprintln!("Rerank error: {}", msg),
    Err(e) => return Err(e.into()),
}
```

## Configuration Reference

| Parameter | Default | Description |
|-----------|---------|-------------|
| `api_key` | required | HuggingFace access token (`hf_...`) |
| `model` | `BgeRerankerV2M3` | Reranker model |
| `base_url` | HF inference URL | Override for custom deployments |
