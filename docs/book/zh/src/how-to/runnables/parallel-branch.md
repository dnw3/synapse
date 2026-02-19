# Parallel 与 Branch

本指南展示如何使用 `RunnableParallel` 并发运行多个 runnable，以及如何使用 `RunnableBranch` 将输入路由到不同的 runnable。

## RunnableParallel

`RunnableParallel` 对相同的输入并发运行命名分支，然后将所有输出合并为一个以分支名称为键的 `serde_json::Value` 对象。

输入类型必须实现 `Clone`，因为每个分支会接收其自己的副本。每个分支必须产出 `serde_json::Value` 输出。

### 基本用法

```rust
use serde_json::Value;
use synaptic_runnables::{Runnable, RunnableParallel, RunnableLambda};
use synaptic_core::RunnableConfig;

let parallel = RunnableParallel::new(vec![
    (
        "upper".to_string(),
        RunnableLambda::new(|x: String| async move {
            Ok(Value::String(x.to_uppercase()))
        }).boxed(),
    ),
    (
        "lower".to_string(),
        RunnableLambda::new(|x: String| async move {
            Ok(Value::String(x.to_lowercase()))
        }).boxed(),
    ),
    (
        "length".to_string(),
        RunnableLambda::new(|x: String| async move {
            Ok(Value::Number(x.len().into()))
        }).boxed(),
    ),
]);

let config = RunnableConfig::default();
let result = parallel.invoke("Hello".to_string(), &config).await?;

// result is a JSON object:
// {"upper": "HELLO", "lower": "hello", "length": 5}
assert_eq!(result["upper"], "HELLO");
assert_eq!(result["lower"], "hello");
assert_eq!(result["length"], 5);
```

### 构造函数

`RunnableParallel::new()` 接受一个 `Vec<(String, BoxRunnable<I, Value>)>`——一个 `(名称, runnable)` 对的列表。所有分支通过 `futures::future::join_all` 并发运行。

### 在链中使用

`RunnableParallel` 实现了 `Runnable<I, Value>`，因此你可以在 pipe 链中使用它。一种常见模式是扇出处理然后合并结果：

```rust
let analyze = RunnableParallel::new(vec![
    ("summary".to_string(), summarizer.boxed()),
    ("keywords".to_string(), keyword_extractor.boxed()),
]);

let format_report = RunnableLambda::new(|data: Value| async move {
    Ok(format!(
        "Summary: {}\nKeywords: {}",
        data["summary"], data["keywords"]
    ))
});

let chain = analyze.boxed() | format_report.boxed();
```

### 错误处理

如果任何分支失败，整个 `RunnableParallel` 调用将返回遇到的第一个错误。在失败之前已完成的成功分支将被丢弃。

---

## RunnableBranch

`RunnableBranch` 根据条件函数将输入路由到多个 runnable 之一。它按顺序评估条件，调用与第一个匹配条件关联的 runnable。如果没有条件匹配，则使用默认的 runnable。

### 基本用法

```rust
use synaptic_runnables::{Runnable, RunnableBranch, RunnableLambda, BoxRunnable};
use synaptic_core::RunnableConfig;

let branch = RunnableBranch::new(
    vec![
        (
            Box::new(|x: &String| x.starts_with("hi")) as Box<dyn Fn(&String) -> bool + Send + Sync>,
            RunnableLambda::new(|x: String| async move {
                Ok(format!("Greeting: {x}"))
            }).boxed(),
        ),
        (
            Box::new(|x: &String| x.starts_with("bye")),
            RunnableLambda::new(|x: String| async move {
                Ok(format!("Farewell: {x}"))
            }).boxed(),
        ),
    ],
    // Default: used when no condition matches
    RunnableLambda::new(|x: String| async move {
        Ok(format!("Other: {x}"))
    }).boxed(),
);

let config = RunnableConfig::default();

let r1 = branch.invoke("hi there".to_string(), &config).await?;
assert_eq!(r1, "Greeting: hi there");

let r2 = branch.invoke("bye now".to_string(), &config).await?;
assert_eq!(r2, "Farewell: bye now");

let r3 = branch.invoke("something else".to_string(), &config).await?;
assert_eq!(r3, "Other: something else");
```

### 构造函数

`RunnableBranch::new()` 接受两个参数：

1. `branches: Vec<(BranchCondition<I>, BoxRunnable<I, O>)>`——按顺序评估的条件/runnable 对。条件类型为 `Box<dyn Fn(&I) -> bool + Send + Sync>`。
2. `default: BoxRunnable<I, O>`——当没有条件匹配时的回退 runnable。

### 在链中使用

`RunnableBranch` 实现了 `Runnable<I, O>`，因此它可以与 pipe 运算符配合使用：

```rust
let preprocess = RunnableLambda::new(|x: String| async move {
    Ok(x.trim().to_string())
});

let route = RunnableBranch::new(
    vec![/* conditions */],
    default_handler.boxed(),
);

let chain = preprocess.boxed() | route.boxed();
```

### 何时使用各自

- 当你需要对同一输入并发运行多个操作并组合所有结果时，使用 **`RunnableParallel`**。
- 当你需要根据输入值选择单一处理路径时，使用 **`RunnableBranch`**。
