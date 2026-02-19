# Generator

本指南展示如何使用 `RunnableGenerator` 从流式生成器函数创建 runnable。

## 概述

`RunnableGenerator` 包装一个产出 `Stream` 结果的函数。它在流式生成器和 `Runnable` trait 之间架起桥梁：

- **`invoke()`** 将整个流收集为 `Vec<O>`
- **`stream()`** 在每个项产出时逐个返回

当你需要一个自然产出增量输出的 runnable 时非常有用——例如分词器、分块器，或任何产出部分结果的计算。

## 基本用法

```rust
use synaptic_runnables::{Runnable, RunnableGenerator};
use synaptic_core::{RunnableConfig, SynapticError};

let gen = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_uppercase());
        }
    }
});

let config = RunnableConfig::default();
let result = gen.invoke("hello world".to_string(), &config).await?;
assert_eq!(result, vec!["HELLO", "WORLD"]);
```

## 流式处理

`RunnableGenerator` 的真正强大之处在于流式处理。`stream()` 在每个项产出时立即返回，无需等待生成器完成：

```rust
use futures::StreamExt;
use synaptic_runnables::{Runnable, RunnableGenerator};
use synaptic_core::RunnableConfig;

let gen = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for ch in input.chars() {
            yield Ok(ch.to_string());
        }
    }
});

let config = RunnableConfig::default();
let mut stream = gen.stream("abc".to_string(), &config);

// Each item arrives individually wrapped in a Vec
while let Some(item) = stream.next().await {
    let chunk = item?;
    println!("{:?}", chunk); // ["a"], ["b"], ["c"]
}
```

每个流式项被包装在 `Vec<O>` 中以匹配 `invoke()` 的输出类型。这意味着 `stream()` 产出 `Result<Vec<O>, SynapticError>`，其中每个 `Vec` 包含单个元素。

## 错误处理

如果生成器产出一个 `Err`，`invoke()` 停止收集并返回该错误。`stream()` 则产出该错误并继续到下一项：

```rust
use synaptic_runnables::RunnableGenerator;
use synaptic_core::SynapticError;

let gen = RunnableGenerator::new(|_input: String| {
    async_stream::stream! {
        yield Ok("first".to_string());
        yield Err(SynapticError::Other("oops".into()));
        yield Ok("third".to_string());
    }
});

// invoke() fails on the error:
// gen.invoke("x".to_string(), &config).await => Err(...)

// stream() yields all three items:
// Ok(["first"]), Err(...), Ok(["third"])
```

## 在管道中使用

`RunnableGenerator` 实现了 `Runnable<I, Vec<O>>`，因此它可以与 pipe 运算符配合使用。在链中需要流式生成的地方放置它即可：

```rust
use synaptic_runnables::{Runnable, RunnableGenerator, RunnableLambda};

let tokenize = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for token in input.split_whitespace() {
            yield Ok(token.to_string());
        }
    }
});

let count = RunnableLambda::new(|tokens: Vec<String>| async move {
    Ok(tokens.len())
});

let chain = tokenize.boxed() | count.boxed();

// chain.invoke("one two three".to_string(), &config).await => Ok(3)
```

## 类型签名

```rust,ignore
pub struct RunnableGenerator<I: Send + 'static, O: Send + 'static> { ... }

impl<I, O> Runnable<I, Vec<O>> for RunnableGenerator<I, O> { ... }
```

构造函数接受任意函数 `Fn(I) -> S`，其中 `S: Stream<Item = Result<O, SynapticError>> + Send + 'static`。`async_stream::stream!` 宏是产出此类流的最便捷方式。
