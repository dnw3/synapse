# MongoDB Atlas Vector Search

This guide shows how to use [MongoDB Atlas Vector Search](https://www.mongodb.com/products/platform/atlas-vector-search) as a vector store backend in Synaptic. Atlas Vector Search enables semantic similarity search on data stored in MongoDB.

## Setup

Add the `mongodb` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "mongodb"] }
```

### Prerequisites

1. A MongoDB Atlas cluster (M10 or higher, or a free shared cluster with Atlas Search enabled).
2. A **vector search index** configured on the target collection. Create one via the Atlas UI or the Atlas Admin API.

Example index definition (JSON):

```json
{
  "type": "vectorSearch",
  "fields": [
    {
      "type": "vector",
      "path": "embedding",
      "numDimensions": 1536,
      "similarity": "cosine"
    }
  ]
}
```

## Configuration

Create a `MongoVectorConfig` with the database name, collection name, index name, and vector dimensionality:

```rust,ignore
use synaptic::mongodb::{MongoVectorConfig, MongoVectorStore};

let config = MongoVectorConfig::new("my_database", "my_collection", "vector_index", 1536);
let store = MongoVectorStore::from_uri("mongodb+srv://user:pass@cluster.mongodb.net/", config).await?;
```

The `from_uri` constructor connects to MongoDB and is async.

### Embedding field name

By default, vectors are stored in a field called `"embedding"`. You can change this:

```rust,ignore
let config = MongoVectorConfig::new("mydb", "docs", "vector_index", 1536)
    .with_embedding_field("vector");
```

Make sure this matches the `path` in your Atlas vector search index definition.

### Content and metadata fields

Customize which fields store the document content and metadata:

```rust,ignore
let config = MongoVectorConfig::new("mydb", "docs", "vector_index", 1536)
    .with_content_field("text")
    .with_metadata_field("meta");
```

## Adding documents

`MongoVectorStore` implements the `VectorStore` trait:

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
| `database` | `String` | required | MongoDB database name |
| `collection` | `String` | required | MongoDB collection name |
| `index_name` | `String` | required | Atlas vector search index name |
| `dims` | `u32` | required | Dimensionality of embedding vectors |
| `embedding_field` | `String` | `"embedding"` | Field name for the vector embedding |
| `content_field` | `String` | `"content"` | Field name for document text content |
| `metadata_field` | `String` | `"metadata"` | Field name for document metadata |
