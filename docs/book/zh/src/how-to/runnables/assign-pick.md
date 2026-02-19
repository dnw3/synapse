# Assign 与 Pick

本指南展示如何使用 `RunnableAssign` 将计算值合并到 JSON 对象中，以及如何使用 `RunnablePick` 从中提取特定的键。

## RunnableAssign

`RunnableAssign` 接受一个 JSON 对象作为输入，对该对象并行运行命名分支，并将分支输出合并回原始对象中。这在链中流动数据时非常有用——你可以保留原始字段并添加新的计算字段。

### 基本用法

```rust
use serde_json::{json, Value};
use synaptic_runnables::{Runnable, RunnableAssign, RunnableLambda};
use synaptic_core::RunnableConfig;

let assign = RunnableAssign::new(vec![
    (
        "name_upper".to_string(),
        RunnableLambda::new(|input: Value| async move {
            let name = input["name"].as_str().unwrap_or_default();
            Ok(Value::String(name.to_uppercase()))
        }).boxed(),
    ),
    (
        "greeting".to_string(),
        RunnableLambda::new(|input: Value| async move {
            let name = input["name"].as_str().unwrap_or_default();
            Ok(Value::String(format!("Hello, {name}!")))
        }).boxed(),
    ),
]);

let config = RunnableConfig::default();
let input = json!({"name": "Alice", "age": 30});
let result = assign.invoke(input, &config).await?;

// Original fields are preserved, new fields are merged in
assert_eq!(result["name"], "Alice");
assert_eq!(result["age"], 30);
assert_eq!(result["name_upper"], "ALICE");
assert_eq!(result["greeting"], "Hello, Alice!");
```

### 工作原理

1. 输入必须是 JSON 对象（`Value::Object`）。如果不是，`RunnableAssign` 返回 `SynapticError::Validation` 错误。
2. 每个分支接收完整输入对象的克隆。
3. 所有分支通过 `futures::future::join_all` 并发运行。
4. 分支输出以分支名称作为键插入到原始对象中。如果分支名称与已有键冲突，分支输出将覆盖原始值。

### 构造函数

`RunnableAssign::new()` 接受一个 `Vec<(String, BoxRunnable<Value, Value>)>`——命名分支，每个分支将输入转换为一个待合并的值。

### 通过 `RunnablePassthrough` 的简写方式

`RunnablePassthrough` 提供了一个便捷方法，可以直接创建 `RunnableAssign`：

```rust
use synaptic_runnables::{RunnablePassthrough, RunnableLambda};
use serde_json::Value;

let assign = RunnablePassthrough::assign(vec![
    (
        "processed".to_string(),
        RunnableLambda::new(|input: Value| async move {
            // compute something from the input
            Ok(Value::String("result".to_string()))
        }).boxed(),
    ),
]);
```

---

## RunnablePick

`RunnablePick` 从 JSON 对象中提取指定的键，生成一个仅包含这些键的新对象。输入中不存在的键会从输出中静默忽略。

### 基本用法

```rust
use serde_json::{json, Value};
use synaptic_runnables::{Runnable, RunnablePick};
use synaptic_core::RunnableConfig;

let pick = RunnablePick::new(vec![
    "name".to_string(),
    "age".to_string(),
]);

let config = RunnableConfig::default();
let input = json!({
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
    "internal_id": 42
});

let result = pick.invoke(input, &config).await?;

// Only the picked keys are present
assert_eq!(result, json!({"name": "Alice", "age": 30}));
```

### 错误处理

`RunnablePick` 期望 JSON 对象作为输入。如果输入不是对象（例如字符串或数组），它会返回 `SynapticError::Validation` 错误。

缺失的键不算错误——它们只是不出现在输出中：

```rust
let pick = RunnablePick::new(vec!["name".to_string(), "missing_key".to_string()]);
let result = pick.invoke(json!({"name": "Bob"}), &config).await?;
assert_eq!(result, json!({"name": "Bob"}));
```

---

## 在链中组合 Assign 和 Pick

一种常见模式是使用 `RunnableAssign` 丰富数据，然后使用 `RunnablePick` 仅选择下游需要的字段：

```rust
use serde_json::{json, Value};
use synaptic_runnables::{Runnable, RunnableAssign, RunnablePick, RunnableLambda};
use synaptic_core::RunnableConfig;

// Step 1: Enrich input with a computed field
let assign = RunnableAssign::new(vec![
    (
        "full_name".to_string(),
        RunnableLambda::new(|input: Value| async move {
            let first = input["first"].as_str().unwrap_or_default();
            let last = input["last"].as_str().unwrap_or_default();
            Ok(Value::String(format!("{first} {last}")))
        }).boxed(),
    ),
]);

// Step 2: Pick only what the next step needs
let pick = RunnablePick::new(vec!["full_name".to_string()]);

let chain = assign.boxed() | pick.boxed();

let config = RunnableConfig::default();
let input = json!({"first": "Jane", "last": "Doe", "internal_id": 99});
let result = chain.invoke(input, &config).await?;

assert_eq!(result, json!({"full_name": "Jane Doe"}));
```
