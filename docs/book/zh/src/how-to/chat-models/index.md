# Chat Models

Synaptic 通过 `synaptic_core` 中定义的 `ChatModel` trait 支持多种 LLM 提供商。每个提供商适配器都实现了该 trait，为您提供统一的消息发送和响应接收接口——无论您使用的是 OpenAI、Anthropic、Gemini 还是本地 Ollama 实例。

## 提供商

每个提供商适配器接受一个配置结构体和一个 `ProviderBackend`：

| 提供商 | 适配器 | 配置 |
|----------|---------|--------|
| OpenAI | `OpenAiChatModel` | `OpenAiConfig::new(api_key, model)` |
| Anthropic | `AnthropicChatModel` | `AnthropicConfig::new(api_key, model)` |
| Google Gemini | `GeminiChatModel` | `GeminiConfig::new(api_key, model)` |
| Ollama (本地) | `OllamaChatModel` | `OllamaConfig::new(model)` |

```rust
use std::sync::Arc;
use synaptic_models::{OpenAiChatModel, OpenAiConfig, HttpBackend};

let config = OpenAiConfig::new("sk-...", "gpt-4o");
let backend = Arc::new(HttpBackend::new());
let model = OpenAiChatModel::new(config, backend);
```

测试时，可使用 `ScriptedChatModel`（返回预定义响应）或 `FakeBackend`（无需网络调用即可模拟 HTTP 响应）。

## 包装器

Synaptic 提供可组合的包装器，为任意 `ChatModel` 添加额外行为：

| 包装器 | 用途 |
|---------|---------|
| `RetryChatModel` | 带指数退避的自动重试 |
| `RateLimitedChatModel` | 基于并发的速率限制（信号量） |
| `TokenBucketChatModel` | 令牌桶速率限制 |
| `StructuredOutputChatModel<T>` | 结构化输出的 JSON Schema 约束 |
| `CachedChatModel` | 响应缓存（精确匹配或语义匹配） |
| `BoundToolsChatModel` | 自动为每个请求附加 Tool 定义 |

所有包装器都实现了 `ChatModel`，因此可以层层叠加：

```rust
use std::sync::Arc;
use synaptic_models::{RetryChatModel, RetryPolicy, RateLimitedChatModel};

let model: Arc<dyn ChatModel> = Arc::new(base_model);
let with_retry = Arc::new(RetryChatModel::new(model, RetryPolicy::default()));
let with_rate_limit = RateLimitedChatModel::new(with_retry, 5);
```

## 指南

- [流式响应](streaming.md) -- 使用 `stream_chat()` 逐步消费生成的 token
- [为模型绑定 Tool](bind-tools.md) -- 随请求发送 Tool 定义
- [控制 Tool Choice](tool-choice.md) -- 强制、禁止或指定特定 Tool 的使用
- [Structured Output](structured-output.md) -- 从 LLM 响应中获取类型化的 Rust 结构体
- [缓存 LLM 响应](caching.md) -- 通过内存缓存或语义缓存避免重复 API 调用
- [重试与速率限制](retry-rate-limit.md) -- 处理临时故障并控制请求吞吐量
- [Model Profiles](model-profiles.md) -- 在运行时查询模型的能力与限制
