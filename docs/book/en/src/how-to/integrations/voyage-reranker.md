# Voyage AI Reranker

Voyage AI's reranking models are high-quality cross-encoder rerankers that significantly improve retrieval precision. Built by the same team behind the top-ranked Voyage embeddings, they are optimized for RAG applications.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["voyage"] }
```

Sign up at [voyageai.com](https://www.voyageai.com/) and create an API key.

## Available Models

| Variant | API Model ID | Best For |
|---------|-------------|----------|
| `Rerank2` | `rerank-2` | General purpose (recommended) |
| `Rerank2Lite` | `rerank-2-lite` | Fast, cost-efficient |
| `Custom(String)` | _(any)_ | Unlisted models |

## Usage

```rust,ignore
use synaptic::voyage::reranker::{VoyageReranker, VoyageRerankerModel};
use synaptic::core::Document;

let reranker = VoyageReranker::new("pa-your-api-key")
    .with_model(VoyageRerankerModel::Rerank2);

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
// 0.9234: Paris is the capital of France.
// 0.6821: France is a country in Western Europe.
```

## RAG Pipeline Integration

```rust,ignore
use synaptic::voyage::reranker::VoyageReranker;
use synaptic::voyage::VoyageEmbeddings;
use synaptic::vectorstores::InMemoryVectorStore;

// Retrieve 20 candidates with fast vector search
let candidates = vector_store
    .similarity_search("capital of France", 20, &embeddings)
    .await?;

// Rerank to top 5 with Voyage cross-encoder
let reranker = VoyageReranker::new("pa-your-api-key");
let top5 = reranker
    .rerank("capital of France", candidates, 5)
    .await?;

// Use top5 as context for the LLM
```

## Custom Endpoint

Point to a custom or self-hosted deployment:

```rust,ignore
let reranker = VoyageReranker::new("pa-key")
    .with_base_url("https://custom.voyageai.com/v1");
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
| `api_key` | required | Voyage AI API key (`pa-...`) |
| `model` | `Rerank2` | Reranker model |
| `base_url` | Voyage AI URL | Override for custom deployments |
