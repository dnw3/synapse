# Bind

本指南展示如何使用 `BoxRunnable::bind()` 为 runnable 附加配置变换和监听器。

## 概述

`bind()` 创建一个新的 `BoxRunnable`，在每次调用之前对 `RunnableConfig` 应用变换。这在需要向 runnable 注入标签、元数据或其他配置字段而不修改调用位置时非常有用。

在内部，`bind()` 将 runnable 包装在一个 `RunnableBind` 中，该包装器在配置上调用变换函数，然后使用修改后的配置委托给内部 runnable。

## 基本用法

```rust
use synaptic_runnables::{Runnable, RunnableLambda};
use synaptic_core::RunnableConfig;

let step = RunnableLambda::new(|x: String| async move {
    Ok(x.to_uppercase())
});

// Bind a config transform that adds a tag
let bound = step.boxed().bind(|mut config| {
    config.tags.push("my-tag".to_string());
    config
});

let config = RunnableConfig::default();
let result = bound.invoke("hello".to_string(), &config).await?;
assert_eq!(result, "HELLO");
// The inner runnable received a config with tags: ["my-tag"]
```

变换函数按值接收 `RunnableConfig`（从原始配置克隆），并返回修改后的配置。

## 添加元数据

你可以使用 `bind()` 附加下游 runnable 或回调可以检查的元数据：

```rust
use serde_json::json;

let bound = step.boxed().bind(|mut config| {
    config.metadata.insert("source".to_string(), json!("user-query"));
    config.metadata.insert("priority".to_string(), json!("high"));
    config
});
```

## 使用 `with_config()` 设置固定配置

如果你想完全替换配置而非修改它，请使用 `with_config()`。这会忽略调用时传入的任何配置，使用提供的配置代替：

```rust
let fixed_config = RunnableConfig {
    tags: vec!["production".to_string()],
    run_name: Some("fixed-pipeline".to_string()),
    ..RunnableConfig::default()
};

let bound = step.boxed().with_config(fixed_config);

// Even if a different config is passed to invoke(), the fixed config is used
let any_config = RunnableConfig::default();
let result = bound.invoke("hello".to_string(), &any_config).await?;
```

## 使用 bind 进行流式处理

`bind()` 在 `stream()` 调用期间也会应用配置变换，不仅限于 `invoke()`：

```rust
use futures::StreamExt;

let bound = step.boxed().bind(|mut config| {
    config.tags.push("streaming".to_string());
    config
});

let mut stream = bound.stream("hello".to_string(), &config);
while let Some(result) = stream.next().await {
    let value = result?;
    println!("{value}");
}
```

## 使用 `with_listeners()` 附加监听器

`with_listeners()` 为 runnable 包装前置/后置回调，在每次调用时触发。回调接收 `RunnableConfig` 的引用：

```rust
let with_logging = step.boxed().with_listeners(
    |config| {
        println!("Starting run: {:?}", config.run_name);
    },
    |config| {
        println!("Finished run: {:?}", config.run_name);
    },
);

let result = with_logging.invoke("hello".to_string(), &config).await?;
// Prints: Starting run: None
// Prints: Finished run: None
```

监听器也会在 `stream()` 调用前后触发——`on_start` 在第一项产出之前触发，`on_end` 在流完成后触发。

## 在链中与 bind 组合

`bind()` 返回一个 `BoxRunnable`，因此你可以使用 pipe 运算符将其串联：

```rust
let tagged_step = step.boxed().bind(|mut config| {
    config.tags.push("step-1".to_string());
    config
});

let chain = tagged_step | next_step.boxed();
let result = chain.invoke("input".to_string(), &config).await?;
```

## RunnableConfig 字段参考

`RunnableConfig` 结构体包含以下可通过 `bind()` 修改的字段：

| 字段 | 类型 | 描述 |
|-------|------|-------------|
| `tags` | `Vec<String>` | 用于过滤和分类的标签 |
| `metadata` | `HashMap<String, Value>` | 任意键值对元数据 |
| `max_concurrency` | `Option<usize>` | 批处理操作的并发限制 |
| `recursion_limit` | `Option<usize>` | 链的最大递归深度 |
| `run_id` | `Option<String>` | 当前运行的唯一标识符 |
| `run_name` | `Option<String>` | 当前运行的人类可读名称 |
