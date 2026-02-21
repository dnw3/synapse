# PgVector

This guide shows how to use PostgreSQL with the [pgvector](https://github.com/pgvector/pgvector) extension as a vector store backend in Synaptic. This is a good choice when you already run PostgreSQL and want to keep embeddings alongside your relational data.

## Prerequisites

Your PostgreSQL instance must have the pgvector extension installed. On most systems:

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

Refer to the [pgvector installation guide](https://github.com/pgvector/pgvector#installation) for platform-specific instructions.

## Setup

Add the `pgvector` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "pgvector"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
```

The `sqlx` dependency is needed to create the connection pool. Synaptic uses `sqlx::PgPool` for all database operations.

## Creating a store

Connect to PostgreSQL and create the store:

```rust,ignore
use sqlx::postgres::PgPoolOptions;
use synaptic::pgvector::{PgVectorConfig, PgVectorStore};

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;

let config = PgVectorConfig::new("documents", 1536);
let store = PgVectorStore::new(pool, config);
```

The first argument to `PgVectorConfig::new` is the table name; the second is the embedding vector dimensionality (e.g. 1536 for OpenAI `text-embedding-3-small`).

## Initializing the table

Call `initialize()` once to create the pgvector extension and the backing table. This is idempotent and safe to run on every application startup:

```rust,ignore
store.initialize().await?;
```

This creates a table with the following schema:

```sql
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    embedding vector(1536)
);
```

The `vector(N)` column type is provided by the pgvector extension, where `N` matches the `vector_dimensions` in your config.

## Adding documents

`PgVectorStore` implements the `VectorStore` trait. Pass an embeddings provider to compute vectors:

```rust,ignore
use synaptic::pgvector::VectorStore;
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

Documents with empty IDs are assigned a random UUID. Existing documents with the same ID are upserted (content, metadata, and embedding are updated).

## Similarity search

Find the `k` most similar documents using cosine distance (`<=>`):

```rust,ignore
let results = store.similarity_search("fast systems language", 3, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### Search with scores

Get cosine similarity scores (higher is more similar):

```rust,ignore
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

Scores are computed as `1 - cosine_distance`, so a score of 1.0 means identical vectors.

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

Wrap the store in a `VectorStoreRetriever` for use with Synaptic's retrieval infrastructure:

```rust,ignore
use std::sync::Arc;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::openai::OpenAiEmbeddings;
use synaptic::retrieval::Retriever;

let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(store);

let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("fast language", 5).await?;
```

## Schema-qualified table names

You can use schema-qualified names (e.g. `public.documents`) for the table:

```rust,ignore
let config = PgVectorConfig::new("myschema.embeddings", 1536);
```

Table names are validated to contain only alphanumeric characters, underscores, and dots, preventing SQL injection.

## Common patterns

### RAG pipeline with PgVector

```rust,ignore
use synaptic::pgvector::{PgVectorConfig, PgVectorStore, VectorStore};
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::openai::{OpenAiChatModel, OpenAiEmbeddings};
use synaptic::retrieval::{Document, Retriever};
use synaptic::core::{ChatModel, ChatRequest, Message};
use std::sync::Arc;

// Set up the store
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgres://user:pass@localhost/mydb")
    .await?;
let config = PgVectorConfig::new("knowledge_base", 1536);
let store = PgVectorStore::new(pool, config);
store.initialize().await?;

// Add documents
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let docs = vec![
    Document::new("doc1", "Synaptic is a Rust agent framework"),
    Document::new("doc2", "It supports RAG with vector stores"),
];
store.add_documents(docs, embeddings.as_ref()).await?;

// Retrieve and generate
let store = Arc::new(store);
let retriever = VectorStoreRetriever::new(store, embeddings, 3);
let context_docs = retriever.retrieve("What is Synaptic?", 3).await?;

let context = context_docs.iter()
    .map(|d| d.content.as_str())
    .collect::<Vec<_>>()
    .join("\n");

let model = OpenAiChatModel::new("gpt-4o-mini");
let request = ChatRequest::new(vec![
    Message::system(format!("Answer using this context:\n{context}")),
    Message::human("What is Synaptic?"),
]);
let response = model.chat(request).await?;
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `table_name` | `String` | required | PostgreSQL table name (supports schema-qualified names) |
| `vector_dimensions` | `u32` | required | Dimensionality of the embedding vectors |
