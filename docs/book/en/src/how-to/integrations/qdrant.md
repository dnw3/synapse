# Qdrant Vector Store

This guide shows how to use [Qdrant](https://qdrant.tech/) as a vector store backend in Synaptic. Qdrant is a high-performance vector database purpose-built for similarity search.

## Setup

Add the `qdrant` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant"] }
```

Start a Qdrant instance (e.g. via Docker):

```bash
docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
```

Port 6333 is the REST API; port 6334 is the gRPC endpoint used by the Rust client.

## Configuration

Create a `QdrantConfig` with the connection URL, collection name, and vector dimensionality:

```rust,ignore
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};

let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536);
let store = QdrantVectorStore::new(config)?;
```

### API key authentication

For Qdrant Cloud or secured deployments, attach an API key:

```rust,ignore
let config = QdrantConfig::new("https://my-cluster.cloud.qdrant.io:6334", "docs", 1536)
    .with_api_key("your-api-key-here");

let store = QdrantVectorStore::new(config)?;
```

### Distance metric

The default distance metric is cosine similarity. You can change it with `with_distance()`:

```rust,ignore
use qdrant_client::qdrant::Distance;

let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536)
    .with_distance(Distance::Euclid);
```

Available options: `Distance::Cosine` (default), `Distance::Euclid`, `Distance::Dot`, `Distance::Manhattan`.

## Creating the collection

Call `ensure_collection()` to create the collection if it does not already exist. This is idempotent and safe to call on every startup:

```rust,ignore
store.ensure_collection().await?;
```

The collection is created with the vector size and distance metric from your config.

## Adding documents

`QdrantVectorStore` implements the `VectorStore` trait. Pass an embeddings provider to compute vectors:

```rust,ignore
use synaptic::qdrant::VectorStore;
use synaptic::retrieval::Document;
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
    Document::new("3", "Go is designed for concurrency"),
];

let ids = store.add_documents(docs, &embeddings).await?;
```

Document IDs are mapped to Qdrant point UUIDs. If a document ID is already a valid UUID, it is used directly. Otherwise, a deterministic UUID v5 is generated from the ID string.

## Similarity search

Find the `k` most similar documents to a text query:

```rust,ignore
let results = store.similarity_search("fast systems language", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### Search with scores

Get similarity scores alongside results:

```rust,ignore
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### Search by vector

Search using a pre-computed embedding vector:

```rust,ignore
use synaptic::embeddings::Embeddings;

let query_vec = embeddings.embed_query("systems programming").await?;
let results = store.similarity_search_by_vector(&query_vec, 3).await?;
```

## Deleting documents

Remove documents by their IDs:

```rust,ignore
store.delete(&["1", "3"]).await?;
```

## Using with a retriever

Wrap the store in a `VectorStoreRetriever` to use it with the rest of Synaptic's retrieval infrastructure:

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::retrieval::Retriever;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(store);

let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("fast language", 5).await?;
```

## Using an existing client

If you already have a configured `qdrant_client::Qdrant` instance, you can pass it directly:

```rust,ignore
use qdrant_client::Qdrant;
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};

let client = Qdrant::from_url("http://localhost:6334").build()?;
let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536);

let store = QdrantVectorStore::from_client(client, config);
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | `String` | required | Qdrant gRPC URL (e.g. `http://localhost:6334`) |
| `collection_name` | `String` | required | Name of the Qdrant collection |
| `vector_size` | `u64` | required | Dimensionality of the embedding vectors |
| `api_key` | `Option<String>` | `None` | API key for authenticated access |
| `distance` | `Distance` | `Cosine` | Distance metric for similarity search |
