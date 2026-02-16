# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Synapse is a Rust agent framework with LangChain-compatible architecture. It provides composable building blocks for AI agents: tool execution, memory, callbacks, retrieval, and evaluation. Phases 1–5 (core refactor, multi-provider model adapters + streaming, LCEL composition, prompt templates + output parsers, document pipeline) are complete; Phase 6 (embeddings + vector stores) is next.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo test --workspace               # Run all tests (all should pass)
cargo test -p synapse-tools          # Test a single crate
cargo test -p synapse-core -- chunk  # Run specific test by name pattern
cargo run -p tool_calling_basic      # Run example binary
cargo clippy --workspace             # Lint
cargo fmt --all -- --check           # Check formatting
```

## Workspace Architecture

16 library crates in `crates/`, 3 example binaries in `examples/`:

**Core layer** — `synapse-core` defines all shared traits and types:
- `ChatModel`, `Tool`, `MemoryStore`, `CallbackHandler`, `Agent` traits
- `Message` enum (`System`, `Human`, `AI`, `Tool` variants) — factory methods: `Message::system()`, `human()`, `ai()`, `ai_with_tool_calls()`, `tool()`; accessors: `content()`, `role()`, `is_*()`, `tool_calls()`, `tool_call_id()`
- `AIMessageChunk` — streaming chunk with `+`/`+=` merge and `into_message()`
- `ChatRequest` (messages + tools), `ChatResponse` (message + usage), `ToolCall`, `ToolDefinition`, `RunEvent`, `TokenUsage`
- `ChatStream` type alias — `Pin<Box<dyn Stream<Item = Result<AIMessageChunk, SynapseError>> + Send>>`
- `RunnableConfig` — runtime config (tags, metadata, max_concurrency, recursion_limit, run_id, run_name)
- `SynapseError` enum (19 variants covering all subsystems)

**Implementation crates** — each implements one core trait:
- `synapse-agents` — `ReActAgentExecutor` (think → tool → observe loop, max_steps guard)
- `synapse-tools` — `ToolRegistry` (Arc<RwLock<HashMap>>) + `SerialToolExecutor`
- `synapse-memory` — `InMemoryStore` (session-keyed message storage)
- `synapse-callbacks` — `RecordingCallback`, `LoggingCallback`
- `synapse-models` — provider adapters (`OpenAiChatModel`, `AnthropicChatModel`, `GeminiChatModel`, `OllamaChatModel`) + `ScriptedChatModel` (test double) + `RetryChatModel`, `RateLimitedChatModel` wrappers + `ProviderBackend` trait (`HttpBackend`, `FakeBackend`)
- `synapse-prompts` — `PromptTemplate` (`{{ variable }}` interpolation), `ChatPromptTemplate` (produces `Vec<Message>` with `MessageTemplate` variants: System/Human/AI/Placeholder), `FewShotChatMessagePromptTemplate` (example-based prompting); all chat templates implement `Runnable`
- `synapse-parsers` — output parsers, all implement `Runnable`: `StrOutputParser` (Message→String), `JsonOutputParser` (String→Value), `StructuredOutputParser<T>` (String→T via serde), `ListOutputParser` (String→Vec<String>, configurable separator), `EnumOutputParser` (validates against allowed values)

**Composition & retrieval crates:**
- `synapse-runnables` — `Runnable<I, O>` trait with `invoke()`/`batch()`/`boxed()`, `BoxRunnable` (type-erased, `|` pipe operator via `BitOr`), composition types: `RunnablePassthrough`, `RunnableLambda`, `RunnableSequence`, `RunnableParallel`, `RunnableBranch`, `RunnableWithFallbacks`
- `synapse-chains` — `SequentialChain` (pipes `BoxRunnable<String, String>` steps with `RunnableConfig`)
- `synapse-retrieval` — `Retriever` trait + `InMemoryRetriever`; `Document` has `id`, `content`, `metadata: HashMap<String, Value>`
- `synapse-loaders` — `Loader` async trait, `TextLoader`, `JsonLoader` (configurable id/content keys), `CsvLoader` (column-based with metadata), `DirectoryLoader` (glob filtering, recursive)
- `synapse-splitters` — `TextSplitter` trait with `split_text()`/`split_documents()`, `CharacterTextSplitter`, `RecursiveCharacterTextSplitter` (hierarchical separators), `MarkdownHeaderTextSplitter` (header-aware with metadata)
- `synapse-guardrails` — `JsonObjectGuard` (validates JSON shape)
- `synapse-eval` — `EvalCase`/`EvalReport` (accuracy metrics)

## Key Patterns

- **Message is a tagged enum** — `#[serde(tag = "role")]` with variants `System`, `Human`, `AI` (carries `tool_calls`), `Tool` (carries `tool_call_id`). Use factory methods, not struct literals.
- **ChatModel has streaming** — `chat()` for single response, `stream_chat()` returns `ChatStream` (default impl wraps `chat()` as single chunk).
- **ChatRequest uses constructor** — `ChatRequest::new(messages)`, optional `.with_tools(tools)`. Never use struct literal.
- **Provider adapters use ProviderBackend** — `HttpBackend` for production, `FakeBackend` for tests. Adapters map Synapse types ↔ provider JSON.
- **All traits are async** via `#[async_trait]`. Tests use `#[tokio::test]`.
- **Concurrency**: `Arc<RwLock<_>>` for registries, `Arc<tokio::sync::Mutex<_>>` for callbacks/memory.
- **Session isolation**: Memory and agent runs are keyed by `session_id`.
- **Event-driven callbacks**: `RunEvent` enum fired at each agent lifecycle stage.
- **LCEL pipe composition** — `let chain = step1.boxed() | step2.boxed();` via `BitOr` on `BoxRunnable`. `RunnableLambda::new(|x| async { Ok(transform(x)) })` wraps async closures. `RunnableParallel` runs named branches concurrently, merges to `serde_json::Value`. `RunnableBranch` routes by condition with default fallthrough.
- **Each crate has `tests/` directory** with integration-style tests in separate files.

## Workspace Dependencies (shared via `[workspace.dependencies]`)

async-trait, serde/serde_json, thiserror 2.0, tokio (macros + rt-multi-thread + sync + time), tracing/tracing-subscriber, reqwest (json + stream), futures, async-stream, eventsource-stream, bytes, csv. Rust edition 2021, MSRV 1.78.

## Development Roadmap

Full 12-phase plan in `docs/plans/2026-02-16-synapse-full-langchain-parity-design.md`. Phase 1 implementation details in `docs/plans/2026-02-16-phase1-core-refactor.md`.
