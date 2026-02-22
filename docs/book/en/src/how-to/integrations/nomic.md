# Nomic AI

[Nomic AI](https://www.nomic.ai/) provides open-weight embedding models with a free API tier. The `nomic-embed-text-v1.5` model supports 8192-token context windows and offers task-type-specific encoding for search, classification, and clustering.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["nomic"] }
```

Get a free API key at [atlas.nomic.ai](https://atlas.nomic.ai/).

## Usage

```rust,ignore
use synaptic::nomic::{NomicConfig, NomicEmbeddings};
use synaptic::core::Embeddings;

let config = NomicConfig::new("your-api-key");
let embeddings = NomicEmbeddings::new(config);

let docs = embeddings.embed_documents(&["Long document text...", "Another document."]).await?;
let query_vec = embeddings.embed_query("search query").await?;
```

## Models

| Enum Variant | API Model ID | Context | Notes |
|---|---|---|---|
| `NomicEmbedTextV1_5` | `nomic-embed-text-v1.5` | 8192 tokens | Default, best quality |
| `NomicEmbedTextV1` | `nomic-embed-text-v1` | 2048 tokens | Older generation |

## Task Types

Nomic uses task-type specific encoding. `embed_documents()` uses `search_document` and `embed_query()` uses `search_query` automatically.
