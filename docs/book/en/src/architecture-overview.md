# Architecture Overview

Synaptic is organized as a Cargo workspace with 26 library crates, 1 facade crate, and several example binaries. The crates form a layered architecture where each layer builds on the one below it.

## Crate Layers

### Core Layer

**`synaptic-core`** defines all shared traits and types. Every other crate depends on it.

- Traits: `ChatModel`, `Tool`, `RuntimeAwareTool`, `MemoryStore`, `CallbackHandler`, `Store`, `Embeddings`
- Types: `Message`, `ChatRequest`, `ChatResponse`, `ToolCall`, `ToolDefinition`, `ToolChoice`, `AIMessageChunk`, `TokenUsage`, `RunEvent`, `RunnableConfig`, `Runtime`, `ToolRuntime`, `ModelProfile`, `Item`, `ContentBlock`
- Error type: `SynapticError` (20 variants covering all subsystems)
- Stream type: `ChatStream` (`Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send>>`)

### Implementation Crates

Each crate implements one core trait or provides a focused capability:

| Crate | Purpose |
|---|---|
| `synaptic-models` | `ProviderBackend` abstraction, `ScriptedChatModel` test double, wrappers (retry, rate limit, structured output, bound tools) |
| `synaptic-openai` | `OpenAiChatModel` + `OpenAiEmbeddings` |
| `synaptic-anthropic` | `AnthropicChatModel` |
| `synaptic-gemini` | `GeminiChatModel` |
| `synaptic-ollama` | `OllamaChatModel` + `OllamaEmbeddings` |
| `synaptic-tools` | `ToolRegistry`, `SerialToolExecutor`, `ParallelToolExecutor` |
| `synaptic-memory` | Memory strategies: buffer, window, summary, token buffer, summary buffer, `RunnableWithMessageHistory` |
| `synaptic-callbacks` | `RecordingCallback`, `TracingCallback`, `CompositeCallback` |
| `synaptic-prompts` | `PromptTemplate`, `ChatPromptTemplate`, `FewShotChatMessagePromptTemplate` |
| `synaptic-parsers` | Output parsers: string, JSON, structured, list, enum, boolean, XML, markdown list, numbered list |
| `synaptic-cache` | `InMemoryCache`, `SemanticCache`, `CachedChatModel` |

### Composition Crates

These crates provide higher-level orchestration:

| Crate | Purpose |
|---|---|
| `synaptic-runnables` | `Runnable` trait with `invoke()`/`batch()`/`stream()`, `BoxRunnable` with pipe operator, `RunnableLambda`, `RunnableParallel`, `RunnableBranch`, `RunnableAssign`, `RunnablePick`, `RunnableWithFallbacks` |
| `synaptic-graph` | LangGraph-style state machines: `StateGraph`, `CompiledGraph`, `ToolNode`, `create_react_agent`, `create_supervisor`, `create_swarm`, `Command`, `GraphResult`, `Checkpointer`, `MemorySaver`, multi-mode streaming |

### Retrieval Pipeline

These crates form the document ingestion and retrieval pipeline:

| Crate | Purpose |
|---|---|
| `synaptic-loaders` | `TextLoader`, `JsonLoader`, `CsvLoader`, `DirectoryLoader` |
| `synaptic-splitters` | `CharacterTextSplitter`, `RecursiveCharacterTextSplitter`, `MarkdownHeaderTextSplitter`, `TokenTextSplitter` |
| `synaptic-embeddings` | `Embeddings` trait, `FakeEmbeddings`, `CacheBackedEmbeddings` |
| `synaptic-vectorstores` | `VectorStore` trait, `InMemoryVectorStore`, `VectorStoreRetriever` |
| `synaptic-retrieval` | `Retriever` trait, `BM25Retriever`, `MultiQueryRetriever`, `EnsembleRetriever`, `ContextualCompressionRetriever`, `SelfQueryRetriever`, `ParentDocumentRetriever` |

### Evaluation

| Crate | Purpose |
|---|---|
| `synaptic-eval` | `Evaluator` trait, `ExactMatchEvaluator`, `RegexMatchEvaluator`, `JsonValidityEvaluator`, `EmbeddingDistanceEvaluator`, `LLMJudgeEvaluator`, `Dataset`, batch evaluation pipeline |

### Advanced Crates

These crates provide specialized capabilities for production agent systems:

| Crate | Purpose |
|---|---|
| `synaptic-store` | `Store` trait implementation, `InMemoryStore` with semantic search (optional embeddings) |
| `synaptic-middleware` | `AgentMiddleware` trait, `MiddlewareChain`, built-in middleware: model retry, PII filtering, prompt caching, summarization, human-in-the-loop approval, tool call limiting |
| `synaptic-mcp` | Model Context Protocol adapters: `MultiServerMcpClient`, Stdio/SSE/HTTP transports for tool discovery and invocation |
| `synaptic-macros` | Procedural macros: `#[tool]`, `#[chain]`, `#[entrypoint]`, `#[task]`, `#[traceable]`, middleware macros |
| `synaptic-deep` | Deep Agent harness: `Backend` trait (State/Store/Filesystem), 7 filesystem tools, 6 middleware, `create_deep_agent()` factory |

### Integration Crates

These crates provide third-party service integrations:

| Crate | Purpose |
|---|---|
| `synaptic-qdrant` | `QdrantVectorStore` (Qdrant vector database) |
| `synaptic-pgvector` | `PgVectorStore` (PostgreSQL pgvector extension) |
| `synaptic-redis` | `RedisStore` + `RedisCache` (Redis key-value store and LLM cache) |
| `synaptic-pdf` | `PdfLoader` (PDF document loading) |

### Facade

**`synaptic`** re-exports all sub-crates for convenient single-import usage:

```rust
use synaptic::core::{ChatModel, Message, ChatRequest};
use synaptic::openai::OpenAiChatModel;     // requires "openai" feature
use synaptic::models::ScriptedChatModel;   // requires "model-utils" feature
use synaptic::runnables::{Runnable, RunnableLambda};
use synaptic::graph::{StateGraph, create_react_agent};
```

## Dependency Diagram

All crates depend on `synaptic-core` for shared traits and types. Higher-level crates depend on the layer below:

```text
                            ┌──────────┐
                            │ synaptic │  (facade: re-exports all)
                            └─────┬────┘
                                  │
     ┌──────────────┬─────────────┼──────────────┬───────────────┐
     │              │             │              │               │
 ┌───┴───┐   ┌─────┴────┐  ┌────┴─────┐  ┌─────┴────┐   ┌─────┴───┐
 │ deep  │   │middleware│  │  graph   │  │runnables │   │  eval   │
 └───┬───┘   └─────┬────┘  └────┬─────┘  └────┬─────┘   └─────┬───┘
     │              │            │              │               │
     ├──────────────┴────┬───────┴──────────────┤               │
     │                   │                      │               │
┌────┴──┐ ┌─────┐ ┌─────┴──┐ ┌──────┐ ┌───────┐│┌──────┐┌─────┴──┐
│models │ │tools│ │memory  │ │store │ │prompts│││parsers││cache   │
└───┬───┘ └──┬──┘ └───┬────┘ └──┬───┘ └───┬───┘│└───┬───┘└───┬────┘
    │        │        │         │         │    │    │        │
    │  ┌─────┴─┬──────┤    ┌────┘         │    │    │        │
    │  │       │      │    │              │    │    │        │
    ├──┤  ┌────┴──┐   │  ┌─┴────┐  ┌─────┴────┴────┴────────┤
    │  │  │macros │   │  │ mcp  │  │    callbacks            │
    │  │  └───┬───┘   │  └──┬───┘  └────────┬────────────────┘
    │  │      │       │     │               │
  ┌─┴──┴──────┴───────┴─────┴───────────────┴──┐
  │              synaptic-core                  │
  │  (ChatModel, Tool, Store, Embeddings, ...) │
  └──────────────────┬──────────────────────────┘
                     │
  Provider crates (each depends on synaptic-core + synaptic-models):
  openai, anthropic, gemini, ollama

  Retrieval pipeline:

  loaders ──► splitters ──► embeddings ──► vectorstores ──► retrieval

  Integration crates: qdrant, pgvector, redis, pdf
```

## Design Principles

### Async-first with `#[async_trait]`

Every trait in Synaptic is async. The `ChatModel::chat()` method, `Tool::call()`, `MemoryStore::load()`, and `Runnable::invoke()` are all async functions. This means you can freely `await` network calls, database queries, and concurrent operations inside any implementation without blocking the runtime.

### `Arc`-based sharing

Synaptic uses `Arc<RwLock<_>>` for registries (like `ToolRegistry`) where many readers need concurrent access, and `Arc<tokio::sync::Mutex<_>>` for stateful components (like callbacks and memory stores) where mutations must be serialized. This allows safe sharing across async tasks and agent sessions.

### Session isolation

Memory stores and agent runs are keyed by `session_id`. Multiple conversations can run concurrently on the same model and tool set without state leaking between sessions.

### Event-driven callbacks

The `CallbackHandler` trait receives `RunEvent` values at each lifecycle stage (run started, LLM called, tool called, run finished, run failed). You can compose multiple handlers with `CompositeCallback` for logging, tracing, metrics, and recording simultaneously.

### Typed error handling

`SynapticError` has one variant per subsystem (`Prompt`, `Model`, `Tool`, `Memory`, `Graph`, etc.). This makes it straightforward to match on specific failure modes and provide targeted recovery logic.

### Composition over inheritance

Rather than deep trait hierarchies, Synaptic favors composition. A `CachedChatModel` wraps any `ChatModel`. A `RetryChatModel` wraps any `ChatModel`. A `RunnableWithFallbacks` wraps any `Runnable`. You stack behaviors by wrapping, not by extending base classes.
