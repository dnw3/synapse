# Migrating from 0.2 to 0.3

This guide covers the breaking changes in Synaptic 0.3 and how to update your code. The main theme of this release is the **integrations architecture refactor**: provider-specific types have been extracted into independent crates, and core traits have been consolidated.

## Overview of Changes

1. Provider implementations (OpenAI, Anthropic, Gemini, Ollama) moved to their own crates.
2. Import paths changed for all provider types.
3. Feature flags refined -- you can now enable individual providers without pulling in the others.
4. Core traits (`Document`, `Retriever`, `VectorStore`, `Loader`, `LlmCache`) consolidated into `synaptic-core`.
5. New integration crates added: Qdrant, pgvector, Redis, PDF.

## Cargo.toml Changes

Update your version from `0.2` to `0.3`:

```toml
# Before
synaptic = { version = "0.2", features = ["models"] }

# After — same behavior (all 4 providers)
synaptic = { version = "0.3", features = ["models"] }

# After — pick only the providers you need
synaptic = { version = "0.3", features = ["openai", "anthropic"] }
```

The `models` feature still exists as a convenience that enables all four providers. However, you no longer need it if you only use one or two providers. Enabling `openai` alone is sufficient for OpenAI models and embeddings.

### Feature Flag Reference

| Feature | What it enables |
|---------|----------------|
| `openai` | `OpenAiChatModel`, `OpenAiConfig`, `OpenAiEmbeddings`, `OpenAiEmbeddingsConfig` |
| `anthropic` | `AnthropicChatModel`, `AnthropicConfig` |
| `gemini` | `GeminiChatModel`, `GeminiConfig` |
| `ollama` | `OllamaChatModel`, `OllamaConfig`, `OllamaEmbeddings`, `OllamaEmbeddingsConfig` |
| `models` | All four provider features above |
| `model-utils` | `ProviderBackend`, `HttpBackend`, `FakeBackend`, `ScriptedChatModel`, `RetryChatModel`, `RateLimitedChatModel`, `TokenBucketChatModel`, `StructuredOutputChatModel`, `BoundToolsChatModel` |
| `qdrant` | Qdrant vector store integration |
| `pgvector` | PostgreSQL pgvector integration |
| `redis` | Redis store and cache integration |
| `pdf` | PDF document loader |

The `model-utils` feature gives you access to `synaptic::models` (provider-agnostic wrappers and test doubles) without pulling in any provider. In 0.2, the `models` feature flag was required for these utilities; in 0.3, `model-utils` is the minimal option.

## Import Path Changes

This is the most common change you will encounter. All provider-specific types have moved from `synaptic::models` and `synaptic::embeddings` into provider-namespaced modules.

### Chat Models

```rust
// Before (0.2)
use synaptic::models::OpenAiChatModel;
use synaptic::models::OpenAiConfig;
use synaptic::models::AnthropicChatModel;
use synaptic::models::AnthropicConfig;
use synaptic::models::GeminiChatModel;
use synaptic::models::GeminiConfig;
use synaptic::models::OllamaChatModel;
use synaptic::models::OllamaConfig;

// After (0.3)
use synaptic::openai::OpenAiChatModel;
use synaptic::openai::OpenAiConfig;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::anthropic::AnthropicConfig;
use synaptic::gemini::GeminiChatModel;
use synaptic::gemini::GeminiConfig;
use synaptic::ollama::OllamaChatModel;
use synaptic::ollama::OllamaConfig;
```

### Embeddings

```rust
// Before (0.2)
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::embeddings::OpenAiEmbeddingsConfig;
use synaptic::embeddings::OllamaEmbeddings;
use synaptic::embeddings::OllamaEmbeddingsConfig;

// After (0.3)
use synaptic::openai::OpenAiEmbeddings;
use synaptic::openai::OpenAiEmbeddingsConfig;
use synaptic::ollama::OllamaEmbeddings;
use synaptic::ollama::OllamaEmbeddingsConfig;
```

### Provider-Agnostic Utilities (Unchanged)

Types that are not provider-specific remain in `synaptic::models`:

```rust
// These paths are the same in 0.2 and 0.3
use synaptic::models::ScriptedChatModel;
use synaptic::models::RetryChatModel;
use synaptic::models::RateLimitedChatModel;
use synaptic::models::TokenBucketChatModel;
use synaptic::models::StructuredOutputChatModel;
use synaptic::models::BoundToolsChatModel;
use synaptic::models::ProviderBackend;
use synaptic::models::HttpBackend;
use synaptic::models::FakeBackend;
```

These require the `model-utils` feature (or any provider feature, which implies `model-utils`).

## Quick Migration Recipes

### Recipe 1: OpenAI-only Application

```toml
# Cargo.toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "graph", "memory"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
// main.rs
use synaptic::openai::OpenAiChatModel;   // was synaptic::models::OpenAiChatModel
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::graph::create_react_agent;
```

### Recipe 2: Multi-Provider Application

```toml
# Cargo.toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "anthropic", "graph"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use synaptic::openai::OpenAiChatModel;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::core::ChatModel;
```

### Recipe 3: RAG with Qdrant

```toml
# Cargo.toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "retrieval", "loaders", "splitters", "qdrant"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use synaptic::openai::{OpenAiChatModel, OpenAiEmbeddings};
use synaptic::qdrant::QdrantVectorStore;
use synaptic::retrieval::Retriever;
use synaptic::loaders::TextLoader;
use synaptic::splitters::RecursiveCharacterTextSplitter;
```

### Recipe 4: Testing Without a Provider

```toml
# Cargo.toml — no provider feature needed
[dependencies]
synaptic = { version = "0.3", features = ["model-utils", "graph"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use synaptic::models::ScriptedChatModel;
use synaptic::core::{ChatModel, Message};
```

## Core Trait Consolidation

In 0.3, several traits that were previously defined only in their implementation crates have been promoted to `synaptic-core`:

- `Document` (was only in `synaptic-retrieval`)
- `Retriever` (was only in `synaptic-retrieval`)
- `VectorStore` (was only in `synaptic-vectorstores`)
- `Loader` (was only in `synaptic-loaders`)
- `LlmCache` (was only in `synaptic-cache`)

This enables integration crates (Qdrant, pgvector, etc.) to depend on `synaptic-core` alone, without pulling in the full implementation crates.

**For end users, this is not a breaking change.** The original crates (`synaptic-retrieval`, `synaptic-vectorstores`, `synaptic-loaders`, `synaptic-cache`) continue to re-export these traits. Your existing imports via `synaptic::retrieval::Retriever` or `synaptic::vectorstores::VectorStore` will continue to work.

If you depend on sub-crates directly (not through the facade), you can now import these traits from `synaptic-core` instead:

```rust
// Both work in 0.3:
use synaptic::core::Retriever;       // newly available
use synaptic::retrieval::Retriever;  // still works (re-export)
```

## New Integration Crates

Synaptic 0.3 introduces optional integration crates for external services:

| Crate | Feature | Description |
|-------|---------|-------------|
| `synaptic-qdrant` | `qdrant` | Qdrant vector database adapter implementing `VectorStore` |
| `synaptic-pgvector` | `pgvector` | PostgreSQL with pgvector extension implementing `VectorStore` |
| `synaptic-redis` | `redis` | Redis-backed store and LLM cache |
| `synaptic-pdf` | `pdf` | PDF document loader implementing `Loader` |

Enable them via feature flags:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["openai", "qdrant", "pdf"] }
```

## Search-and-Replace Cheat Sheet

If you prefer a mechanical migration, here are the import substitutions to apply across your codebase:

| Find | Replace |
|------|---------|
| `synaptic::models::OpenAiChatModel` | `synaptic::openai::OpenAiChatModel` |
| `synaptic::models::OpenAiConfig` | `synaptic::openai::OpenAiConfig` |
| `synaptic::models::AnthropicChatModel` | `synaptic::anthropic::AnthropicChatModel` |
| `synaptic::models::AnthropicConfig` | `synaptic::anthropic::AnthropicConfig` |
| `synaptic::models::GeminiChatModel` | `synaptic::gemini::GeminiChatModel` |
| `synaptic::models::GeminiConfig` | `synaptic::gemini::GeminiConfig` |
| `synaptic::models::OllamaChatModel` | `synaptic::ollama::OllamaChatModel` |
| `synaptic::models::OllamaConfig` | `synaptic::ollama::OllamaConfig` |
| `synaptic::embeddings::OpenAiEmbeddings` | `synaptic::openai::OpenAiEmbeddings` |
| `synaptic::embeddings::OpenAiEmbeddingsConfig` | `synaptic::openai::OpenAiEmbeddingsConfig` |
| `synaptic::embeddings::OllamaEmbeddings` | `synaptic::ollama::OllamaEmbeddings` |
| `synaptic::embeddings::OllamaEmbeddingsConfig` | `synaptic::ollama::OllamaEmbeddingsConfig` |

All other imports (`synaptic::core::*`, `synaptic::graph::*`, `synaptic::tools::*`, `synaptic::models::ScriptedChatModel`, etc.) remain unchanged.
