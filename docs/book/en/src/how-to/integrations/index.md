# Integrations

Synaptic provides optional integration crates that connect to external services. Each integration is gated behind a Cargo feature flag and adds no overhead when not enabled.

## Available Integrations

| Integration | Feature | Purpose |
|-------------|---------|---------|
| [Qdrant](qdrant.md) | `qdrant` | Vector store backed by the Qdrant vector database |
| [PgVector](pgvector.md) | `pgvector` | Vector store backed by PostgreSQL with the pgvector extension |
| [Redis](redis.md) | `redis` | Key-value store and LLM response cache backed by Redis |
| [PDF Loader](pdf.md) | `pdf` | Document loader for PDF files |

## Enabling integrations

Add the desired feature flags to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant", "redis"] }
```

You can combine any number of feature flags. Each integration pulls in only the dependencies it needs.

## Trait compatibility

Every integration implements a core Synaptic trait, so it plugs directly into the existing framework:

- **Qdrant** and **PgVector** implement `VectorStore` -- use them with `VectorStoreRetriever`, `MultiVectorRetriever`, or any component that accepts `&dyn VectorStore`.
- **Redis Store** implements `Store` -- use it anywhere `InMemoryStore` is used, including agent `ToolRuntime` injection.
- **Redis Cache** implements `LlmCache` -- wrap any `ChatModel` with `CachedChatModel` for persistent response caching.
- **PDF Loader** implements `Loader` -- use it in RAG pipelines alongside `TextSplitter`, `Embeddings`, and `VectorStore`.

## Guides

- [Qdrant Vector Store](qdrant.md) -- store and search embeddings with Qdrant
- [PgVector](pgvector.md) -- store and search embeddings with PostgreSQL + pgvector
- [Redis Store & Cache](redis.md) -- persistent key-value storage and LLM caching with Redis
- [PDF Loader](pdf.md) -- load documents from PDF files
