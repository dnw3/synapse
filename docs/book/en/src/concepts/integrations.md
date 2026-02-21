# Integrations

Synaptic uses a **provider-centric** architecture for external service integrations. Each integration lives in its own crate, depends only on `synaptic-core` (plus any provider SDK), and implements one or more core traits.

## Architecture

```text
synaptic-core (defines traits)
  ├── synaptic-openai     (ChatModel + Embeddings)
  ├── synaptic-anthropic  (ChatModel)
  ├── synaptic-gemini     (ChatModel)
  ├── synaptic-ollama     (ChatModel + Embeddings)
  ├── synaptic-qdrant     (VectorStore)
  ├── synaptic-pgvector   (VectorStore)
  ├── synaptic-redis      (Store + LlmCache)
  └── synaptic-pdf        (Loader)
```

All integration crates share a common pattern:

1. **Core traits** — `ChatModel`, `Embeddings`, `VectorStore`, `Store`, `LlmCache`, `Loader` are defined in `synaptic-core`
2. **Independent crates** — Each integration is a separate crate with its own feature flag
3. **Zero coupling** — Integration crates never depend on each other
4. **Config structs** — Builder-pattern configuration with `new()` + `with_*()` methods

## Core Traits

| Trait | Purpose | Crate Implementations |
|-------|---------|----------------------|
| `ChatModel` | LLM chat completion | openai, anthropic, gemini, ollama |
| `Embeddings` | Text embedding vectors | openai, ollama |
| `VectorStore` | Vector similarity search | qdrant, pgvector, (+ in-memory) |
| `Store` | Key-value storage | redis, (+ in-memory) |
| `LlmCache` | LLM response caching | redis, (+ in-memory) |
| `Loader` | Document loading | pdf, (+ text, json, csv, directory) |

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
| `openai` | OpenAI ChatModel + Embeddings |
| `anthropic` | Anthropic ChatModel |
| `gemini` | Google Gemini ChatModel |
| `ollama` | Ollama ChatModel + Embeddings |
| `qdrant` | Qdrant vector store |
| `pgvector` | PostgreSQL pgvector store |
| `redis` | Redis store + cache |
| `pdf` | PDF document loader |

Convenience combinations: `models` (all LLM providers), `agent` (includes openai), `rag` (includes openai + retrieval stack), `full` (everything).

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
- [Migration Guide (0.2 → 0.3)](../migration-0.3.md) — Import path changes
