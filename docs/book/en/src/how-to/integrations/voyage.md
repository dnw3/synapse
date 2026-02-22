# Voyage AI

[Voyage AI](https://www.voyageai.com/) provides state-of-the-art text embeddings optimized for retrieval and RAG pipelines. The `voyage-3-large` model consistently ranks in the top tier of the MTEB leaderboard. Voyage also offers domain-specific models for code and finance.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["voyage"] }
```

Get an API key from [dash.voyageai.com](https://dash.voyageai.com/).

## Usage

```rust,ignore
use synaptic::voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};
use synaptic::core::Embeddings;

let config = VoyageConfig::new("your-api-key", VoyageModel::Voyage3Large);
let embeddings = VoyageEmbeddings::new(config);

// Embed documents for RAG
let docs = embeddings.embed_documents(&["Rust is fast.", "Memory safety matters."]).await?;

// Embed a query
let query_vec = embeddings.embed_query("What is Rust?").await?;
```

## Available Models

| Enum Variant | API Model ID | Dimensions | Best For |
|---|---|---|---|
| `Voyage3Large` | `voyage-3-large` | 1024 | Best quality (recommended) |
| `Voyage3` | `voyage-3` | 1024 | Balanced quality/speed |
| `Voyage3Lite` | `voyage-3-lite` | 512 | Fastest, cheapest |
| `VoyageCode3` | `voyage-code-3` | 1024 | Code retrieval |
| `VoyageFinance2` | `voyage-finance-2` | 1024 | Finance documents |

## With Vector Store

```rust,ignore
use synaptic::voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};
use synaptic::vectorstores::InMemoryVectorStore;
use synaptic::core::{Document, VectorStore};

let config = VoyageConfig::new("your-api-key", VoyageModel::Voyage3);
let embeddings = VoyageEmbeddings::new(config);
let store = InMemoryVectorStore::new();

let docs = vec![
    Document::new("doc-1", "Rust provides memory safety without garbage collection."),
    Document::new("doc-2", "Zero-cost abstractions enable high performance."),
];

store.add_documents(docs, &embeddings).await?;
let results = store.similarity_search("memory safety", 2, &embeddings).await?;
```
