# Chat Models

Synaptic supports multiple LLM providers through the `ChatModel` trait defined in `synaptic_core`. Every provider adapter implements this trait, giving you a uniform interface for sending messages and receiving responses -- whether you are using OpenAI, Anthropic, Gemini, or a local Ollama instance.

## Providers

Each provider adapter takes a configuration struct and a `ProviderBackend`:

| Provider | Adapter | Config |
|----------|---------|--------|
| OpenAI | `OpenAiChatModel` | `OpenAiConfig::new(api_key, model)` |
| Anthropic | `AnthropicChatModel` | `AnthropicConfig::new(api_key, model)` |
| Google Gemini | `GeminiChatModel` | `GeminiConfig::new(api_key, model)` |
| Ollama (local) | `OllamaChatModel` | `OllamaConfig::new(model)` |

```rust
use std::sync::Arc;
use synaptic_models::{OpenAiChatModel, OpenAiConfig, HttpBackend};

let config = OpenAiConfig::new("sk-...", "gpt-4o");
let backend = Arc::new(HttpBackend::new());
let model = OpenAiChatModel::new(config, backend);
```

For testing, use `ScriptedChatModel` (returns pre-defined responses) or `FakeBackend` (simulates HTTP responses without network calls).

## Wrappers

Synaptic provides composable wrappers that add behavior on top of any `ChatModel`:

| Wrapper | Purpose |
|---------|---------|
| `RetryChatModel` | Automatic retry with exponential backoff |
| `RateLimitedChatModel` | Concurrency-based rate limiting (semaphore) |
| `TokenBucketChatModel` | Token bucket rate limiting |
| `StructuredOutputChatModel<T>` | JSON schema enforcement for structured output |
| `CachedChatModel` | Response caching (exact-match or semantic) |
| `BoundToolsChatModel` | Automatically attach tool definitions to every request |

All wrappers implement `ChatModel`, so they can be stacked:

```rust
use std::sync::Arc;
use synaptic_models::{RetryChatModel, RetryPolicy, RateLimitedChatModel};

let model: Arc<dyn ChatModel> = Arc::new(base_model);
let with_retry = Arc::new(RetryChatModel::new(model, RetryPolicy::default()));
let with_rate_limit = RateLimitedChatModel::new(with_retry, 5);
```

## Guides

- [Streaming Responses](streaming.md) -- consume tokens as they arrive with `stream_chat()`
- [Bind Tools to a Model](bind-tools.md) -- send tool definitions alongside your request
- [Control Tool Choice](tool-choice.md) -- force, prevent, or target specific tool usage
- [Structured Output](structured-output.md) -- get typed Rust structs from LLM responses
- [Caching LLM Responses](caching.md) -- avoid redundant API calls with in-memory or semantic caching
- [Retry & Rate Limiting](retry-rate-limit.md) -- handle transient failures and control request throughput
- [Model Profiles](model-profiles.md) -- query model capabilities and limits at runtime
