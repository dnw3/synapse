# Elasticsearch Vector Store

This guide shows how to use [Elasticsearch](https://www.elastic.co/elasticsearch) as a vector store backend in Synaptic. Elasticsearch supports approximate kNN (k-nearest neighbors) search using dense vector fields.

## Setup

Add the `elasticsearch` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "elasticsearch"] }
```

Start an Elasticsearch instance (e.g. via Docker):

```bash
docker run -p 9200:9200 -e "discovery.type=single-node" -e "xpack.security.enabled=false" \
  docker.elastic.co/elasticsearch/elasticsearch:8.12.0
```

## Configuration

Create an `ElasticsearchConfig` with the server URL, index name, and vector dimensionality:

```rust,ignore
use synaptic::elasticsearch::{ElasticsearchConfig, ElasticsearchVectorStore};

let config = ElasticsearchConfig::new("http://localhost:9200", "my_index", 1536);
let store = ElasticsearchVectorStore::new(config);
```

### Authentication

For secured Elasticsearch clusters, provide credentials:

```rust,ignore
let config = ElasticsearchConfig::new("https://es.example.com:9200", "my_index", 1536)
    .with_credentials("elastic", "changeme");
```

### Creating the index

Call `ensure_index()` to create the index with the appropriate kNN vector mapping if it does not already exist:

```rust,ignore
store.ensure_index().await?;
```

This creates an index with a `dense_vector` field configured for the specified dimensionality and cosine similarity. The call is idempotent.

### Similarity metric

The default similarity is cosine. You can change it:

```rust,ignore
let config = ElasticsearchConfig::new("http://localhost:9200", "my_index", 1536)
    .with_similarity("dot_product");
```

Available options: `"cosine"` (default), `"dot_product"`, `"l2_norm"`.

## Adding documents

`ElasticsearchVectorStore` implements the `VectorStore` trait:

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
| `url` | `String` | required | Elasticsearch server URL |
| `index_name` | `String` | required | Name of the Elasticsearch index |
| `dims` | `u32` | required | Dimensionality of embedding vectors |
| `username` | `Option<String>` | `None` | Username for basic auth |
| `password` | `Option<String>` | `None` | Password for basic auth |
| `similarity` | `String` | `"cosine"` | Similarity metric (`cosine`, `dot_product`, `l2_norm`) |
