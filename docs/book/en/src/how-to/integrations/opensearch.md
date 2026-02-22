# OpenSearch

[OpenSearch](https://opensearch.org/) is an open-source search and analytics
engine with a k-NN (k-Nearest Neighbor) plugin for approximate vector search.
The `synaptic-opensearch` crate implements the `VectorStore` trait using
OpenSearch's HNSW-based k-NN indexing.

## Setup

Add the feature flag to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["opensearch"] }
```

Run OpenSearch locally with Docker:

```bash
docker run -d --name opensearch \
  -p 9200:9200 -p 9600:9600 \
  -e "discovery.type=single-node" \
  -e "plugins.security.disabled=true" \
  opensearchproject/opensearch:latest
```

## Usage

```rust,ignore
use synaptic::opensearch::{OpenSearchConfig, OpenSearchVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OpenSearchConfig::new("http://localhost:9200", "my_index", 1536)
        .with_credentials("admin", "admin");
    let store = OpenSearchVectorStore::new(config);

    // Create the index with k-NN mappings (idempotent)
    store.initialize().await?;

    // Add documents
    // store.add_documents(docs, &embeddings).await?;

    // Search
    // let results = store.similarity_search("query text", 5, &embeddings).await?;

    Ok(())
}
```

## Amazon OpenSearch Service

For Amazon OpenSearch Service, set the endpoint to your AWS-provisioned domain:

```rust,ignore
let config = OpenSearchConfig::new(
    "https://my-domain.us-east-1.es.amazonaws.com",
    "my_index",
    1536,
);
```

## Configuration

| Field | Type | Description |
|---|---|---|
| `endpoint` | `String` | OpenSearch endpoint URL (e.g., `http://localhost:9200`) |
| `index` | `String` | Index name |
| `dim` | `usize` | Vector dimension — must match your embedding model |
| `username` | `Option<String>` | HTTP Basic Auth username |
| `password` | `Option<String>` | HTTP Basic Auth password |
