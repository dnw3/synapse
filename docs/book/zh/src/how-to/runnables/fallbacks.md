# Fallbacks

本指南展示如何使用 `RunnableWithFallbacks` 提供在主 runnable 失败时尝试的替代 runnable。

## 概述

`RunnableWithFallbacks` 包装一个主 runnable 和一个回退 runnable 列表。调用时，它首先尝试主 runnable。如果主 runnable 返回错误，它按顺序尝试每个回退，直到有一个成功。如果所有回退都失败，它返回最后一个尝试的回退的错误。

这在使用可能遭遇瞬态故障的 LLM 提供商时特别有用，或者当你想先尝试更便宜的模型再回退到更强大的模型时。

## 基本用法

```rust
use synaptic_runnables::{Runnable, RunnableWithFallbacks, RunnableLambda};
use synaptic_core::{RunnableConfig, SynapticError};

// A runnable that always fails
let unreliable = RunnableLambda::new(|_x: String| async move {
    Err::<String, _>(SynapticError::Provider("service unavailable".into()))
});

// A reliable fallback
let fallback = RunnableLambda::new(|x: String| async move {
    Ok(format!("Fallback handled: {x}"))
});

let with_fallbacks = RunnableWithFallbacks::new(
    unreliable.boxed(),
    vec![fallback.boxed()],
);

let config = RunnableConfig::default();
let result = with_fallbacks.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "Fallback handled: hello");
```

## 多个回退

你可以提供多个回退。它们按顺序尝试：

```rust
let primary = failing_model.boxed();
let fallback_1 = cheaper_model.boxed();
let fallback_2 = local_model.boxed();

let resilient = RunnableWithFallbacks::new(
    primary,
    vec![fallback_1, fallback_2],
);

// Tries: primary -> fallback_1 -> fallback_2
let result = resilient.invoke(input, &config).await?;
```

如果主 runnable 成功，则不会尝试任何回退。如果主 runnable 失败但 `fallback_1` 成功，则永远不会尝试 `fallback_2`。

## 输入克隆要求

输入类型必须实现 `Clone`，因为 `RunnableWithFallbacks` 需要将输入的副本传递给每次回退尝试。这由类型签名强制保证：

```rust
pub struct RunnableWithFallbacks<I: Send + Clone + 'static, O: Send + 'static> {
    primary: BoxRunnable<I, O>,
    fallbacks: Vec<BoxRunnable<I, O>>,
}
```

`String`、`Vec<Message>`、`serde_json::Value` 以及大多数标准类型都实现了 `Clone`。

## 使用 fallbacks 进行流式处理

`RunnableWithFallbacks` 也支持 `stream()`。在流式处理时，它会缓冲主流的输出。如果主流产出一个错误，它会丢弃缓冲的项并尝试下一个回退的流。这意味着失败的提供商不会有部分输出——你会从成功的提供商获得完整输出。

```rust
use futures::StreamExt;

let resilient = RunnableWithFallbacks::new(primary.boxed(), vec![fallback.boxed()]);

let mut stream = resilient.stream("input".to_string(), &config);
while let Some(result) = stream.next().await {
    let value = result?;
    println!("Got: {value}");
}
```

## 在链中使用

`RunnableWithFallbacks` 实现了 `Runnable<I, O>`，因此它可以与 pipe 运算符组合：

```rust
let resilient_model = RunnableWithFallbacks::new(
    primary_model.boxed(),
    vec![fallback_model.boxed()],
);

let chain = preprocess.boxed() | resilient_model.boxed() | postprocess.boxed();
```

## 何时使用 fallbacks 与 retry

- 当你有真正不同的替代方案时（例如不同的 LLM 提供商或不同的策略），使用 **`RunnableWithFallbacks`**。
- 当你想以指数退避重试同一个 runnable 时（例如瞬态网络错误），使用 **`RunnableRetry`**。

你可以将两者组合——将带重试的 runnable 作为主 runnable，用不同的提供商作为回退：

```rust
use synaptic_runnables::{RunnableRetry, RetryPolicy, RunnableWithFallbacks};

let retrying_primary = RunnableRetry::new(primary.boxed(), RetryPolicy::default());
let resilient = RunnableWithFallbacks::new(
    retrying_primary.boxed(),
    vec![fallback.boxed()],
);
```
