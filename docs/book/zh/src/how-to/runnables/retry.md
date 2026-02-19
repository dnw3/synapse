# Retry

本指南展示如何使用 `RunnableRetry` 和 `RetryPolicy` 在失败时自动以指数退避重试 runnable。

## 概述

`RunnableRetry` 为任意 runnable 包装重试逻辑。当内部 runnable 返回错误时，`RunnableRetry` 等待退避延迟后重试，最多可配置的尝试次数。退避遵循指数调度：`min(base_delay * 2^attempt, max_delay)`。

## 基本用法

```rust
use std::time::Duration;
use synaptic_runnables::{Runnable, RunnableRetry, RetryPolicy, RunnableLambda};
use synaptic_core::RunnableConfig;

let flaky_step = RunnableLambda::new(|x: String| async move {
    // Imagine this sometimes fails due to network issues
    Ok(x.to_uppercase())
});

let policy = RetryPolicy::default();  // 3 attempts, 100ms base delay, 10s max delay

let with_retry = RunnableRetry::new(flaky_step.boxed(), policy);

let config = RunnableConfig::default();
let result = with_retry.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "HELLO");
```

## 配置重试策略

`RetryPolicy` 使用构建器模式进行配置：

```rust
use std::time::Duration;
use synaptic_runnables::RetryPolicy;

let policy = RetryPolicy::default()
    .with_max_attempts(5)               // Up to 5 total attempts (1 initial + 4 retries)
    .with_base_delay(Duration::from_millis(200))   // Start with 200ms delay
    .with_max_delay(Duration::from_secs(30));      // Cap delay at 30 seconds
```

### 默认值

| 字段 | 默认值 |
|-------|---------|
| `max_attempts` | 3 |
| `base_delay` | 100ms |
| `max_delay` | 10 秒 |

### 退避调度

每次重试的延迟按以下公式计算：

```
delay = min(base_delay * 2^attempt, max_delay)
```

对于默认值（100ms 基础延迟，10s 最大延迟）：

| 尝试次数 | 延迟 |
|---------|-------|
| 第 1 次重试（attempt 0） | 100ms |
| 第 2 次重试（attempt 1） | 200ms |
| 第 3 次重试（attempt 2） | 400ms |
| 第 4 次重试（attempt 3） | 800ms |
| ... | ... |
| 上限 | 10s |

## 过滤可重试的错误

默认情况下，所有错误都会触发重试。使用 `with_retry_on()` 指定一个谓词，决定哪些错误值得重试：

```rust
use synaptic_runnables::RetryPolicy;
use synaptic_core::SynapticError;

let policy = RetryPolicy::default()
    .with_max_attempts(4)
    .with_retry_on(|error: &SynapticError| {
        // Only retry provider errors (e.g., rate limits, timeouts)
        matches!(error, SynapticError::Provider(_))
    });
```

当谓词对某个错误返回 `false` 时，`RunnableRetry` 会立即返回该错误而不再重试。

## 输入克隆要求

输入类型必须实现 `Clone`，因为输入会在每次重试尝试时复用：

```rust
pub struct RunnableRetry<I: Send + Clone + 'static, O: Send + 'static> { ... }
```

## 在链中使用

`RunnableRetry` 实现了 `Runnable<I, O>`，因此它可以与 pipe 运算符配合使用：

```rust
use synaptic_runnables::{Runnable, RunnableRetry, RetryPolicy, RunnableLambda};

let preprocess = RunnableLambda::new(|x: String| async move {
    Ok(x.trim().to_string())
});

let retrying_model = RunnableRetry::new(
    model_step.boxed(),
    RetryPolicy::default().with_max_attempts(3),
);

let chain = preprocess.boxed() | retrying_model.boxed();
```

## 组合 retry 与 fallbacks

为了最大程度的弹性，可以用 fallbacks 包装带重试的 runnable。主 runnable 会重试到其限制次数；如果仍然失败，则尝试回退：

```rust
use synaptic_runnables::{RunnableRetry, RetryPolicy, RunnableWithFallbacks};

let retrying_primary = RunnableRetry::new(
    primary_model.boxed(),
    RetryPolicy::default().with_max_attempts(3),
);

let resilient = RunnableWithFallbacks::new(
    retrying_primary.boxed(),
    vec![fallback_model.boxed()],
);
```

## 完整示例

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use synaptic_runnables::{Runnable, RunnableRetry, RetryPolicy, RunnableLambda};
use synaptic_core::{RunnableConfig, SynapticError};

// Simulate a flaky service that fails twice then succeeds
let call_count = Arc::new(AtomicUsize::new(0));
let counter = call_count.clone();

let flaky = RunnableLambda::new(move |x: String| {
    let counter = counter.clone();
    async move {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Err(SynapticError::Provider("temporary failure".into()))
        } else {
            Ok(format!("Success: {x}"))
        }
    }
});

let policy = RetryPolicy::default()
    .with_max_attempts(5)
    .with_base_delay(Duration::from_millis(10));

let retrying = RunnableRetry::new(flaky.boxed(), policy);

let config = RunnableConfig::default();
let result = retrying.invoke("test".to_string(), &config).await?;
assert_eq!(result, "Success: test");
assert_eq!(call_count.load(Ordering::SeqCst), 3);  // 2 failures + 1 success
```
