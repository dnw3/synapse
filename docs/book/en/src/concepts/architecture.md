# Architecture

Synaptic is organized as a workspace of focused Rust crates. Each crate owns exactly one concern, and they compose together through shared traits defined in a single core crate. This page explains the layered design, the principles behind it, and how the crates depend on each other.

## Design Principles

**Async-first.** Every trait in Synaptic is async via `#[async_trait]`, and the runtime is tokio. This is not an afterthought bolted onto a synchronous API -- async is the foundation. LLM calls, tool execution, memory access, and embedding queries are all naturally asynchronous operations, and Synaptic models them as such from the start.

**One crate, one concern.** Each provider has its own crate: `synaptic-openai`, `synaptic-anthropic`, `synaptic-gemini`, `synaptic-ollama`. The `synaptic-tools` crate knows how to register and execute tools. The `synaptic-memory` crate knows how to store and retrieve conversation history. No crate does two jobs. This keeps compile times manageable, makes it possible to use only what you need, and ensures that changes to one subsystem do not cascade across the codebase.

**Shared traits in core.** The `synaptic-core` crate defines every trait and type that crosses crate boundaries: `ChatModel`, `Tool`, `MemoryStore`, `CallbackHandler`, `Message`, `ChatRequest`, `ChatResponse`, `ToolCall`, `SynapticError`, `RunnableConfig`, and more. Implementation crates depend on core, never on each other (unless composition requires it).

**Concurrency-safe by default.** Shared registries use `Arc<RwLock<_>>` (standard library `RwLock` for low-contention read-heavy data like tool registries). Mutable state that requires async access -- callbacks, memory stores, checkpointers -- uses `Arc<tokio::sync::Mutex<_>>` or `Arc<tokio::sync::RwLock<_>>`. All core traits require `Send + Sync`.

**Session isolation.** Memory, agent runs, and graph checkpoints are keyed by a session or thread identifier. Two concurrent conversations never interfere with each other, even when they share the same model and tool instances.

**Event-driven observability.** The `RunEvent` enum captures every significant lifecycle event (run started, LLM called, tool called, run finished, run failed). Callback handlers receive these events asynchronously, enabling logging, tracing, recording, and custom side effects without modifying application code.

## The Four Layers

Synaptic's crates fall into four layers, each building on the ones below it.

### Layer 1: Core

`synaptic-core` is the foundation. It defines:

- **Traits**: `ChatModel`, `Tool`, `MemoryStore`, `CallbackHandler`
- **Message types**: The `Message` enum (System, Human, AI, Tool, Chat, Remove), `AIMessageChunk` for streaming, `ToolCall`, `ToolDefinition`, `ToolChoice`
- **Request/response**: `ChatRequest`, `ChatResponse`, `TokenUsage`
- **Streaming**: The `ChatStream` type alias (`Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapticError>> + Send>>`)
- **Configuration**: `RunnableConfig` (tags, metadata, concurrency limits, run IDs)
- **Events**: `RunEvent` enum with six lifecycle variants
- **Errors**: `SynapticError` enum with 19 variants spanning all subsystems

Every other crate in the workspace depends on `synaptic-core`. Nothing depends on `synaptic-core` except through this single shared foundation.

### Layer 2: Implementation Crates

Each crate implements one core concern:

| Crate | Purpose |
|-------|---------|
| `synaptic-models` | `ProviderBackend` abstraction, test doubles (`ScriptedChatModel`), wrappers (`RetryChatModel`, `RateLimitedChatModel`, `StructuredOutputChatModel<T>`, `BoundToolsChatModel`) |
| `synaptic-openai` | `OpenAiChatModel` + `OpenAiEmbeddings` |
| `synaptic-anthropic` | `AnthropicChatModel` |
| `synaptic-gemini` | `GeminiChatModel` |
| `synaptic-ollama` | `OllamaChatModel` + `OllamaEmbeddings` |
| `synaptic-tools` | `ToolRegistry`, `SerialToolExecutor`, `ParallelToolExecutor`, `HandleErrorTool`, `ReturnDirectTool` |
| `synaptic-memory` | `ChatMessageHistory` and strategy types: Buffer, Window, Summary, TokenBuffer, SummaryBuffer, `RunnableWithMessageHistory` |
| `synaptic-callbacks` | `RecordingCallback`, `TracingCallback`, `CompositeCallback` |
| `synaptic-prompts` | `PromptTemplate`, `ChatPromptTemplate`, `FewShotChatMessagePromptTemplate`, `ExampleSelector` |
| `synaptic-parsers` | Output parsers: `StrOutputParser`, `JsonOutputParser`, `StructuredOutputParser<T>`, `ListOutputParser`, `EnumOutputParser`, `BooleanOutputParser`, `MarkdownListOutputParser`, `NumberedListOutputParser`, `XmlOutputParser`, `RetryOutputParser`, `FixingOutputParser` |
| `synaptic-cache` | `InMemoryCache`, `SemanticCache`, `CachedChatModel` |
| `synaptic-eval` | Evaluators (ExactMatch, JsonValidity, RegexMatch, EmbeddingDistance, LLMJudge), `Dataset`, batch evaluation pipeline |

### Layer 3: Composition and Retrieval

These crates combine the implementation crates into higher-level abstractions:

| Crate | Purpose |
|-------|---------|
| `synaptic-runnables` | The LCEL system: `Runnable` trait, `BoxRunnable` with pipe operator, `RunnableSequence`, `RunnableParallel`, `RunnableBranch`, `RunnableWithFallbacks`, `RunnableAssign`, `RunnablePick`, `RunnableEach`, `RunnableRetry`, `RunnableGenerator` |
| `synaptic-graph` | LangGraph-style state machines: `StateGraph` builder, `CompiledGraph`, `Node` trait, `ToolNode`, `create_react_agent()`, `StoreCheckpointer`, streaming, visualization |
| `synaptic-loaders` | Document loaders: `TextLoader`, `JsonLoader`, `CsvLoader`, `DirectoryLoader`, `FileLoader`, `MarkdownLoader`, `WebLoader` |
| `synaptic-splitters` | Text splitters: `CharacterTextSplitter`, `RecursiveCharacterTextSplitter`, `MarkdownHeaderTextSplitter`, `HtmlHeaderTextSplitter`, `LanguageTextSplitter`, `TokenTextSplitter` |
| `synaptic-embeddings` | `Embeddings` trait, `FakeEmbeddings`, `CacheBackedEmbeddings` |
| `synaptic-vectorstores` | `VectorStore` trait, `InMemoryVectorStore`, `MultiVectorRetriever` |
| `synaptic-retrieval` | `Retriever` trait and seven implementations: InMemory, BM25, MultiQuery, Ensemble, ContextualCompression, SelfQuery, ParentDocument |
| `synaptic-qdrant` | `QdrantVectorStore` (Qdrant integration) |
| `synaptic-postgres` | `PgVectorStore`, `PgStore`, `PgCache`, `PgCheckpointer` (PostgreSQL integration) |
| `synaptic-redis` | `RedisStore` + `RedisCache` (Redis integration) |
| `synaptic-pdf` | `PdfLoader` (PDF document loading) |

### Layer 4: Facade

The `synaptic` crate re-exports everything from all sub-crates under a unified namespace. Application code can use a single dependency:

```toml
[dependencies]
synaptic = "0.2"
```

And then import from organized modules:

```rust
use synaptic::core::{Message, ChatRequest};
use synaptic::openai::OpenAiChatModel;       // requires "openai" feature
use synaptic::anthropic::AnthropicChatModel; // requires "anthropic" feature
use synaptic::graph::{create_react_agent, MessageState};
use synaptic::runnables::{BoxRunnable, Runnable};
```

## Crate Dependency Diagram

```
                       synaptic (facade)
                             |
        +--------------------+--------------------+
        |                    |                    |
   synaptic-graph      synaptic-runnables    synaptic-eval
        |                    |                    |
   synaptic-tools        synaptic-core       synaptic-embeddings
        |                    ^                    |
   synaptic-core              |               synaptic-core
                             |
        +--------+-----------+-----------+--------+--------+
        |        |           |           |        |        |
   synap-   synap-    synap-    synap-   synap-  Provider
   tic-     tic-      tic-      tic-     tic-    crates:
   models   memory    callbacks prompts  parsers openai,
        |        |           |           |        | anthropic,
        +--------+-----------+-----------+--------+ gemini,
                             |                      ollama
                        synaptic-core

   Retrieval pipeline (all depend on synaptic-core):

   synaptic-loaders --> synaptic-splitters --> synaptic-embeddings
                                                   |
                                            synaptic-vectorstores
                                                   |
                                            synaptic-retrieval

   Integration crates (each depends on synaptic-core):

   synaptic-qdrant, synaptic-postgres, synaptic-redis, synaptic-pdf
```

The arrows point downward toward dependencies. Every crate ultimately depends on `synaptic-core`. The composition crates (`synaptic-graph`, `synaptic-runnables`) additionally depend on the implementation crates they orchestrate.

## Provider Abstraction

Each LLM provider lives in its own crate (`synaptic-openai`, `synaptic-anthropic`, `synaptic-gemini`, `synaptic-ollama`). They all use the `ProviderBackend` trait from `synaptic-models` to separate HTTP concerns from protocol mapping. `HttpBackend` makes real HTTP requests; `FakeBackend` returns scripted responses for testing. This means you can test any code that uses `ChatModel` without network access and without mocking at the HTTP level. You only compile the providers you actually use.

## The Runnable Abstraction

The `Runnable<I, O>` trait in `synaptic-runnables` is the universal composition primitive. Prompt templates, output parsers, chat models, and entire graphs can all be treated as runnables. They compose via the `|` pipe operator into chains that can be invoked, batched, or streamed. See [Runnables & LCEL](./runnables-lcel.md) for details.

## The Graph Abstraction

The `StateGraph` builder in `synaptic-graph` provides a higher-level orchestration model for complex workflows. Where LCEL chains are linear pipelines (with branching), graphs support cycles, conditional routing, checkpointing, human-in-the-loop interrupts, and dynamic control flow via `GraphCommand`. See [Graph](./graph.md) for details.

## See Also

- [Installation](../installation.md) -- feature flags for enabling specific crates
- [Runnables & LCEL](./runnables-lcel.md) -- the composition primitive
- [Graph](./graph.md) -- state-machine orchestration
- [Middleware](./middleware.md) -- cross-cutting agent concerns
- [Key-Value Store](./store.md) -- persistent namespaced storage
