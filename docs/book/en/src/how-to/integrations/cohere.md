# Cohere Reranker

This guide shows how to use the Cohere Reranker in Synaptic. The reranker re-scores a list of documents by relevance to a query, improving retrieval quality when used as a second-stage filter.

> **Note:** For Cohere chat models and embeddings, use the [OpenAI-compatible constructors](openai-compatible.md) (`cohere_chat_model`, `cohere_embeddings`) instead. This page covers the **Reranker** only.

## Setup

Add the `cohere` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["cohere"] }
```

Set your Cohere API key:

```bash
export CO_API_KEY="your-cohere-api-key"
```

## Configuration

Create a `CohereRerankerConfig` and build the reranker:

```rust,ignore
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};

let config = CohereRerankerConfig::new("your-cohere-api-key");
let reranker = CohereReranker::new(config);
```

### Custom model

The default model is `"rerank-v3.5"`. You can specify a different one:

```rust,ignore
let config = CohereRerankerConfig::new("your-cohere-api-key")
    .with_model("rerank-english-v3.0");
```

## Usage

### Reranking documents

Pass a query, a list of documents, and the number of top results to return:

```rust,ignore
use synaptic::core::Document;

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is popular for data science"),
    Document::new("3", "Rust ensures memory safety without a garbage collector"),
    Document::new("4", "JavaScript runs in the browser"),
];

let top_docs = reranker.rerank("memory safe language", &docs, 2).await?;

for doc in &top_docs {
    println!("{}: {}", doc.id, doc.content);
}
// Likely returns docs 3 and 1, re-ordered by relevance
```

The returned documents are sorted by descending relevance score. Only the top `top_n` documents are returned.

## With ContextualCompressionRetriever

When the `retrieval` feature is also enabled, `CohereReranker` implements the `DocumentCompressor` trait. This allows it to plug into a `ContextualCompressionRetriever` for automatic reranking:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "cohere", "retrieval", "vectorstores", "embeddings"] }
```

```rust,ignore
use std::sync::Arc;
use synaptic::cohere::{CohereRerankerConfig, CohereReranker};
use synaptic::retrieval::ContextualCompressionRetriever;
use synaptic::vectorstores::{InMemoryVectorStore, VectorStoreRetriever};
use synaptic::openai::OpenAiEmbeddings;

// Set up a base retriever
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(InMemoryVectorStore::new());
// ... add documents to the store ...

let base_retriever = Arc::new(VectorStoreRetriever::new(store, embeddings, 20));

// Wrap with reranker for two-stage retrieval
let reranker = Arc::new(CohereReranker::new(
    CohereRerankerConfig::new("your-cohere-api-key"),
));

let retriever = ContextualCompressionRetriever::new(base_retriever, reranker);

// Retrieves 20 candidates, then reranks and returns the top 5
use synaptic::core::Retriever;
let results = retriever.retrieve("memory safety in Rust", 5).await?;
```

This two-stage pattern (broad retrieval followed by reranking) often produces better results than relying on embedding similarity alone.

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Cohere API key |
| `model` | `String` | `"rerank-v3.5"` | Reranker model name |
