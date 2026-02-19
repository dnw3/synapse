# Vector Stores

This guide shows how to store and search document embeddings using Synaptic's `VectorStore` trait and the built-in `InMemoryVectorStore`.

## Overview

The `VectorStore` trait from `synaptic_vectorstores` provides methods for adding, searching, and deleting documents:

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add_documents(
        &self, docs: Vec<Document>, embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapticError>;

    async fn similarity_search(
        &self, query: &str, k: usize, embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapticError>;

    async fn similarity_search_with_score(
        &self, query: &str, k: usize, embeddings: &dyn Embeddings,
    ) -> Result<Vec<(Document, f32)>, SynapticError>;

    async fn similarity_search_by_vector(
        &self, embedding: &[f32], k: usize,
    ) -> Result<Vec<Document>, SynapticError>;

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapticError>;
}
```

The `embeddings` parameter is passed to each method rather than stored inside the vector store. This design lets you swap embedding providers without rebuilding the store.

## InMemoryVectorStore

An in-memory vector store that uses cosine similarity for search. Backed by a `RwLock<HashMap>`.

### Creating a store

```rust
use synaptic_vectorstores::InMemoryVectorStore;

let store = InMemoryVectorStore::new();
```

### Adding documents

```rust
use synaptic_vectorstores::{InMemoryVectorStore, VectorStore};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::Document;

let store = InMemoryVectorStore::new();
let embeddings = FakeEmbeddings::new(128);

let docs = vec![
    Document::new("1", "Rust is a systems programming language"),
    Document::new("2", "Python is great for data science"),
    Document::new("3", "Go is designed for concurrency"),
];

let ids = store.add_documents(docs, &embeddings).await?;
// ids == ["1", "2", "3"]
```

### Similarity search

Find the `k` most similar documents to a query:

```rust
let results = store.similarity_search("fast systems language", 2, &embeddings).await?;
for doc in &results {
    println!("{}: {}", doc.id, doc.content);
}
```

### Search with scores

Get similarity scores alongside results (higher is more similar):

```rust
let scored = store.similarity_search_with_score("concurrency", 3, &embeddings).await?;
for (doc, score) in &scored {
    println!("{} (score: {:.3}): {}", doc.id, score, doc.content);
}
```

### Search by vector

Search using a pre-computed embedding vector instead of a text query:

```rust
use synaptic_embeddings::Embeddings;

let query_vec = embeddings.embed_query("systems programming").await?;
let results = store.similarity_search_by_vector(&query_vec, 3).await?;
```

### Deleting documents

```rust
store.delete(&["1", "3"]).await?;
```

## Convenience constructors

Create a store pre-populated with documents:

```rust
use synaptic_vectorstores::InMemoryVectorStore;
use synaptic_embeddings::FakeEmbeddings;

let embeddings = FakeEmbeddings::new(128);

// From (id, content) tuples
let store = InMemoryVectorStore::from_texts(
    vec![("1", "Rust is fast"), ("2", "Python is flexible")],
    &embeddings,
).await?;

// From Document values
let store = InMemoryVectorStore::from_documents(docs, &embeddings).await?;
```

## Maximum Marginal Relevance (MMR)

MMR search balances relevance with diversity. The `lambda_mult` parameter controls the trade-off:

- `1.0` -- pure relevance (equivalent to standard similarity search)
- `0.0` -- maximum diversity
- `0.5` -- balanced (typical default)

```rust
let results = store.max_marginal_relevance_search(
    "programming language",
    3,        // k: number of results
    10,       // fetch_k: initial candidates to consider
    0.5,      // lambda_mult: relevance vs. diversity
    &embeddings,
).await?;
```

## VectorStoreRetriever

`VectorStoreRetriever` bridges any `VectorStore` to the `Retriever` trait, making it compatible with the rest of Synaptic's retrieval infrastructure.

```rust
use std::sync::Arc;
use synaptic_vectorstores::{InMemoryVectorStore, VectorStoreRetriever};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::Retriever;

let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::new());
// ... add documents to store ...

let retriever = VectorStoreRetriever::new(store, embeddings, 5);

let results = retriever.retrieve("query", 5).await?;
```

## MultiVectorRetriever

`MultiVectorRetriever` stores small child chunks in a vector store for precise retrieval, but returns the larger parent documents they came from. This gives you the best of both worlds: small chunks for accurate embedding search and full documents for LLM context.

```rust
use std::sync::Arc;
use synaptic_vectorstores::{InMemoryVectorStore, MultiVectorRetriever};
use synaptic_embeddings::FakeEmbeddings;
use synaptic_retrieval::{Document, Retriever};

let embeddings = Arc::new(FakeEmbeddings::new(128));
let store = Arc::new(InMemoryVectorStore::new());

let retriever = MultiVectorRetriever::new(store, embeddings, 3);

// Add parent documents with their child chunks
let parent = Document::new("parent-1", "Full article about Rust ownership...");
let children = vec![
    Document::new("child-1", "Ownership rules in Rust"),
    Document::new("child-2", "Borrowing and references"),
];

retriever.add_documents(parent, children).await?;

// Search finds child chunks but returns the parent
let results = retriever.retrieve("ownership", 1).await?;
assert_eq!(results[0].id, Some("parent-1".to_string()));
```

The `id_key` metadata field links children to their parent. By default it is `"doc_id"`.

### Score threshold filtering

Set a minimum similarity score. Only documents meeting the threshold are returned:

```rust
let retriever = VectorStoreRetriever::new(store, embeddings, 10)
    .with_score_threshold(0.7);

let results = retriever.retrieve("query", 10).await?;
// Only documents with cosine similarity >= 0.7 are included
```
