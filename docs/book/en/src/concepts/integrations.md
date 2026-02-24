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
  ├── synaptic-groq            (ChatModel — OpenAI-compatible, LPU)
  ├── synaptic-mistral         (ChatModel — OpenAI-compatible)
  ├── synaptic-deepseek        (ChatModel — OpenAI-compatible)
  ├── synaptic-cohere          (DocumentCompressor + Embeddings)
  ├── synaptic-huggingface     (Embeddings)
  ├── synaptic-qdrant          (VectorStore)
  ├── synaptic-postgres        (VectorStore + Store + LlmCache + Checkpointer)
  ├── synaptic-pinecone        (VectorStore)
  ├── synaptic-chroma          (VectorStore)
  ├── synaptic-mongodb         (VectorStore)
  ├── synaptic-elasticsearch   (VectorStore)
  ├── synaptic-weaviate        (VectorStore)
  ├── synaptic-redis           (Store + LlmCache + Checkpointer)
  ├── synaptic-sqlite          (LlmCache)
  ├── synaptic-pdf             (Loader)
  ├── synaptic-tavily          (Tool)
  └── synaptic-sqltoolkit      (Tool×3: ListTables, DescribeTable, ExecuteQuery)
```

All integration crates share a common pattern:

1. **Core traits** — `ChatModel`, `Embeddings`, `VectorStore`, `Store`, `LlmCache`, `Loader` are defined in `synaptic-core`
2. **Independent crates** — Each integration is a separate crate with its own feature flag
3. **Zero coupling** — Integration crates never depend on each other
4. **Config structs** — Builder-pattern configuration with `new()` + `with_*()` methods

## Core Traits

| Trait | Purpose | Crate Implementations |
|-------|---------|----------------------|
| `ChatModel` | LLM chat completion | openai, anthropic, gemini, ollama, bedrock, groq, mistral, deepseek |
| `Embeddings` | Text embedding vectors | openai, ollama, cohere, huggingface |
| `VectorStore` | Vector similarity search | qdrant, postgres, pinecone, chroma, mongodb, elasticsearch, weaviate, (+ in-memory) |
| `Store` | Key-value storage | redis, postgres, (+ in-memory) |
| `LlmCache` | LLM response caching | redis, postgres, sqlite, (+ in-memory) |
| `Checkpointer` | Graph state persistence | redis, postgres |
| `Loader` | Document loading | pdf, (+ text, json, csv, directory) |
| `DocumentCompressor` | Document reranking/filtering | cohere, (+ embeddings filter) |
| `Tool` | Agent tool | tavily, sqltoolkit (3 tools), duckduckgo, wikipedia, (+ custom tools) |

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
| `groq` | Groq ChatModel (ultra-fast LPU inference, OpenAI-compatible) |
| `mistral` | Mistral ChatModel (OpenAI-compatible) |
| `deepseek` | DeepSeek ChatModel (cost-efficient reasoning, OpenAI-compatible) |
| `cohere` | Cohere Reranker + Embeddings |
| `huggingface` | HuggingFace Inference API Embeddings |
| `qdrant` | Qdrant vector store |
| `postgres` | PostgreSQL store, cache, vector store, graph checkpointer |
| `pinecone` | Pinecone vector store |
| `chroma` | Chroma vector store |
| `mongodb` | MongoDB Atlas vector search |
| `elasticsearch` | Elasticsearch vector store |
| `weaviate` | Weaviate vector store |
| `redis` | Redis store + cache + graph checkpointer |
| `sqlite` | SQLite LLM cache |
| `pdf` | PDF document loader |
| `tavily` | Tavily search tool |
| `sqltoolkit` | SQL database toolkit (ListTables, DescribeTable, ExecuteQuery) |

Convenience combinations: `models` (all 9 LLM providers), `agent` (includes openai + graph), `rag` (includes openai + retrieval stack), `full` (everything).

## Provider Selection Guide

Choose a provider based on your requirements:

| Provider | Auth | Streaming | Tool Calling | Embeddings | Best For |
|----------|------|-----------|-------------|------------|----------|
| **OpenAI** | API key (header) | SSE | Yes | Yes | General-purpose, widest model selection |
| **Anthropic** | API key (`x-api-key`) | SSE | Yes | No | Long context, reasoning tasks |
| **Gemini** | API key (query param) | SSE | Yes | No | Google ecosystem, multimodal |
| **Ollama** | None (local) | NDJSON | Yes | Yes | Privacy-sensitive, offline, development |
| **Bedrock** | AWS IAM | AWS SDK | Yes | No | Enterprise AWS environments |
| **Groq** | API key (header) | SSE | Yes | No | Ultra-fast inference (LPU), latency-critical |
| **Mistral** | API key (header) | SSE | Yes | No | EU compliance, cost-efficient tool calling |
| **DeepSeek** | API key (header) | SSE | Yes | No | Cost-efficient reasoning (90%+ cheaper) |
| **Cohere** | API key (header) | — | — | Yes | Reranking + production-grade embeddings |
| **HuggingFace** | API key (optional) | — | — | Yes | Open-source sentence-transformers |

**Deciding factors:**

- **Privacy & compliance** — Ollama runs entirely locally; Bedrock keeps data within AWS
- **Cost** — Ollama is free; OpenAI-compatible providers (Groq, DeepSeek) offer competitive pricing
- **Latency** — Ollama has no network round-trip; Groq is optimized for speed
- **Ecosystem** — OpenAI has the most third-party integrations; Bedrock integrates with AWS services

## Vector Store Selection Guide

| Store | Deployment | Managed | Filtering | Scaling | Best For |
|-------|-----------|---------|-----------|---------|----------|
| **Qdrant** | Self-hosted / Cloud | Yes (Qdrant Cloud) | Rich (payload filters) | Horizontal | General-purpose, production |
| **pgvector** | Self-hosted | Via managed Postgres | SQL WHERE clauses | Vertical | Teams already using PostgreSQL |
| **Pinecone** | Fully managed | Yes | Metadata filters | Automatic | Zero-ops, rapid prototyping |
| **Chroma** | Self-hosted / Docker | No | Metadata filters | Single node | Development, small-medium datasets |
| **MongoDB Atlas** | Fully managed | Yes | MQL filters | Automatic | Teams already using MongoDB |
| **Elasticsearch** | Self-hosted / Cloud | Yes (Elastic Cloud) | Full query DSL | Horizontal | Hybrid text + vector search |
| **Weaviate** | Self-hosted / Cloud | Yes (WCS) | GraphQL filters | Horizontal | Multi-tenancy, hybrid search |
| **InMemory** | In-process | N/A | None | N/A | Testing, prototyping |

**Deciding factors:**

- **Existing infrastructure** — Use pgvector if you have PostgreSQL, MongoDB Atlas if you use MongoDB, Elasticsearch if you already run an ES cluster
- **Operational complexity** — Pinecone and MongoDB Atlas are fully managed; Qdrant and Elasticsearch require cluster management
- **Query capabilities** — Elasticsearch excels at hybrid text + vector queries; Qdrant has the richest filtering
- **Cost** — InMemory and Chroma are free; pgvector reuses existing database infrastructure

## Cache Selection Guide

| Cache | Persistence | Deployment | TTL Support | Best For |
|-------|------------|-----------|-------------|----------|
| **InMemory** | No (process lifetime) | In-process | Yes | Testing, single-process apps |
| **Redis** | Yes (configurable) | External server | Yes | Multi-process, distributed |
| **SQLite** | Yes (file-based) | In-process | Yes | Single-machine persistence |
| **Semantic** | Depends on backing store | In-process | No | Fuzzy-match caching |

## Complete RAG Pipeline Example

This example combines multiple integrations into a full retrieval-augmented generation pipeline with caching and reranking:

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message, Embeddings};
use synaptic::openai::{OpenAiChatModel, OpenAiConfig, OpenAiEmbeddings};
use synaptic::qdrant::{QdrantConfig, QdrantVectorStore};
use synaptic::cohere::{CohereReranker, CohereConfig};
use synaptic::cache::{CachedChatModel, InMemoryCache};
use synaptic::retrieval::ContextualCompressionRetriever;
use synaptic::splitters::RecursiveCharacterTextSplitter;
use synaptic::loaders::TextLoader;
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::models::HttpBackend;
use std::sync::Arc;

let backend = Arc::new(HttpBackend::new());

// 1. Set up embeddings
let embeddings = Arc::new(OpenAiEmbeddings::new(
    OpenAiEmbeddings::config("text-embedding-3-small"),
    backend.clone(),
));

// 2. Ingest documents into Qdrant
let loader = TextLoader::new("knowledge-base.txt");
let docs = loader.load().await?;
let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&docs)?;

let qdrant_config = QdrantConfig::new("http://localhost:6334", "knowledge", 1536);
let store = QdrantVectorStore::new(qdrant_config, embeddings.clone()).await?;
store.add_documents(&chunks).await?;

// 3. Build retriever with Cohere reranking
let base_retriever = Arc::new(VectorStoreRetriever::new(Arc::new(store)));
let reranker = CohereReranker::new(CohereConfig::new(std::env::var("COHERE_API_KEY")?));
let retriever = ContextualCompressionRetriever::new(base_retriever, Arc::new(reranker));

// 4. Wrap the LLM with a cache
let llm_config = OpenAiConfig::new(std::env::var("OPENAI_API_KEY")?, "gpt-4o");
let base_model = OpenAiChatModel::new(llm_config, backend.clone());
let cache = Arc::new(InMemoryCache::new());
let model = CachedChatModel::new(Arc::new(base_model), cache);

// 5. Retrieve and generate
let relevant = retriever.retrieve("How does Synaptic handle streaming?").await?;
let context = relevant.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join("\n\n");

let request = ChatRequest::new(vec![
    Message::system(&format!("Answer based on the following context:\n\n{context}")),
    Message::human("How does Synaptic handle streaming?"),
]);
let response = model.chat(&request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

This pipeline demonstrates:
- **Qdrant** for vector storage and retrieval
- **Cohere** for reranking retrieved documents
- **InMemoryCache** for caching LLM responses (swap with Redis/SQLite for persistence)
- **OpenAI** for both embeddings and chat completion

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
