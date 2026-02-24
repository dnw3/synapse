# Embeddings

This guide shows how to convert text into vector representations using Synaptic's `Embeddings` trait and its built-in providers.

## Overview

All embedding providers implement the `Embeddings` trait from `synaptic_embeddings`:

```rust
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError>;
}
```

- `embed_documents()` embeds multiple texts in a single batch -- use this for indexing.
- `embed_query()` embeds a single query text -- use this at retrieval time.

## FakeEmbeddings

Generates deterministic vectors based on a simple hash of the input text. Useful for testing and development without API calls.

```rust
use synaptic::embeddings::FakeEmbeddings;
use synaptic::embeddings::Embeddings;

// Specify the number of dimensions (default is 4)
let embeddings = FakeEmbeddings::new(4);

let doc_vectors = embeddings.embed_documents(&["doc one", "doc two"]).await?;
let query_vector = embeddings.embed_query("search query").await?;

// Vectors are normalized to unit length
// Similar texts produce similar vectors
```

## OpenAiEmbeddings

Uses the OpenAI embeddings API. Requires an API key and a `ProviderBackend`.

```rust
use std::sync::Arc;
use synaptic::embeddings::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};
use synaptic::embeddings::Embeddings;
use synaptic::models::backend::HttpBackend;

let config = OpenAiEmbeddingsConfig::new("sk-...")
    .with_model("text-embedding-3-small");  // default model

let backend = Arc::new(HttpBackend::new());
let embeddings = OpenAiEmbeddings::new(config, backend);

let vectors = embeddings.embed_documents(&["hello world"]).await?;
```

You can customize the base URL for compatible APIs:

```rust
let config = OpenAiEmbeddingsConfig::new("sk-...")
    .with_base_url("https://my-proxy.example.com/v1");
```

## OllamaEmbeddings

Uses a local Ollama instance for embedding. No API key required -- just specify the model name.

```rust
use std::sync::Arc;
use synaptic::embeddings::{OllamaEmbeddings, OllamaEmbeddingsConfig};
use synaptic::embeddings::Embeddings;
use synaptic::models::backend::HttpBackend;

let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
// Default base_url: http://localhost:11434

let backend = Arc::new(HttpBackend::new());
let embeddings = OllamaEmbeddings::new(config, backend);

let vector = embeddings.embed_query("search query").await?;
```

Custom Ollama endpoint:

```rust
let config = OllamaEmbeddingsConfig::new("nomic-embed-text")
    .with_base_url("http://my-ollama:11434");
```

## CacheBackedEmbeddings

Wraps any `Embeddings` provider with a `Store`-backed cache. Previously computed embeddings are returned from the store; only uncached texts are sent to the underlying provider. The third argument is a namespace prefix that isolates cache entries by provider or model.

```rust
use std::sync::Arc;
use synaptic::embeddings::{CacheBackedEmbeddings, FakeEmbeddings, Embeddings};
use synaptic::store::InMemoryStore;

let inner = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryStore::new());
let cached = CacheBackedEmbeddings::new(inner, store, "fake");

// First call computes the embedding
let v1 = cached.embed_query("hello").await?;

// Second call returns the cached result -- no recomputation
let v2 = cached.embed_query("hello").await?;

assert_eq!(v1, v2);
```

This is especially useful when adding documents to a vector store and then querying, since the same text may be embedded multiple times across operations.

### With OpenAI embeddings

```rust
use synaptic::embeddings::CacheBackedEmbeddings;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let embeddings = Arc::new(OpenAiEmbeddings::new());
let store = Arc::new(InMemoryStore::new());
let cached = CacheBackedEmbeddings::new(embeddings, store, "openai");
```

### Persistent backends

For production use, swap `InMemoryStore` with a persistent backend so cached embeddings survive process restarts:

- **SqliteStore** -- single-machine persistence (requires `sqlite` feature)
- **PgStore** -- PostgreSQL-backed persistence (requires `postgres` feature)
- **RedisStore** -- distributed persistence (requires `redis` feature)

## Using embeddings with vector stores

Embeddings are passed to vector store methods rather than stored inside the vector store. This lets you swap embedding providers without rebuilding the store.

```rust
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::retrieval::Document;

let embeddings = FakeEmbeddings::new(128);
let store = InMemoryVectorStore::new();

let docs = vec![Document::new("1", "Rust is fast")];
store.add_documents(docs, &embeddings).await?;

let results = store.similarity_search("fast language", 5, &embeddings).await?;
```
