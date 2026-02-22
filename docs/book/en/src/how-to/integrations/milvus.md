# Milvus

[Milvus](https://milvus.io/) is a purpose-built vector database designed for
billion-scale Approximate Nearest Neighbor Search (ANNS). The `synaptic-milvus`
crate implements the `VectorStore` trait using the Milvus REST API v2.

## Setup

Add the feature flag to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["milvus"] }
```

Run Milvus locally with Docker:

```bash
docker run -d --name milvus-standalone \
  -p 19530:19530 -p 9091:9091 \
  milvusdb/milvus:latest standalone
```

## Usage

```rust,ignore
use synaptic::milvus::{MilvusConfig, MilvusVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MilvusConfig::new("http://localhost:19530", "my_collection", 1536);
    let store = MilvusVectorStore::new(config);

    // Create the collection (idempotent — safe to call on every startup)
    store.initialize().await?;

    // Add documents
    // store.add_documents(docs, &embeddings).await?;

    // Search
    // let results = store.similarity_search("query text", 5, &embeddings).await?;

    Ok(())
}
```

## Zilliz Cloud

For Zilliz Cloud (managed Milvus), add your API key:

```rust,ignore
let config = MilvusConfig::new("https://your-cluster.zillizcloud.com", "collection", 1536)
    .with_api_key("your-api-key");
```

## Configuration

| Field | Type | Description |
|---|---|---|
| `endpoint` | `String` | Milvus endpoint URL (e.g., `http://localhost:19530`) |
| `collection` | `String` | Collection name |
| `dim` | `usize` | Vector dimension — must match your embedding model |
| `api_key` | `Option<String>` | API key for Zilliz Cloud authentication |
