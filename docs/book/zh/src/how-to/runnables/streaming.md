# 通过链进行流式处理

本指南展示如何使用 `stream()` 从 LCEL 链中消费增量输出。

## 概述

每个 `Runnable` 都提供一个 `stream()` 方法，返回一个 `RunnableOutputStream`——一个固定的、装箱的 `Stream`，其中每个元素为 `Result<O, SynapticError>`。这允许下游消费者在结果可用时立即处理，而无需等待整个链完成。

默认的 `stream()` 实现将 `invoke()` 包装为单项流。支持真正增量输出的 Runnable（如 LLM 模型适配器或 `RunnableGenerator`）会重写 `stream()` 以逐项产出。

## 对单个 runnable 进行流式处理

```rust
use futures::StreamExt;
use synaptic_runnables::{Runnable, RunnableLambda};
use synaptic_core::RunnableConfig;

let upper = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
});

let config = RunnableConfig::default();
let mut stream = upper.stream("hello".to_string(), &config);

while let Some(result) = stream.next().await {
    let value = result?;
    println!("Got: {value}");
}
// Prints: Got: HELLO
```

因为 `RunnableLambda` 使用默认的 `stream()` 实现，所以它只产出一项——`invoke()` 的完整结果。

## 通过链进行流式处理

当你通过 `RunnableSequence`（由 `|` 运算符创建）进行流式处理时，行为如下：

1. 第一个步骤通过 `invoke()` 完整运行并产出其完整输出。
2. 该输出被传入第二个步骤的 `stream()`，后者增量地产出各项。

这意味着**只有链中的最后一个组件才真正进行流式处理**。中间步骤会缓冲其输出。这与 LangChain 的行为一致。

```rust
use futures::StreamExt;
use synaptic_runnables::{Runnable, RunnableLambda};
use synaptic_core::RunnableConfig;

let step1 = RunnableLambda::new(|x: String| async move {
    Ok(format!("processed: {x}"))
});

let step2 = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
});

let chain = step1.boxed() | step2.boxed();

let config = RunnableConfig::default();
let mut stream = chain.stream("input".to_string(), &config);

while let Some(result) = stream.next().await {
    let value = result?;
    println!("Got: {value}");
}
// Prints: Got: PROCESSED: INPUT
```

## 使用 `BoxRunnable` 进行流式处理

`BoxRunnable` 保留内部 runnable 的流式行为。直接在其上调用 `.stream()` 即可：

```rust
let boxed_chain = step1.boxed() | step2.boxed();
let mut stream = boxed_chain.stream("input".to_string(), &config);

while let Some(result) = stream.next().await {
    let value = result?;
    println!("{value}");
}
```

## 使用 `RunnableGenerator` 进行真正的流式处理

`RunnableGenerator` 包装一个返回 `Stream` 的生成器函数，实现真正的增量输出：

```rust
use futures::StreamExt;
use synaptic_runnables::{Runnable, RunnableGenerator};
use synaptic_core::RunnableConfig;

let gen = RunnableGenerator::new(|input: String| {
    async_stream::stream! {
        for word in input.split_whitespace() {
            yield Ok(word.to_uppercase());
        }
    }
});

let config = RunnableConfig::default();
let mut stream = gen.stream("hello world foo".to_string(), &config);

while let Some(result) = stream.next().await {
    let items = result?;
    println!("Chunk: {:?}", items);
}
// Prints each word as a separate chunk:
// Chunk: ["HELLO"]
// Chunk: ["WORLD"]
// Chunk: ["FOO"]
```

当你对 `RunnableGenerator` 调用 `invoke()` 时，它会将所有流式项收集到一个 `Vec<O>` 中。

## 将流收集为单一结果

如果你需要完整结果而非增量输出，请使用 `invoke()` 而非 `stream()`。或者，手动收集流：

```rust
use futures::StreamExt;

let mut stream = chain.stream("input".to_string(), &config);
let mut items = Vec::new();

while let Some(result) = stream.next().await {
    items.push(result?);
}

// items now contains all yielded values
```

## 流中的错误处理

如果链中的任何步骤在流式处理期间失败，流会产出一个 `Err` 项。消费者应检查每个项：

```rust
while let Some(result) = stream.next().await {
    match result {
        Ok(value) => println!("Got: {value}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

当 `RunnableSequence` 的第一个步骤失败时（在其 `invoke()` 期间），流会立即将该错误作为唯一的项产出。
