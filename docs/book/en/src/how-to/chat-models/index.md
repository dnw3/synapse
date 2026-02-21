# Chat Models

Synaptic supports multiple LLM providers through the `ChatModel` trait defined in `synaptic-core`. Each provider lives in its own crate, giving you a uniform interface for sending messages and receiving responses -- whether you are using OpenAI, Anthropic, Gemini, or a local Ollama instance.

## Providers

Each provider adapter lives in its own crate. You enable only the providers you need via feature flags:

| Provider | Adapter | Crate | Feature |
|----------|---------|-------|---------|
| OpenAI | `OpenAiChatModel` | `synaptic-openai` | `"openai"` |
| Anthropic | `AnthropicChatModel` | `synaptic-anthropic` | `"anthropic"` |
| Google Gemini | `GeminiChatModel` | `synaptic-gemini` | `"gemini"` |
| Ollama (local) | `OllamaChatModel` | `synaptic-ollama` | `"ollama"` |

```rust
use std::sync::Arc;
use synaptic::openai::OpenAiChatModel;

let model = OpenAiChatModel::new("gpt-4o");
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
use synaptic::models::{RetryChatModel, RetryPolicy, RateLimitedChatModel};

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
