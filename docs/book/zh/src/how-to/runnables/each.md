# Each

本指南展示如何使用 `RunnableEach` 对列表中的每个元素映射执行一个 `runnable`。

## 概述

`RunnableEach` 包裹任意 `BoxRunnable<I, O>` 并将其应用于 `Vec<I>` 中的每个元素，产生 `Vec<O>`。它是 `runnable` 版本的 `Iterator::map()` -- 通过相同的转换处理一批项。

## 基本用法

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::RunnableConfig;

let upper = RunnableLambda::new(|s: String| async move {
    Ok(s.to_uppercase())
});

let each = RunnableEach::new(upper.boxed());

let config = RunnableConfig::default();
let result = each.invoke(
    vec!["hello".into(), "world".into()],
    &config,
).await?;

assert_eq!(result, vec!["HELLO", "WORLD"]);
```

## 错误传播

如果内部 `runnable` 在任何元素上失败，`RunnableEach` 会立即停止并返回该错误。失败之前已处理的元素会被丢弃：

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::{RunnableConfig, SynapticError};

let must_be_short = RunnableLambda::new(|s: String| async move {
    if s.len() > 5 {
        Err(SynapticError::Other(format!("too long: {s}")))
    } else {
        Ok(s.to_uppercase())
    }
});

let each = RunnableEach::new(must_be_short.boxed());
let config = RunnableConfig::default();

let result = each.invoke(
    vec!["hi".into(), "toolong".into(), "ok".into()],
    &config,
).await;

assert!(result.is_err()); // 在 "toolong" 上失败
```

## 空输入

空的输入向量会产生空的输出向量：

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};
use synaptic_core::RunnableConfig;

let identity = RunnableLambda::new(|s: String| async move { Ok(s) });
let each = RunnableEach::new(identity.boxed());

let config = RunnableConfig::default();
let result = each.invoke(vec![], &config).await?;
assert!(result.is_empty());
```

## 在管道中使用

`RunnableEach` 实现了 `Runnable<Vec<I>, Vec<O>>`，因此可以与管道操作符组合使用。一个常见的模式是将输入拆分为多个部分，用 `RunnableEach` 处理每个部分，然后合并结果：

```rust
use synaptic_runnables::{Runnable, RunnableEach, RunnableLambda};

// 步骤 1：将字符串拆分为单词
let split = RunnableLambda::new(|s: String| async move {
    Ok(s.split_whitespace().map(String::from).collect::<Vec<_>>())
});

// 步骤 2：处理每个单词
let process = RunnableEach::new(
    RunnableLambda::new(|w: String| async move {
        Ok(w.to_uppercase())
    }).boxed()
);

// 步骤 3：合并结果
let join = RunnableLambda::new(|words: Vec<String>| async move {
    Ok(words.join(", "))
});

let chain = split.boxed() | process.boxed() | join.boxed();
// chain.invoke("hello world".to_string(), &config).await => Ok("HELLO, WORLD")
```

## 类型签名

```rust,ignore
pub struct RunnableEach<I: Send + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
}

impl<I, O> Runnable<Vec<I>, Vec<O>> for RunnableEach<I, O> { ... }
```

元素按顺序依次处理。如需并发处理，请使用 `RunnableParallel` 或 `BoxRunnable` 上的 `batch()` 方法。
