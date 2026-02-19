# Pipe 运算符

本指南展示如何使用 `|` pipe 运算符将 runnable 串联在一起，以构建顺序处理管道。

## 概述

`BoxRunnable` 上的 `|` 运算符会创建一个 `RunnableSequence`，将第一个 runnable 的输出传递给第二个 runnable 的输入。这是在 Synaptic 中构建 LCEL 链的主要方式。

pipe 运算符通过 Rust 的 `BitOr` trait 在 `BoxRunnable` 上实现。两侧必须先使用 `.boxed()` 进行包装，因为该运算符需要类型擦除的包装器来连接具有不同具体类型的 runnable。

## 基本串联

```rust
use synaptic_runnables::{Runnable, RunnableLambda, BoxRunnable};
use synaptic_core::RunnableConfig;

let step1 = RunnableLambda::new(|x: String| async move {
    Ok(format!("Step 1: {x}"))
});

let step2 = RunnableLambda::new(|x: String| async move {
    Ok(format!("{x} -> Step 2"))
});

// Pipe operator creates a RunnableSequence
let chain = step1.boxed() | step2.boxed();

let config = RunnableConfig::default();
let result = chain.invoke("input".to_string(), &config).await?;
assert_eq!(result, "Step 1: input -> Step 2");
```

类型必须兼容：`step1` 的输出类型必须与 `step2` 的输入类型匹配。在此示例中，两者都使用 `String`，因此类型对齐。如果类型不匹配，编译器将拒绝该链。

## 多步骤链

你可以串联两个以上的步骤，继续使用 pipe 即可。结果仍然是一个 `BoxRunnable`：

```rust
let step3 = RunnableLambda::new(|x: String| async move {
    Ok(format!("{x} -> Step 3"))
});

let chain = step1.boxed() | step2.boxed() | step3.boxed();

let result = chain.invoke("start".to_string(), &config).await?;
assert_eq!(result, "Step 1: start -> Step 2 -> Step 3");
```

每个 `|` 都会将左侧包装为一个新的 `RunnableSequence`，因此 `a | b | c` 会产生 `RunnableSequence(RunnableSequence(a, b), c)`。这种嵌套是透明的——你可以将结果作为单个 `BoxRunnable<I, O>` 来使用。

## 跨步骤的类型转换

各步骤可以改变流经链的类型，只要每个步骤的输出与下一个步骤的输入匹配即可：

```rust
use synaptic_runnables::{Runnable, RunnableLambda};
use synaptic_core::RunnableConfig;

// String -> usize -> String
let count_chars = RunnableLambda::new(|s: String| async move {
    Ok(s.len())
});

let format_count = RunnableLambda::new(|n: usize| async move {
    Ok(format!("Length: {n}"))
});

let chain = count_chars.boxed() | format_count.boxed();

let config = RunnableConfig::default();
let result = chain.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "Length: 5");
```

## 为什么需要 `boxed()`

Rust 的类型系统需要在编译时知道确切的类型。如果不使用 `boxed()`，每个 `RunnableLambda` 都有一个唯一的闭包类型，无法出现在 `|` 的两侧。调用 `.boxed()` 会将具体类型擦除为 `BoxRunnable<I, O>`，这是一个 trait 对象，只要输入/输出类型对齐，就可以与任何其他 `BoxRunnable` 组合。

`BoxRunnable::new(runnable)` 等同于 `runnable.boxed()`——根据上下文选择可读性更好的方式即可。

## 使用 `RunnablePassthrough`

`RunnablePassthrough` 是一个无操作的 runnable，原样传递其输入。当你需要在链中使用一个恒等步骤时它很有用——例如，作为 `RunnableParallel` 中的一个分支：

```rust
use synaptic_runnables::{Runnable, RunnablePassthrough};

let passthrough = RunnablePassthrough;
let result = passthrough.invoke("unchanged".to_string(), &config).await?;
assert_eq!(result, "unchanged");
```

## 错误传播

如果链中的任何步骤返回 `Err`，链会立即短路并返回该错误。后续步骤不会被执行：

```rust
use synaptic_core::SynapticError;

let failing = RunnableLambda::new(|_x: String| async move {
    Err::<String, _>(SynapticError::Validation("something went wrong".into()))
});

let after = RunnableLambda::new(|x: String| async move {
    Ok(format!("This won't run: {x}"))
});

let chain = failing.boxed() | after.boxed();
let result = chain.invoke("test".to_string(), &config).await;
assert!(result.is_err());
```
