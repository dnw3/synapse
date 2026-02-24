# Synaptic

[![CI](https://github.com/dnw3/synaptic/actions/workflows/ci.yml/badge.svg)](https://github.com/dnw3/synaptic/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/synaptic.svg)](https://crates.io/crates/synaptic)
[![docs.rs](https://docs.rs/synaptic/badge.svg)](https://docs.rs/synaptic)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-orange.svg)](https://blog.rust-lang.org/2025/05/15/Rust-1.88.0.html)

A Rust agent framework with LangChain-compatible architecture and Rust-native async interfaces.

## Features

- **LLM Providers** — OpenAI, Anthropic, Gemini, Ollama, AWS Bedrock, Groq, Mistral AI, DeepSeek, and any OpenAI-compatible API (xAI, Together, Fireworks, OpenRouter…)
- **LCEL Composition** — `Runnable` trait with pipe operator (`|`), streaming, bind, parallel, branch, assign/pick, fallbacks
- **Graph Orchestration** — LangGraph-style `StateGraph` with conditional edges, persistent checkpointing (Redis + PostgreSQL), human-in-the-loop, streaming
- **ReAct Agent** — `create_react_agent(model, tools)` with automatic tool dispatch
- **Tool System** — `Tool` trait, `ToolRegistry`, `SerialToolExecutor`, `ParallelToolExecutor`, built-in tools (Tavily, DuckDuckGo, Wikipedia, SQL Toolkit)
- **Memory** — Buffer, Window, Summary, SummaryBuffer, TokenBuffer strategies with `RunnableWithMessageHistory`
- **Prompt Templates** — Chat templates, few-shot prompting, placeholder interpolation
- **Output Parsers** — String, JSON, Structured\<T\>, List, Boolean, Enum, XML — all composable as `Runnable`
- **RAG Pipeline** — Document loaders (Text, JSON, CSV, Markdown, Directory, Web, PDF), text splitters, embeddings (OpenAI, Ollama, Cohere, HuggingFace), vector stores (InMemory, Qdrant, pgvector, Pinecone, Chroma, MongoDB, Elasticsearch, Weaviate), 7 retriever types
- **Caching** — In-memory (TTL), semantic (embedding similarity), Redis, SQLite, `CachedChatModel` wrapper
- **Evaluation** — ExactMatch, JsonValidity, Regex, EmbeddingDistance, LLMJudge evaluators
- **Structured Output** — `StructuredOutputChatModel<T>` with JSON schema enforcement
- **Observability** — `TracingCallback` (structured spans), `CompositeCallback`, `StdOutCallback`
- **MCP** — `MultiServerMcpClient` with Stdio/SSE/HTTP transport adapters
- **Macros** — `#[tool]`, `#[chain]`, `#[entrypoint]`, `#[task]`, `#[traceable]` proc-macros
- **Deep Agent** — Filesystem-aware deep research agent harness (`create_deep_agent`)
- **Middleware** — `AgentMiddleware` trait: Retry, PII redaction, Prompt Caching, Summarization

## Installation

```toml
[dependencies]
synaptic = { version = "0.2", features = ["agent"] }
```

### Feature flags

| Feature | Contents |
|---------|----------|
| `default` | runnables + prompts + parsers + tools + callbacks |
| `openai` | OpenAI ChatModel + Embeddings |
| `anthropic` | Anthropic Claude ChatModel |
| `gemini` | Google Gemini ChatModel |
| `ollama` | Ollama ChatModel + Embeddings |
| `bedrock` | AWS Bedrock ChatModel |
| `groq` | Groq ChatModel (ultra-fast LPU inference) |
| `mistral` | Mistral AI ChatModel + Embeddings |
| `deepseek` | DeepSeek ChatModel (R1 reasoning + V3) |
| `models` | All chat model providers |
| `qdrant` | Qdrant vector store |
| `postgres` | PostgreSQL store, cache, vector store, graph checkpointer |
| `redis` | Redis store + LLM cache + graph checkpointer |
| `weaviate` | Weaviate vector store |
| `pinecone` | Pinecone vector store |
| `chroma` | Chroma vector store |
| `mongodb` | MongoDB Atlas vector search |
| `elasticsearch` | Elasticsearch vector store |
| `sqlite` | SQLite LLM cache |
| `huggingface` | HuggingFace Inference API embeddings |
| `cohere` | Cohere reranker + embeddings |
| `tavily` | Tavily search tool |
| `sqltoolkit` | SQL database toolkit (ListTables, DescribeTable, ExecuteQuery) |
| `pdf` | PDF document loader |
| `graph` | LangGraph-style StateGraph |
| `memory` | Conversation memory strategies |
| `retrieval` | Retriever types (BM25, Ensemble, etc.) |
| `cache` | LLM response caching |
| `eval` | Evaluators |
| `mcp` | MCP server client |
| `macros` | Proc-macros |
| `deep` | Deep Agent harness |
| `agent` | default + openai + graph + memory + middleware + store |
| `rag` | default + openai + embeddings + retrieval + loaders + splitters + vectorstores |
| `full` | Everything |

## Quick Start

```rust
use synaptic::core::{ChatModel, Message, ChatRequest, ToolChoice};
use synaptic::runnables::{Runnable, RunnableLambda};
use synaptic::graph::{create_react_agent, MessageState};

// LCEL pipe composition
let chain = step1.boxed() | step2.boxed() | step3.boxed();
let result = chain.invoke(input, &config).await?;

// ReAct agent
let graph = create_react_agent(model, tools)?;
let state = MessageState { messages: vec![Message::human("Hello")] };
let result = graph.invoke(state).await?;
```

## Workspace Layout

45+ library crates in `crates/`, examples in `examples/`:

### Core

| Crate | Description |
|-------|-------------|
| `synaptic-core` | Shared traits and types: `ChatModel`, `Message`, `ToolChoice`, `SynapticError` |
| `synaptic-models` | `ProviderBackend` + `HttpBackend` + `FakeBackend`, wrappers (Retry, RateLimit, StructuredOutput) |
| `synaptic-runnables` | LCEL: `Runnable`, `BoxRunnable`, pipe, Lambda, Parallel, Branch, Assign, Pick, Fallbacks |
| `synaptic-prompts` | `ChatPromptTemplate`, `FewShotChatMessagePromptTemplate` |
| `synaptic-parsers` | Str, JSON, Structured, List, Boolean, Enum, XML output parsers |
| `synaptic-tools` | `ToolRegistry`, `SerialToolExecutor`, `ParallelToolExecutor`, DuckDuckGo, Wikipedia |
| `synaptic-memory` | Buffer, Window, Summary, SummaryBuffer, TokenBuffer, `RunnableWithMessageHistory` |
| `synaptic-callbacks` | `RecordingCallback`, `TracingCallback`, `CompositeCallback` |
| `synaptic-graph` | `StateGraph`, `CompiledGraph`, `ToolNode`, `create_react_agent`, `MemorySaver` |
| `synaptic-retrieval` | BM25, MultiQuery, Ensemble, Compression, SelfQuery, ParentDocument retrievers |
| `synaptic-loaders` | Text, JSON, CSV, Markdown, Directory, Web, PDF document loaders |
| `synaptic-splitters` | Character, Recursive, Markdown, Token, HTML text splitters |
| `synaptic-embeddings` | `Embeddings` trait, `FakeEmbeddings`, `CacheBackedEmbeddings` |
| `synaptic-vectorstores` | `VectorStore` trait, `InMemoryVectorStore`, `VectorStoreRetriever` |
| `synaptic-cache` | InMemory + Semantic LLM caches, `CachedChatModel` |
| `synaptic-eval` | `Evaluator` trait, 5 evaluators, `Dataset`, batch `evaluate()` |
| `synaptic-store` | `InMemoryStore` with semantic search |
| `synaptic-middleware` | `AgentMiddleware` trait, PII, Retry, Prompt Caching, Summarization |
| `synaptic-mcp` | `MultiServerMcpClient`, Stdio/SSE/HTTP transports |
| `synaptic-macros` | Proc-macros: `#[tool]`, `#[chain]`, `#[entrypoint]`, `#[task]`, `#[traceable]` |
| `synaptic-deep` | Deep Agent harness with filesystem tools + `create_deep_agent()` |
| `synaptic` | Unified facade with feature-gated re-exports |

### Chat Model Providers

| Crate | Provider |
|-------|----------|
| `synaptic-openai` | OpenAI (GPT-4o, o1, o3…) + Azure OpenAI + 9 OpenAI-compatible APIs |
| `synaptic-anthropic` | Anthropic (Claude 4.6, Claude Haiku…) |
| `synaptic-gemini` | Google Gemini (1.5 Pro, 2.0 Flash…) |
| `synaptic-ollama` | Ollama (local models) |
| `synaptic-bedrock` | AWS Bedrock (Titan, Claude, Llama via Bedrock) |
| `synaptic-groq` | Groq (Llama 3.3 70B, Mixtral 8x7B — fastest inference via LPU) |
| `synaptic-mistral` | Mistral AI (Mistral Large, Codestral, Mistral NeMo) |
| `synaptic-deepseek` | DeepSeek (V3 chat + R1 reasoning — ultra-low cost) |

### Embeddings

| Crate | Provider |
|-------|----------|
| `synaptic-openai` | OpenAI `text-embedding-3-small/large` |
| `synaptic-ollama` | Ollama local embedding models |
| `synaptic-cohere` | Cohere `embed-english-v3.0`, `embed-multilingual-v3.0` |
| `synaptic-huggingface` | HuggingFace Inference API (BAAI/bge, sentence-transformers…) |

### Vector Stores

| Crate | Backend |
|-------|---------|
| `synaptic-vectorstores` | In-memory (cosine similarity) |
| `synaptic-qdrant` | Qdrant |
| `synaptic-postgres` | PostgreSQL |
| `synaptic-pinecone` | Pinecone |
| `synaptic-chroma` | Chroma |
| `synaptic-mongodb` | MongoDB Atlas Vector Search |
| `synaptic-elasticsearch` | Elasticsearch |
| `synaptic-weaviate` | Weaviate |

### Store, Cache & Graph Persistence

| Crate | Backend |
|-------|---------|
| `synaptic-redis` | Redis Store + LLM Cache + Graph Checkpointer |
| `synaptic-postgres` | PostgreSQL (Store + Cache + VectorStore + Graph Checkpointer) |
| `synaptic-sqlite` | SQLite LLM Cache |

### Tools

| Crate | Tools |
|-------|-------|
| `synaptic-tavily` | Tavily AI search (API key required) |
| `synaptic-tools` | DuckDuckGo search, Wikipedia (no API key required) |
| `synaptic-sqltoolkit` | ListTables, DescribeTable, ExecuteQuery (read-only SQL) |

## Examples

```bash
cargo run -p tool_calling_basic   # Tool registry and execution
cargo run -p memory_chat          # Session-based conversation memory
cargo run -p react_basic          # ReAct agent with tool calling
cargo run -p graph_visualization  # Graph state machine visualization
cargo run -p lcel_chain           # LCEL pipe composition and parallel
cargo run -p prompt_parser_chain  # Prompt template -> model -> parser
cargo run -p streaming            # Streaming chat and runnables
cargo run -p rag_pipeline         # RAG: load -> split -> embed -> retrieve
cargo run -p memory_strategy      # Memory strategies comparison
cargo run -p structured_output    # Structured output with JSON schema
cargo run -p callbacks_tracing    # Callbacks and tracing
cargo run -p evaluation           # Evaluator pipeline
cargo run -p caching              # LLM response caching
cargo run -p macros_showcase      # Proc-macro usage
```

All examples use `ScriptedChatModel` and `FakeEmbeddings` — no API keys required.

## Documentation

- **Book**: [dnw3.github.io/synaptic](https://dnw3.github.io/synaptic) — tutorials, how-to guides, concepts, integration reference
- **API Reference**: [docs.rs/synaptic](https://docs.rs/synaptic) — full Rustdoc API documentation

## Design Principles

- Core abstractions first, feature crates expanded incrementally
- LangChain concept compatibility with Rust-idiomatic APIs
- All traits are async via `#[async_trait]`, runtime is tokio
- Type-erased composition via `BoxRunnable` with `|` pipe operator
- `Arc<RwLock<_>>` for shared registries, session-keyed memory isolation
- MSRV: 1.88

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, or the [full guide](https://dnw3.github.io/synaptic/contributing.html).

## License

MIT — see [LICENSE](LICENSE) for details.
