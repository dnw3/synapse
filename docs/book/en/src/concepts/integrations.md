# Integrations

Synaptic uses a **provider-centric** architecture for external service integrations. Each integration lives in its own crate, depends only on `synaptic-core` (plus any provider SDK), and implements one or more core traits.

## Architecture

```text
synaptic-core (defines traits)
  ├── synaptic-openai          (ChatModel + Embeddings)
  ├── synaptic-anthropic       (ChatModel)
  ├── synaptic-gemini          (ChatModel)
  ├── synaptic-ollama          (ChatModel + Embeddings)
  ├── synaptic-bedrock         (ChatModel)
  ├── synaptic-cohere          (DocumentCompressor)
  ├── synaptic-qdrant          (VectorStore)
  ├── synaptic-pgvector        (VectorStore)
  ├── synaptic-pinecone        (VectorStore)
  ├── synaptic-chroma          (VectorStore)
  ├── synaptic-mongodb         (VectorStore)
  ├── synaptic-elasticsearch   (VectorStore)
  ├── synaptic-redis           (Store + LlmCache)
  ├── synaptic-sqlite          (LlmCache)
  ├── synaptic-pdf             (Loader)
  └── synaptic-tavily          (Tool)
```

All integration crates share a common pattern:

1. **Core traits** — `ChatModel`, `Embeddings`, `VectorStore`, `Store`, `LlmCache`, `Loader` are defined in `synaptic-core`
2. **Independent crates** — Each integration is a separate crate with its own feature flag
3. **Zero coupling** — Integration crates never depend on each other
4. **Config structs** — Builder-pattern configuration with `new()` + `with_*()` methods

## Core Traits

| Trait | Purpose | Crate Implementations |
|-------|---------|----------------------|
| `ChatModel` | LLM chat completion | openai, anthropic, gemini, ollama, bedrock |
| `Embeddings` | Text embedding vectors | openai, ollama |
| `VectorStore` | Vector similarity search | qdrant, pgvector, pinecone, chroma, mongodb, elasticsearch, (+ in-memory) |
| `Store` | Key-value storage | redis, (+ in-memory) |
| `LlmCache` | LLM response caching | redis, sqlite, (+ in-memory) |
| `Loader` | Document loading | pdf, (+ text, json, csv, directory) |
| `DocumentCompressor` | Document reranking/filtering | cohere, (+ embeddings filter) |
| `Tool` | Agent tool | tavily, (+ custom tools) |

## LLM Provider Pattern

All LLM providers follow the same pattern — a config struct, a model struct, and a `ProviderBackend` for HTTP transport:

```rust,ignore
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};
use synaptic::models::{HttpBackend, FakeBackend};

// Production
let config = OpenAiConfig::new("sk-...", "gpt-4o");
let model = OpenAiChatModel::new(config, Arc::new(HttpBackend::new()));

// Testing (no network calls)
let model = OpenAiChatModel::new(config, Arc::new(FakeBackend::with_responses(vec![...])));
```

The `ProviderBackend` abstraction (in `synaptic-models`) enables:
- `HttpBackend` — real HTTP calls in production
- `FakeBackend` — deterministic responses in tests

## Storage & Retrieval Pattern

Vector stores, key-value stores, and caches implement core traits that allow drop-in replacement:

```rust,ignore
// Swap InMemoryVectorStore for QdrantVectorStore — same trait interface
use synaptic::qdrant::{QdrantVectorStore, QdrantConfig};

let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536);
let store = QdrantVectorStore::new(config);
store.add_documents(docs, &embeddings).await?;
let results = store.similarity_search("query", 5, &embeddings).await?;
```

## Feature Flags

Each integration has its own feature flag in the `synaptic` facade crate:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant"] }
```

| Feature | Integration |
|---------|------------|
| `openai` | OpenAI ChatModel + Embeddings (+ OpenAI-compatible providers + Azure) |
| `anthropic` | Anthropic ChatModel |
| `gemini` | Google Gemini ChatModel |
| `ollama` | Ollama ChatModel + Embeddings |
| `bedrock` | AWS Bedrock ChatModel |
| `cohere` | Cohere Reranker |
| `qdrant` | Qdrant vector store |
| `pgvector` | PostgreSQL pgvector store |
| `pinecone` | Pinecone vector store |
| `chroma` | Chroma vector store |
| `mongodb` | MongoDB Atlas vector search |
| `elasticsearch` | Elasticsearch vector store |
| `redis` | Redis store + cache |
| `sqlite` | SQLite LLM cache |
| `pdf` | PDF document loader |
| `tavily` | Tavily search tool |

Convenience combinations: `models` (all 6 LLM providers including bedrock and cohere), `agent` (includes openai), `rag` (includes openai + retrieval stack), `full` (everything).

## Adding a New Integration

To add a new integration:

1. Create a new crate `synaptic-{name}` in `crates/`
2. Depend on `synaptic-core` for trait definitions
3. Implement the appropriate trait(s)
4. Add a feature flag in the `synaptic` facade crate
5. Re-export via `pub use synaptic_{name} as {name}` in the facade `lib.rs`

## See Also

- [Installation](../installation.md) — Feature flag reference
- [Architecture](architecture.md) — Overall system design
