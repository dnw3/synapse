# 重试与速率限制

本指南展示如何为任意 `ChatModel` 添加自动重试逻辑和速率限制。

## 使用 `RetryChatModel` 进行重试

`RetryChatModel` 包装模型并在遇到临时故障（速率限制错误和超时）时自动重试。重试之间使用指数退避策略。

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_models::{RetryChatModel, RetryPolicy};

let base_model: Arc<dyn ChatModel> = Arc::new(model);

// Use default policy: 3 attempts, 500ms base delay
let retry_model = RetryChatModel::new(base_model, RetryPolicy::default());
```

### 自定义重试策略

配置最大尝试次数和指数退避的基础延迟：

```rust
use std::time::Duration;
use synaptic_models::RetryPolicy;

let policy = RetryPolicy {
    max_attempts: 5,                         // Try up to 5 times
    base_delay: Duration::from_millis(200),  // Start with 200ms delay
};

let retry_model = RetryChatModel::new(base_model, policy);
```

重试之间的延迟遵循指数退避：`base_delay * 2^attempt`。以 200ms 基础延迟为例：

| 尝试次数 | 重试前延迟 |
|---------|-------------------|
| 第 1 次重试 | 200ms |
| 第 2 次重试 | 400ms |
| 第 3 次重试 | 800ms |
| 第 4 次重试 | 1600ms |

仅可重试的错误会触发重试：
- `SynapticError::RateLimit` -- 提供商返回了速率限制响应。
- `SynapticError::Timeout` -- 请求超时。

所有其他错误会立即返回，不进行重试。

### 流式传输中的重试

`RetryChatModel` 也会对 `stream_chat()` 调用进行重试。如果在流式传输过程中发生可重试错误，整个流将从头开始重试。

## 使用 `RateLimitedChatModel` 进行并发限制

`RateLimitedChatModel` 使用信号量来限制对底层模型的并发请求数：

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_models::RateLimitedChatModel;

let base_model: Arc<dyn ChatModel> = Arc::new(model);

// Allow at most 5 concurrent requests
let limited = RateLimitedChatModel::new(base_model, 5);
```

当达到并发限制时，额外的调用者会等待直到有空闲槽位。这在以下场景中很有用：

- 遵守提供商的并发限制。
- 防止高吞吐量应用中的资源耗尽。
- 通过限制并行 API 调用来控制成本。

## 使用 `TokenBucketChatModel` 进行令牌桶速率限制

`TokenBucketChatModel` 使用令牌桶算法实现更平滑的速率限制。桶在初始时是满的，并以稳定速率补充：

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_models::TokenBucketChatModel;

let base_model: Arc<dyn ChatModel> = Arc::new(model);

// Bucket capacity: 100 tokens, refill rate: 10 tokens/second
let throttled = TokenBucketChatModel::new(base_model, 100.0, 10.0);
```

每次 `chat()` 或 `stream_chat()` 调用会从桶中消耗一个令牌。当桶为空时，调用者会等待令牌补充。

参数说明：
- **capacity** -- 最大突发量。容量为 100 表示在限流生效前可以连续发送 100 个快速请求。
- **refill_rate** -- 每秒添加的令牌数。速率为 10.0 表示桶以每秒 10 个令牌的速度补充。

### 令牌桶与并发限制的对比

| 特性 | `RateLimitedChatModel` | `TokenBucketChatModel` |
|---------|----------------------|----------------------|
| 控制对象 | 并发请求数 | 一段时间内的请求速率 |
| 机制 | 信号量 | 令牌桶 |
| 突发处理 | 当 N 个请求正在处理时阻塞 | 允许突发直到容量上限 |
| 适用场景 | 并发限制 | 速率限制（请求/秒） |

## 叠加包装器

所有包装器都实现了 `ChatModel`，因此可以自然组合。常见模式是在外层添加重试，内层添加速率限制：

```rust
use std::sync::Arc;
use synaptic_core::ChatModel;
use synaptic_models::{RetryChatModel, RetryPolicy, TokenBucketChatModel};

let base_model: Arc<dyn ChatModel> = Arc::new(model);

// First, apply rate limiting
let throttled: Arc<dyn ChatModel> = Arc::new(
    TokenBucketChatModel::new(base_model, 50.0, 5.0)
);

// Then, add retry on top
let reliable = RetryChatModel::new(throttled, RetryPolicy::default());
```

这确保重试的请求也经过速率限制器，防止重试风暴压垮提供商。
