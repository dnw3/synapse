# API Reference

Synaptic is organized as a workspace of focused crates. Each crate has its own API documentation generated from doc comments in the source code.

## Crate Reference

| Crate | Description | Docs |
|-------|-------------|------|
| `synaptic-core` | Shared traits and types (`ChatModel`, `Tool`, `Message`, `SynapticError`, etc.) | [docs.rs](https://docs.rs/synaptic-core) |
| `synaptic-models` | `ProviderBackend` abstraction, `ScriptedChatModel` test double, wrappers (retry, rate limit, structured output, bound tools) | [docs.rs](https://docs.rs/synaptic-models) |
| `synaptic-openai` | OpenAI provider (`OpenAiChatModel`, `OpenAiEmbeddings`) | [docs.rs](https://docs.rs/synaptic-openai) |
| `synaptic-anthropic` | Anthropic provider (`AnthropicChatModel`) | [docs.rs](https://docs.rs/synaptic-anthropic) |
| `synaptic-gemini` | Google Gemini provider (`GeminiChatModel`) | [docs.rs](https://docs.rs/synaptic-gemini) |
| `synaptic-ollama` | Ollama provider (`OllamaChatModel`, `OllamaEmbeddings`) | [docs.rs](https://docs.rs/synaptic-ollama) |
| `synaptic-runnables` | LCEL composition (`Runnable` trait, `BoxRunnable`, pipe operator, parallel, branch, fallbacks, assign, pick) | [docs.rs](https://docs.rs/synaptic-runnables) |
| `synaptic-prompts` | Prompt templates (`PromptTemplate`, `ChatPromptTemplate`, `FewShotChatMessagePromptTemplate`) | [docs.rs](https://docs.rs/synaptic-prompts) |
| `synaptic-parsers` | Output parsers (string, JSON, structured, list, enum, boolean, XML, fixing, retry) | [docs.rs](https://docs.rs/synaptic-parsers) |
| `synaptic-tools` | Tool system (`ToolRegistry`, `SerialToolExecutor`, `ParallelToolExecutor`) | [docs.rs](https://docs.rs/synaptic-tools) |
| `synaptic-memory` | Memory strategies (buffer, window, summary, token buffer, summary buffer, `RunnableWithMessageHistory`) | [docs.rs](https://docs.rs/synaptic-memory) |
| `synaptic-callbacks` | Callback handlers (`RecordingCallback`, `TracingCallback`, `CompositeCallback`) | [docs.rs](https://docs.rs/synaptic-callbacks) |
| `synaptic-retrieval` | Retriever implementations (in-memory, BM25, multi-query, ensemble, contextual compression, self-query, parent document) | [docs.rs](https://docs.rs/synaptic-retrieval) |
| `synaptic-loaders` | Document loaders (text, JSON, CSV, directory, file, markdown, web) | [docs.rs](https://docs.rs/synaptic-loaders) |
| `synaptic-splitters` | Text splitters (character, recursive character, markdown header, token, HTML header, language) | [docs.rs](https://docs.rs/synaptic-splitters) |
| `synaptic-embeddings` | Embeddings trait, `FakeEmbeddings`, `CacheBackedEmbeddings` | [docs.rs](https://docs.rs/synaptic-embeddings) |
| `synaptic-vectorstores` | Vector store implementations (`InMemoryVectorStore`, `VectorStoreRetriever`, `MultiVectorRetriever`) | [docs.rs](https://docs.rs/synaptic-vectorstores) |
| `synaptic-qdrant` | Qdrant vector store (`QdrantVectorStore`) | [docs.rs](https://docs.rs/synaptic-qdrant) |
| `synaptic-postgres` | PostgreSQL integration (`PgVectorStore`, `PgStore`, `PgCache`, `PgCheckpointer`) | [docs.rs](https://docs.rs/synaptic-postgres) |
| `synaptic-redis` | Redis store and cache (`RedisStore`, `RedisCache`) | [docs.rs](https://docs.rs/synaptic-redis) |
| `synaptic-pdf` | PDF document loader (`PdfLoader`) | [docs.rs](https://docs.rs/synaptic-pdf) |
| `synaptic-graph` | Graph orchestration (`StateGraph`, `CompiledGraph`, `ToolNode`, `create_react_agent`, checkpointing, streaming) | [docs.rs](https://docs.rs/synaptic-graph) |
| `synaptic-cache` | LLM caching (`InMemoryCache`, `SemanticCache`, `CachedChatModel`) | [docs.rs](https://docs.rs/synaptic-cache) |
| `synaptic-eval` | Evaluation framework (exact match, regex, JSON validity, embedding distance, LLM judge evaluators; `Dataset` and `evaluate()`) | [docs.rs](https://docs.rs/synaptic-eval) |
| `synaptic` | Unified facade crate that re-exports all sub-crates under a single namespace | [docs.rs](https://docs.rs/synaptic) |

> **Note:** The docs.rs links above will become active once the crates are published to crates.io. In the meantime, generate local documentation as described below.

## Local API Documentation

You can generate and browse the full API documentation locally with:

```bash
cargo doc --workspace --open
```

This builds rustdoc for every crate in the workspace and opens the result in your browser. The generated documentation includes all public types, traits, functions, and their doc comments.

To generate docs without opening the browser (useful in CI):

```bash
cargo doc --workspace --no-deps
```

## Using the Facade Crate

If you prefer a single dependency instead of listing individual crates, use the `synaptic` facade:

```toml
[dependencies]
synaptic = "0.2"
```

Then import through the unified namespace:

```rust
use synaptic::core::Message;
use synaptic::openai::OpenAiChatModel;   // requires "openai" feature
use synaptic::models::ScriptedChatModel; // requires "model-utils" feature
use synaptic::graph::create_react_agent;
use synaptic::runnables::Runnable;
```
