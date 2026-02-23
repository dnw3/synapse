# Installation

## Requirements

- **Rust edition**: 2021
- **Minimum supported Rust version (MSRV)**: 1.88
- **Runtime**: Tokio (async runtime)

## Adding Synaptic to Your Project

The `synaptic` facade crate re-exports all sub-crates. Use **feature flags** to control which modules are compiled.

### Feature Flags

Synaptic provides fine-grained feature flags, similar to tokio:

```toml
[dependencies]
# Full — everything enabled (equivalent to previous default)
synaptic = { version = "0.2", features = ["full"] }

# Agent development (OpenAI + tools + graph + memory, etc.)
synaptic = { version = "0.2", features = ["agent"] }

# RAG applications (OpenAI + retrieval + loaders + splitters + embeddings + vectorstores, etc.)
synaptic = { version = "0.2", features = ["rag"] }

# Agent + RAG
synaptic = { version = "0.2", features = ["agent", "rag"] }

# Just OpenAI model calls
synaptic = { version = "0.2", features = ["openai"] }

# All 4 providers (OpenAI + Anthropic + Gemini + Ollama)
synaptic = { version = "0.2", features = ["models"] }

# Fine-grained: one provider + specific modules
synaptic = { version = "0.2", features = ["anthropic", "graph", "cache"] }
```

**Composite features:**

| Feature | Description |
|---------|-------------|
| **`default`** | `model-utils`, `runnables`, `prompts`, `parsers`, `tools`, `callbacks` |
| **`agent`** | `default` + `openai`, `graph`, `memory` |
| **`rag`** | `default` + `openai`, `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores` |
| **`models`** | All 6 providers: `openai` + `anthropic` + `gemini` + `ollama` + `bedrock` + `cohere` |
| **`full`** | All features enabled |

**Provider features** (each enables one provider crate):

| Feature | Description |
|---------|-------------|
| `openai` | `OpenAiChatModel` + `OpenAiEmbeddings` (`synaptic-openai`) |
| `anthropic` | `AnthropicChatModel` (`synaptic-anthropic`) |
| `gemini` | `GeminiChatModel` (`synaptic-gemini`) |
| `ollama` | `OllamaChatModel` + `OllamaEmbeddings` (`synaptic-ollama`) |

**Module features:**

Individual features: `model-utils`, `runnables`, `prompts`, `parsers`, `tools`, `memory`, `callbacks`, `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores`, `graph`, `cache`, `eval`, `store`, `middleware`, `mcp`, `macros`, `deep`.

| Feature | Description |
|---------|-------------|
| `model-utils` | `ProviderBackend` abstraction, `ScriptedChatModel`, wrappers (`RetryChatModel`, `RateLimitedChatModel`, `StructuredOutputChatModel`, etc.) |
| `store` | Key-value store with namespace hierarchy and optional semantic search |
| `middleware` | Agent middleware chain (tool call limits, HITL, summarization, context editing) |
| `mcp` | Model Context Protocol client (Stdio/SSE/HTTP transports) |
| `macros` | Proc macros (`#[tool]`, `#[chain]`, `#[entrypoint]`, `#[traceable]`) |
| `deep` | Deep agent harness (backends, filesystem tools, sub-agents, skills) |

**Integration features:**

| Feature | Description |
|---------|-------------|
| `qdrant` | Qdrant vector store (`synaptic-qdrant`) |
| `pgvector` | PostgreSQL pgvector store (`synaptic-pgvector`) |
| `redis` | Redis store + cache (`synaptic-redis`) |
| `pdf` | PDF document loader (`synaptic-pdf`) |
| `bedrock` | AWS Bedrock ChatModel (`synaptic-bedrock`) |
| `cohere` | Cohere Reranker (`synaptic-cohere`) |
| `pinecone` | Pinecone vector store (`synaptic-pinecone`) |
| `chroma` | Chroma vector store (`synaptic-chroma`) |
| `mongodb` | MongoDB Atlas vector search (`synaptic-mongodb`) |
| `elasticsearch` | Elasticsearch vector store (`synaptic-elasticsearch`) |
| `sqlite` | SQLite LLM cache (`synaptic-sqlite`) |
| `tavily` | Tavily search tool (`synaptic-tavily`) |

The `core` module (traits and types) is always available regardless of feature selection.

### Quick Start Example

```toml
[dependencies]
synaptic = { version = "0.2", features = ["agent"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Using the Facade

The facade crate provides namespaced re-exports for all sub-crates. You access types through their module path:

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::openai::{OpenAiChatModel, OpenAiEmbeddings};  // requires "openai" feature
use synaptic::anthropic::AnthropicChatModel;                  // requires "anthropic" feature
use synaptic::models::ScriptedChatModel;                      // requires "model-utils" feature
use synaptic::runnables::{Runnable, BoxRunnable, RunnableLambda};
use synaptic::prompts::ChatPromptTemplate;
use synaptic::parsers::StrOutputParser;
use synaptic::tools::ToolRegistry;
use synaptic::memory::ChatMessageHistory;
use synaptic::graph::{StateGraph, create_react_agent};
use synaptic::retrieval::Retriever;
use synaptic::vectorstores::InMemoryVectorStore;
```

Alternatively, you can depend on individual crates directly if you want to minimize compile times:

```toml
[dependencies]
synaptic-core = "0.2"
synaptic-models = "0.2"
```

## Provider API Keys

Synaptic reads API keys from environment variables. Set the ones you need for your chosen provider:

| Provider | Environment Variable |
|---|---|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Google Gemini | `GOOGLE_API_KEY` |
| Ollama | No key required (runs locally) |

For example, on a Unix shell:

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export GOOGLE_API_KEY="AI..."
```

You do not need any API keys to run the [Quickstart](quickstart.md) example, which uses the `ScriptedChatModel` test double.

## Building and Testing

From the workspace root:

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Test a single crate
cargo test -p synaptic-models

# Run a specific test by name
cargo test -p synaptic-core -- trim_messages

# Check formatting
cargo fmt --all -- --check

# Run lints
cargo clippy --workspace
```

## Workspace Dependencies

Synaptic uses Cargo workspace-level dependency management. Key shared dependencies include:

- `async-trait` -- async trait methods
- `serde` / `serde_json` -- serialization
- `thiserror` 2.0 -- error derive
- `tokio` -- async runtime (macros, rt-multi-thread, sync, time)
- `reqwest` -- HTTP client (json, stream features)
- `futures` / `async-stream` -- stream utilities
- `tracing` / `tracing-subscriber` -- structured logging
