# Installation

## Requirements

- **Rust edition**: 2021
- **Minimum supported Rust version (MSRV)**: 1.78
- **Runtime**: Tokio (async runtime)

## Adding Synaptic to Your Project

The `synaptic` facade crate re-exports all sub-crates. Use **feature flags** to control which modules are compiled.

### Feature Flags

Synaptic provides fine-grained feature flags, similar to tokio:

```toml
[dependencies]
# Full — everything enabled (equivalent to previous default)
synaptic = { version = "0.1", features = ["full"] }

# Agent development (models, tools, graph, memory, etc.)
synaptic = { version = "0.1", features = ["agent"] }

# RAG applications (retrieval, loaders, splitters, embeddings, vectorstores, etc.)
synaptic = { version = "0.1", features = ["rag"] }

# Agent + RAG
synaptic = { version = "0.1", features = ["agent", "rag"] }

# Minimal — just model calls
synaptic = { version = "0.1", features = ["models"] }

# Fine-grained control
synaptic = { version = "0.1", features = ["models", "graph", "cache"] }
```

| Feature | Description |
|---------|-------------|
| **`default`** | `models`, `runnables`, `prompts`, `parsers`, `tools`, `callbacks` |
| **`agent`** | `default` + `graph`, `memory` |
| **`rag`** | `default` + `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores` |
| **`full`** | All features enabled |

Individual features: `models`, `runnables`, `prompts`, `parsers`, `tools`, `memory`, `callbacks`, `retrieval`, `loaders`, `splitters`, `embeddings`, `vectorstores`, `graph`, `cache`, `eval`, `store`, `middleware`, `mcp`, `macros`, `deep`.

| Feature | Description |
|---------|-------------|
| `store` | Key-value store with namespace hierarchy and optional semantic search |
| `middleware` | Agent middleware chain (tool call limits, HITL, summarization, context editing) |
| `mcp` | Model Context Protocol client (Stdio/SSE/HTTP transports) |
| `macros` | Proc macros (`#[tool]`, `#[chain]`, `#[entrypoint]`, `#[traceable]`) |
| `deep` | Deep agent harness (backends, filesystem tools, sub-agents, skills) |

The `core` module (traits and types) is always available regardless of feature selection.

### Quick Start Examples

**From a local checkout** (during development or before the crate is published):

```toml
[dependencies]
synaptic = { path = "path/to/synaptic/crates/synaptic", features = ["agent"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**Once published to crates.io**:

```toml
[dependencies]
synaptic = { version = "0.1", features = ["agent"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Using the Facade

The facade crate provides namespaced re-exports for all sub-crates. You access types through their module path:

```rust
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::{OpenAiChatModel, ScriptedChatModel};
use synaptic::runnables::{Runnable, BoxRunnable, RunnableLambda};
use synaptic::prompts::ChatPromptTemplate;
use synaptic::parsers::StrOutputParser;
use synaptic::tools::ToolRegistry;
use synaptic::memory::InMemoryStore;
use synaptic::graph::{StateGraph, create_react_agent};
use synaptic::retrieval::Retriever;
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::vectorstores::InMemoryVectorStore;
```

Alternatively, you can depend on individual crates directly if you want to minimize compile times:

```toml
[dependencies]
synaptic-core = { path = "path/to/synaptic/crates/synaptic-core" }
synaptic-models = { path = "path/to/synaptic/crates/synaptic-models" }
```

## Provider API Keys

Synaptic reads API keys from environment variables. Set the ones you need for your chosen provider:

| Provider | Environment Variable |
|---|---|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Google Gemini | `GEMINI_API_KEY` |
| Ollama | No key required (runs locally) |

For example, on a Unix shell:

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export GEMINI_API_KEY="AI..."
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
