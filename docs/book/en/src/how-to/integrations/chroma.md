# Chroma Vector Store

This guide shows how to use [Chroma](https://www.trychroma.com/) as a vector store backend in Synaptic. Chroma is an open-source embedding database that runs locally or in the cloud.

## Setup

Add the `chroma` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "chroma"] }
```

Start a Chroma server (e.g. via Docker):

```bash
docker run -p 8000:8000 chromadb/chroma
```

## Configuration

Create a `ChromaConfig` with the server URL and collection name:

```rust,ignore
use synaptic::chroma::{ChromaConfig, ChromaVectorStore};

let config = ChromaConfig::new("http://localhost:8000", "my_collection");
let store = ChromaVectorStore::new(config);
```

The default URL is `http://localhost:8000`.

### Creating the collection

Call `ensure_collection()` to create the collection if it does not already exist. This is idempotent and safe to call on every startup:

```rust,ignore
store.ensure_collection().await?;
```

### Authentication

If your Chroma server requires authentication, pass credentials:

```rust,ignore
let config = ChromaConfig::new("https://chroma.example.com", "my_collection")
    .with_auth_token("your-token");
```

## Adding documents

`ChromaVectorStore` implements the `VectorStore` trait:

```rust,ignore
use synaptic::core::{VectorStore, Document, Embeddings};
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
    Document::new("3", "Go is designed for concurrency"),
];

let ids = store.add_documents(docs, &embeddings).await?;
```

## Similarity search

Find the `k` most similar documents:

```rust,ignore
let results = store.similarity_search("fast systems language", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### Search with scores

```rust,ignore
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

## Deleting documents

Remove documents by their IDs:

```rust,ignore
store.delete(&["1", "3"]).await?;
```

## Using with a retriever

Wrap the store in a `VectorStoreRetriever`:

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::core::Retriever;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(store);

let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("fast language", 5).await?;
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | `String` | `"http://localhost:8000"` | Chroma server URL |
| `collection_name` | `String` | required | Name of the collection |
| `auth_token` | `Option<String>` | `None` | Authentication token |
