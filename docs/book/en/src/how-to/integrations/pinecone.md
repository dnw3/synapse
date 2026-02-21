# Pinecone Vector Store

This guide shows how to use [Pinecone](https://www.pinecone.io/) as a vector store backend in Synaptic. Pinecone is a managed vector database built for real-time similarity search at scale.

## Setup

Add the `pinecone` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "pinecone"] }
```

Set your Pinecone API key:

```bash
export PINECONE_API_KEY="your-pinecone-api-key"
```

You also need an existing Pinecone index. Create one through the [Pinecone console](https://app.pinecone.io/) or the Pinecone API. Note the **index host URL** (e.g. `https://my-index-abc123.svc.aped-1234.pinecone.io`).

## Configuration

Create a `PineconeConfig` with your API key and index host URL:

```rust,ignore
use synaptic::pinecone::{PineconeConfig, PineconeVectorStore};

let config = PineconeConfig::new("your-pinecone-api-key", "https://my-index-abc123.svc.aped-1234.pinecone.io");
let store = PineconeVectorStore::new(config);
```

### Namespace

Pinecone supports namespaces for partitioning data within an index:

```rust,ignore
let config = PineconeConfig::new("api-key", "https://my-index.pinecone.io")
    .with_namespace("production");
```

If no namespace is set, the default namespace is used.

## Adding documents

`PineconeVectorStore` implements the `VectorStore` trait. Pass an embeddings provider to compute vectors:

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

Find the `k` most similar documents to a text query:

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

Wrap the store in a `VectorStoreRetriever` for use with Synaptic's retrieval infrastructure:

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
| `api_key` | `String` | required | Pinecone API key |
| `host` | `String` | required | Index host URL from the Pinecone console |
| `namespace` | `Option<String>` | `None` | Namespace for data partitioning |
