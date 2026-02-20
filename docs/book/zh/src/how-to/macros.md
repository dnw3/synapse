# 过程宏

`synaptic-macros` crate 提供了 **13 个属性宏 (attribute macros)**，用于消除构建 AI Agent 时常见的模板代码。它们涵盖工具定义、可运行链、工作流入口、任务追踪、中间件钩子和链路追踪六大场景。

所有宏均来自 `synaptic_macros` crate，通过 `synaptic` facade 重导出，因此可以直接使用：

```rust,ignore
use synaptic::macros::*;       // 一次性导入所有宏
use synaptic::macros::tool;    // 或按需导入
```

| 宏 | 用途 | 页面 |
|---|---|---|
| `#[tool]` | 从函数定义工具 | 本页 |
| `#[chain]` | 创建可运行链 | 本页 |
| `#[entrypoint]` | 工作流入口点 | 本页 |
| `#[task]` | 可追踪任务 | 本页 |
| `#[traceable]` | 链路追踪 | 本页 |
| `#[before_agent]` | 中间件：Agent 循环开始前 | [中间件宏](macros-middleware.md) |
| `#[before_model]` | 中间件：模型调用前 | [中间件宏](macros-middleware.md) |
| `#[after_model]` | 中间件：模型调用后 | [中间件宏](macros-middleware.md) |
| `#[after_agent]` | 中间件：Agent 循环结束后 | [中间件宏](macros-middleware.md) |
| `#[wrap_model_call]` | 中间件：包装模型调用 | [中间件宏](macros-middleware.md) |
| `#[wrap_tool_call]` | 中间件：包装工具调用 | [中间件宏](macros-middleware.md) |
| `#[dynamic_prompt]` | 中间件：动态生成系统提示词 | [中间件宏](macros-middleware.md) |

完整的端到端场景请参见 [宏使用示例](macros-examples.md)。

---

## `#[tool]` -- 从函数定义工具

将一个异步函数转换为实现 `synaptic::core::Tool` trait 的结构体。宏会自动完成以下工作：

1. 生成一个名为 `{PascalCase}Tool` 的结构体（例如 `search` -> `SearchTool`）。
2. 根据函数签名自动构建 JSON Schema，供 LLM 调用时使用。
3. 生成与函数同名的工厂函数，返回 `Arc<dyn Tool>`。

### 基本用法

```rust
use synaptic::tool;
use synaptic::core::SynapticError;

/// 在网络上搜索信息。
#[tool]
async fn search(
    /// 搜索查询词
    query: String,
    /// 返回结果的最大数量
    max_results: i64,
) -> Result<String, SynapticError> {
    Ok(format!("搜索 '{}', 最多 {} 条结果", query, max_results))
}

// 使用工厂函数获取 Tool 实例
let tool = search(); // Arc<dyn Tool>
```

宏展开后大致等价于：

```rust
struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str { "search" }
    fn description(&self) -> &str { "在网络上搜索信息。" }
    fn parameters(&self) -> Option<Value> {
        // 自动生成的 JSON Schema
        Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索查询词" },
                "max_results": { "type": "integer", "description": "返回结果的最大数量" }
            },
            "required": ["query", "max_results"]
        }))
    }
    async fn call(&self, args: Value) -> Result<Value, SynapticError> { /* ... */ }
}

fn search() -> Arc<dyn Tool> {
    Arc::new(SearchTool)
}
```

### 文档注释作为描述

- **函数级**文档注释 (`///`) 成为工具的 `description`。
- **参数级**文档注释成为 JSON Schema 中对应属性的 `"description"` 字段。

```rust
/// 获取指定城市的天气预报。
#[tool]
async fn get_weather(
    /// 城市名称，如 "北京"
    city: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}：晴", city))
}
```

### 参数类型与 JSON Schema

宏在编译期将 Rust 类型映射为 JSON Schema 类型：

| Rust 类型 | JSON Schema `type` |
|---|---|
| `String` | `"string"` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, ... | `"integer"` |
| `f32`, `f64` | `"number"` |
| `bool` | `"boolean"` |
| `Vec<T>` | `"array"`（含 `"items"` 子 schema） |
| `Option<T>` | 内部类型的 schema，但不加入 `required` |
| `serde_json::Value` | `{}`（任意类型） |
| 其他 | `"object"` |

示例：

```rust
#[tool]
async fn analyze(
    text: String,           // "string", required
    threshold: f64,         // "number", required
    tags: Vec<String>,      // "array" of "string", required
    verbose: Option<bool>,  // "boolean", 不在 required 中
) -> Result<String, SynapticError> {
    Ok("done".into())
}
```

### 自定义类型与 `schemars`

默认情况下，自定义结构体参数只会生成一个最小的 `{"type": "object"}` schema，不包含任何字段细节——LLM 无法得知该结构体的具体形状。要为自定义类型生成完整的 schema，请启用 `schemars` feature 并为参数类型派生 `JsonSchema`。

**在 `Cargo.toml` 中启用 feature**：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["macros", "schemars"] }
schemars = { version = "0.8", features = ["derive"] }
```

**为参数类型派生 `JsonSchema`**：

```rust,ignore
use schemars::JsonSchema;
use serde::Deserialize;
use synaptic::macros::tool;
use synaptic::core::SynapticError;

#[derive(Deserialize, JsonSchema)]
struct UserInfo {
    /// 用户显示名称
    name: String,
    /// 年龄（岁）
    age: i32,
    email: Option<String>,
}

/// 处理用户信息。
#[tool]
async fn process_user(
    /// 要处理的用户
    user: UserInfo,
    /// 要执行的操作
    action: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}: {}", user.name, action))
}
```

**不启用 schemars** 时，`user` 生成的 schema 为：

```json
{ "type": "object", "description": "要处理的用户" }
```

**启用 schemars** 后，`user` 生成完整的 schema：

```json
{
  "type": "object",
  "description": "要处理的用户",
  "properties": {
    "name": { "type": "string" },
    "age": { "type": "integer", "format": "int32" },
    "email": { "type": "string" }
  },
  "required": ["name", "age"]
}
```

嵌套类型同样自动生效——如果 `UserInfo` 中包含一个也派生了 `JsonSchema` 的 `Address` 结构体，地址的 schema 会通过 `$defs` 引用自动包含在内。

> **注意：** 已知的基本类型（`String`、`i32`、`Vec<T>`、`bool` 等）始终使用内置的硬编码 schema，无论是否启用 `schemars`。只有未知/自定义类型才会受益于 `schemars` 集成。

### 可选参数 (`Option<T>`)

使用 `Option<T>` 声明的参数不会出现在 JSON Schema 的 `required` 数组中。当 LLM 未提供该参数或传入 `null` 时，值为 `None`。

```rust
#[tool]
async fn greet(
    name: String,
    /// 问候语前缀
    prefix: Option<String>,
) -> Result<String, SynapticError> {
    let prefix = prefix.unwrap_or_else(|| "你好".into());
    Ok(format!("{}, {}!", prefix, name))
}
```

### 默认值 (`#[default = ...]`)

使用 `#[default = <expr>]` 为参数指定默认值。带有默认值的参数同样不会出现在 `required` 中。当 LLM 未提供该参数时，将使用默认值；若 LLM 提供了值，则使用 LLM 提供的值。

```rust
#[tool]
async fn search(
    query: String,
    #[default = 10]
    max_results: i64,
    #[default = "en"]
    language: String,
) -> Result<String, SynapticError> {
    Ok(format!("搜索 '{}' (最多 {}, 语言 {})", query, max_results, language))
}
```

生成的 JSON Schema 中，带有默认值的属性会包含 `"default"` 字段。

### 自定义工具名 (`#[tool(name = "...")]`)

默认情况下，工具名等于函数名。可以通过 `name` 属性覆盖：

```rust
#[tool(name = "web_search")]
async fn search(query: String) -> Result<String, SynapticError> {
    Ok(format!("搜索: {}", query))
}

// tool.name() == "web_search"
// 工厂函数仍为 search()
let tool = search();
```

也可以同时指定 `description` 属性来覆盖文档注释：

```rust
#[tool(name = "web_search", description = "Search the web")]
async fn search(query: String) -> Result<String, SynapticError> {
    Ok(format!("搜索: {}", query))
}
```

### 结构体字段 (`#[field]`)

有些工具需要持有状态——数据库连接、API 客户端、后端引用等。用 `#[field]` 标记的参数会成为结构体的字段而非 JSON Schema 的参数。工厂函数在构造时需要这些值，它们对 LLM 完全不可见。

```rust,ignore
use std::sync::Arc;
use synaptic::core::SynapticError;
use serde_json::Value;

#[tool]
async fn db_lookup(
    #[field] connection: Arc<String>,
    /// 要查询的表名
    table: String,
) -> Result<String, SynapticError> {
    Ok(format!("在 {} 上查询 {}", connection, table))
}

// 工厂函数现在需要传入 field 参数：
let tool = db_lookup(Arc::new("postgres://localhost".into()));
assert_eq!(tool.name(), "db_lookup");
// JSON Schema 中只包含 "table"；"connection" 对 LLM 不可见
```

宏生成的结构体包含该字段：

```rust,ignore
struct DbLookupTool {
    connection: Arc<String>,
}
```

`#[field]` 可以与普通参数、`Option<T>` 和 `#[default = ...]` 组合使用。支持多个 `#[field]` 参数：

```rust,ignore
#[tool]
async fn annotate(
    #[field] prefix: String,
    #[field] suffix: String,
    /// 输入文本
    text: String,
    #[default = 1]
    repeat: i64,
) -> Result<String, SynapticError> {
    let inner = text.repeat(repeat as usize);
    Ok(format!("{}{}{}", prefix, inner, suffix))
}

let tool = annotate("<<".into(), ">>".into());
```

> **注意：** `#[field]` 和 `#[inject]` 不能用在同一个参数上。
> `#[field]` 用于构造时提供的值；`#[inject]` 用于来自 Agent 运行时的值。

### 原始参数 (`#[args]`)

有些工具需要接收原始的 JSON 参数而不进行任何反序列化——例如 echo 工具（原样转发全部输入）或处理任意 JSON 负载的工具。用 `#[args]` 标记参数后，该参数会直接接收传给 `call()` 的原始 `serde_json::Value`。

```rust,ignore
use synaptic::macros::tool;
use synaptic::core::SynapticError;
use serde_json::{json, Value};

/// 原样回显输入。
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(json!({"echo": args}))
}

let tool = echo();
assert_eq!(tool.name(), "echo");

// parameters() 返回 None —— 不会生成 JSON Schema
assert!(tool.parameters().is_none());
```

`#[args]` 参数的特性：

- 接收原始 `Value`，不生成 JSON Schema 也不进行反序列化
- 导致 `parameters()` 返回 `None`（除非还有其他普通参数）
- 可以与 `#[field]` 参数组合使用（结构体字段仍然有效）
- 不能与 `#[inject]` 用在同一个参数上
- 最多只能有一个参数标记为 `#[args]`

```rust,ignore
/// 带可配置前缀的 echo。
#[tool]
async fn echo_with_prefix(
    #[field] prefix: String,
    #[args] args: Value,
) -> Result<Value, SynapticError> {
    Ok(json!({"prefix": prefix, "data": args}))
}

let tool = echo_with_prefix(">>".into());
```

### 运行时注入 (`#[inject(state)]`, `#[inject(store)]`, `#[inject(tool_call_id)]`)

使用 `#[inject(...)]` 标记的参数不会出现在 LLM 可见的 JSON Schema 中，而是在工具执行时由运行时自动注入。使用任意一个 `#[inject]` 属性后，宏生成的实现从 `Tool` trait 切换为 `RuntimeAwareTool` trait，工厂函数返回 `Arc<dyn RuntimeAwareTool>`。

支持三种注入源：

| 注入标记 | 注入内容 |
|---|---|
| `#[inject(state)]` | Agent 运行时的状态 (`Value`)，从 `ToolRuntime.state` 反序列化 |
| `#[inject(store)]` | 共享存储引用，从 `ToolRuntime.store` 获取 |
| `#[inject(tool_call_id)]` | 当前工具调用的 ID (`String`)，从 `ToolRuntime.tool_call_id` 获取 |

```rust
use synaptic::core::{SynapticError, ToolRuntime};
use serde_json::Value;

/// 保存数据到存储。
#[tool]
async fn save_note(
    /// 笔记内容
    content: String,
    #[inject(tool_call_id)]
    call_id: String,
    #[inject(state)]
    state: Value,
) -> Result<String, SynapticError> {
    Ok(format!("已保存 (call_id={}, state={:?})", call_id, state))
}

// 返回 Arc<dyn RuntimeAwareTool>（而非 Arc<dyn Tool>）
let tool = save_note();
```

生成的 JSON Schema 中只会包含 `content` 参数，`call_id` 和 `state` 对 LLM 不可见。

---

## `#[chain]` -- 创建可运行链

将异步函数转换为返回 `BoxRunnable<Value, Value>` 的工厂函数。生成的 Runnable 内部使用 `RunnableLambda` 包装。

函数必须接受 `serde_json::Value` 作为输入，返回 `Result<serde_json::Value, SynapticError>`。

### 基本用法

```rust
use synaptic::chain;
use synaptic::core::SynapticError;
use serde_json::Value;

#[chain]
async fn uppercase(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default().to_uppercase();
    Ok(Value::String(s))
}

// uppercase() 返回 BoxRunnable<Value, Value>
let runnable = uppercase();
```

宏展开后大致等价于：

```rust
async fn uppercase_impl(input: Value) -> Result<Value, SynapticError> {
    let s = input.as_str().unwrap_or_default().to_uppercase();
    Ok(Value::String(s))
}

pub fn uppercase() -> BoxRunnable<Value, Value> {
    RunnableLambda::new(|input: Value| async move {
        uppercase_impl(input).await
    }).boxed()
}
```

> `#[chain]` 不接受任何属性参数。

### 输出类型推断

宏会自动检测返回类型：

| 返回类型 | 生成的类型 | 行为 |
|---|---|---|
| `Result<Value, _>` | `BoxRunnable<I, Value>` | 将结果序列化为 `Value` |
| `Result<String, _>` | `BoxRunnable<I, String>` | 直接返回，不进行序列化 |
| `Result<T, _>`（其他类型） | `BoxRunnable<I, T>` | 直接返回，不进行序列化 |

### 类型化输出

当返回类型不是 `Value` 时，宏生成的 Runnable 是类型化的，没有序列化开销：

```rust,ignore
// 返回 String —— 生成 BoxRunnable<String, String>
#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[chain]
async fn exclaim(s: String) -> Result<String, SynapticError> {
    Ok(format!("{}!", s))
}

// 类型化的 chain 可以用 | 自然组合
let pipeline = to_upper() | exclaim();
let result = pipeline.invoke("hello".into(), &config).await?;
assert_eq!(result, "HELLO!");
```

### 使用 `|` 组合

`BoxRunnable` 实现了 `|` 管道运算符，可以将多个 chain 组合成流水线：

```rust
#[chain]
async fn step_a(input: Value) -> Result<Value, SynapticError> {
    // 第一步处理
    Ok(input)
}

#[chain]
async fn step_b(input: Value) -> Result<Value, SynapticError> {
    // 第二步处理
    Ok(input)
}

// 组合为 A -> B 的流水线
let pipeline = step_a() | step_b();
```

---

## `#[entrypoint]` -- 工作流入口点

定义 LangGraph 风格的工作流入口。宏将异步函数转换为返回 `synaptic::core::Entrypoint` 的工厂函数。

函数必须：
- 是 `async` 的
- 接受恰好一个参数（`serde_json::Value`）
- 返回 `Result<Value, SynapticError>`

### 基本用法

```rust
use synaptic::entrypoint;
use synaptic::core::SynapticError;
use serde_json::Value;

#[entrypoint]
async fn my_workflow(input: Value) -> Result<Value, SynapticError> {
    // 工作流逻辑
    Ok(input)
}

// my_workflow() 返回 Entrypoint
let ep = my_workflow();
```

宏展开后大致等价于：

```rust
fn my_workflow() -> Entrypoint {
    Entrypoint {
        config: EntrypointConfig {
            name: "my_workflow",
            checkpointer: None,
        },
        invoke_fn: Box::new(|input: Value| {
            Box::pin(async move {
                // 工作流逻辑
                Ok(input)
            })
        }),
    }
}
```

### 属性 (`name`, `checkpointer`)

| 属性 | 说明 | 默认值 |
|---|---|---|
| `name` | 入口点名称 | 函数名 |
| `checkpointer` | 检查点后端提示（如 `"memory"`） | `None` |

```rust
#[entrypoint(name = "chat_agent", checkpointer = "memory")]
async fn my_agent(input: Value) -> Result<Value, SynapticError> {
    Ok(input)
}

let ep = my_agent();
// ep.config.name == "chat_agent"
// ep.config.checkpointer == Some("memory")
```

---

## `#[task]` -- 可追踪任务

将异步函数标记为可追踪任务，用于工作流内部的步骤标识。宏会：

1. 将原始函数体移动到 `{name}_impl` 私有函数中。
2. 生成一个同名的公开包装函数，其中包含 `__TASK_NAME` 常量。
3. 包装函数将调用委托给 `_impl` 版本。

### 基本用法

```rust
use synaptic::task;
use synaptic::core::SynapticError;

#[task]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("{}：晴", city))
}

// 可以直接调用，内部委托给 fetch_weather_impl
let result = fetch_weather("北京".into()).await?;
```

宏展开后大致等价于：

```rust
async fn fetch_weather_impl(city: String) -> Result<String, SynapticError> {
    Ok(format!("{}：晴", city))
}

pub async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    #[allow(dead_code)]
    const __TASK_NAME: &str = "fetch_weather";
    fetch_weather_impl(city).await
}
```

### 自定义任务名

```rust
#[task(name = "weather_lookup")]
async fn fetch_weather(city: String) -> Result<String, SynapticError> {
    Ok(format!("{}：晴", city))
}

// __TASK_NAME == "weather_lookup"
```

---

## `#[traceable]` -- 链路追踪

为异步或同步函数添加 `tracing` 链路追踪 instrumentation。宏会使用 `tracing::info_span!` 创建 span，并将函数参数值记录为 span 字段。

- 对于 **async** 函数，使用 `tracing::Instrument` trait 确保 span 在异步上下文中正确传播。
- 对于 **sync** 函数，使用 span guard (`span.enter()`) 管理生命周期。

### 基本用法

```rust
use synaptic::traceable;

#[traceable]
async fn process_data(input: String, count: usize) -> String {
    format!("{}: {}", input, count)
}
```

展开后大致等价于：

```rust
async fn process_data(input: String, count: usize) -> String {
    use tracing::Instrument;
    let __span = tracing::info_span!(
        "process_data",
        input = tracing::field::debug(&input),
        count = tracing::field::debug(&count),
    );
    async move {
        format!("{}: {}", input, count)
    }.instrument(__span).await
}
```

### 自定义 Span 名称

使用 `name` 属性覆盖默认的 span 名称（默认为函数名）：

```rust
#[traceable(name = "data_processing")]
async fn process_data(input: String) -> String {
    input.to_uppercase()
}
```

### 跳过参数

使用 `skip` 属性排除敏感参数，使其不被记录到 span 中。多个参数名用逗号分隔：

```rust
#[traceable(skip = "api_key")]
async fn call_api(url: String, api_key: String) -> Result<String, SynapticError> {
    // api_key 不会出现在 tracing span 中
    Ok("response".into())
}

#[traceable(name = "secure_call", skip = "token,secret")]
async fn secure_request(
    endpoint: String,
    token: String,
    secret: String,
) -> Result<String, SynapticError> {
    // 只有 endpoint 会被记录
    Ok("ok".into())
}
```
