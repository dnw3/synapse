# FlashRank (Local Reranker)

FlashRank is a fast, zero-dependency local reranker based on BM25 scoring. It runs entirely in-process with no external API calls, making it ideal for development, testing, and offline scenarios.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["flashrank"] }
```

No API key required. No external service needed.

## How It Works

FlashRank uses the Okapi BM25 algorithm (the same foundation as Elasticsearch's default ranking) to score documents against a query. It tokenizes both query and documents, computes term frequency with length normalization, and returns results sorted by relevance score.

**Pros:**
- Zero latency (no network calls)
- No API costs
- Works offline and in CI/CD environments
- Fully deterministic

**Cons:**
- Lexical matching only (no semantic understanding)
- No multilingual support beyond token overlap
- Lower precision than neural rerankers for complex queries

For production use cases requiring semantic understanding, consider [BGE Reranker](bge-reranker.md), [Voyage AI Reranker](voyage-reranker.md), or [Jina AI Reranker](jina.md).

## Usage

```rust,ignore
use synaptic::flashrank::{FlashRankConfig, FlashRankReranker};
use synaptic::core::Document;

let reranker = FlashRankReranker::new(FlashRankConfig::default());

let docs = vec![
    Document::new("doc1", "Paris is the capital of France and home to the Eiffel Tower."),
    Document::new("doc2", "Berlin is the capital of Germany."),
    Document::new("doc3", "The weather is sunny today."),
    Document::new("doc4", "France is a country in Western Europe."),
];

let results = reranker
    .rerank("capital of France", docs, 2)
    .await?;

for (doc, score) in &results {
    println!("{:.4}: {}", score, doc.content);
}
// Output:
// 0.6543: Paris is the capital of France and home to the Eiffel Tower.
// 0.2341: France is a country in Western Europe.
```

## Configuration

```rust,ignore
use synaptic::flashrank::FlashRankConfig;

// Use defaults (k1=1.5, b=0.75 — standard BM25 parameters)
let config = FlashRankConfig::default();

// Tune BM25 parameters
let config = FlashRankConfig::default()
    .with_k1(1.2)   // Term frequency saturation (lower = less sensitive to TF)
    .with_b(0.8);   // Length normalization (1.0 = full, 0.0 = none)
```

## RAG Pipeline Integration

FlashRank is excellent as a lightweight first-pass reranker or for development/testing:

```rust,ignore
use synaptic::flashrank::{FlashRankConfig, FlashRankReranker};
use synaptic::vectorstores::InMemoryVectorStore;

// Retrieve 20 candidates with vector search
let candidates = vector_store
    .similarity_search("capital of France", 20, &embeddings)
    .await?;

// Rerank locally using BM25
let reranker = FlashRankReranker::new(FlashRankConfig::default());
let top5 = reranker
    .rerank("capital of France", candidates, 5)
    .await?;
```

## Upgrading to Neural Reranking

When you're ready for higher precision, FlashRank and neural rerankers share the same API shape, making migration trivial:

```rust,ignore
// Development: local BM25 reranker
let reranker = synaptic::flashrank::FlashRankReranker::new(Default::default());

// Production: neural cross-encoder via HuggingFace
let reranker = synaptic::huggingface::reranker::HuggingFaceReranker::new("hf_token");

// Same call in both cases:
let results = reranker.rerank(query, docs, top_k).await?;
```

## Configuration Reference

| Parameter | Default | Description |
|-----------|---------|-------------|
| `k1` | `1.5` | BM25 term frequency saturation. Range: 1.2–2.0 for most use cases |
| `b` | `0.75` | BM25 length normalization. Range: 0.0 (none) to 1.0 (full) |
