# LanceDB

[LanceDB](https://lancedb.github.io/lancedb/) is a serverless, embedded vector
database — it runs in-process with no separate server. Data is stored in the
[Lance](https://github.com/lancedb/lance) columnar format on local disk or in
cloud object storage (S3, GCS, Azure Blob).

## Setup

Add the feature flag to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["lancedb"] }
```

No Docker container or external service is required.

## Dependency Note

The `lancedb` crate (>= 0.20) has transitive dependencies that require
Rust >= 1.91. The current `synaptic-lancedb` crate ships a pure-Rust
in-memory backend with the full `VectorStore` interface so that your
application compiles and tests run today at MSRV 1.88. Once the toolchain
requirement aligns, the implementation will be upgraded to use native
Lance on-disk storage.

## Usage

```rust,ignore
use synaptic::lancedb::{LanceDbConfig, LanceDbVectorStore};
use synaptic::core::VectorStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Local file-based storage
    let config = LanceDbConfig::new("/var/lib/myapp/vectors", "documents", 1536);
    let store = LanceDbVectorStore::new(config).await?;

    // Add documents
    // store.add_documents(docs, &embeddings).await?;

    // Search
    // let results = store.similarity_search("query text", 5, &embeddings).await?;

    Ok(())
}
```

## Cloud Storage

When the native lancedb backend is available, S3-backed storage is supported
by simply using an S3 URI:

```rust,ignore
let config = LanceDbConfig::new("s3://my-bucket/vectors", "documents", 1536);
let store = LanceDbVectorStore::new(config).await?;
```

## Configuration

| Field | Type | Description |
|---|---|---|
| `uri` | `String` | Storage path — local (`/data/mydb`) or cloud (`s3://bucket/path`) |
| `table_name` | `String` | Table name within the database |
| `dim` | `usize` | Vector dimension — must match your embedding model |

## Advantages

- **No server required** — runs entirely in-process
- **Versioned** — Lance format supports time-travel queries
- **Cloud-native** — S3/GCS/Azure Blob backed storage without an intermediary service
- **High throughput** — columnar format optimised for scan-heavy vector workloads
