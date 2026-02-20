# 过程宏

`synaptic-macros` crate 提供了 **13 个属性宏 (attribute macros)**，用于消除构建 AI Agent 时常见的模板代码。它们涵盖工具定义、可运行链、工作流入口、任务追踪、中间件钩子和链路追踪六大场景。

| 宏 | 用途 |
|---|---|
| `#[tool]` | 将异步函数转为 `Tool` / `RuntimeAwareTool` 实现 |
| `#[chain]` | 将异步函数转为 `BoxRunnable<Value, Value>` 工厂 |
| `#[entrypoint]` | 定义 LangGraph 风格的工作流入口点 |
| `#[task]` | 定义可追踪的工作流任务 |
| `#[before_agent]` | 中间件：Agent 循环开始前 |
| `#[before_model]` | 中间件：模型调用前 |
| `#[after_model]` | 中间件：模型调用后 |
| `#[after_agent]` | 中间件：Agent 循环结束后 |
| `#[wrap_model_call]` | 中间件：包装模型调用 |
| `#[wrap_tool_call]` | 中间件：包装工具调用 |
| `#[dynamic_prompt]` | 中间件：动态生成系统提示词 |
| `#[traceable]` | 为函数添加 `tracing` 链路追踪 |

> 所有宏均来自 `synaptic_macros` crate，通过 `synaptic` facade 重导出，因此可以直接使用 `use synaptic::tool;` 等形式导入。

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

## 中间件宏

Synaptic 提供了 7 个中间件宏，分别对应 Agent 执行生命周期中的不同钩子点。每个宏的生成模式一致：

1. 生成一个名为 `{PascalCase}Middleware` 的结构体（例如 `setup` -> `SetupMiddleware`）。
2. 为该结构体实现 `synaptic::middleware::AgentMiddleware` trait，仅重写对应的钩子方法。
3. 生成与函数同名的工厂函数，返回 `Arc<dyn AgentMiddleware>`。

### `#[before_agent]`

在 Agent 循环**开始前**执行。函数签名：`async fn(messages: &mut Vec<Message>) -> Result<(), SynapticError>`

```rust
use synaptic::before_agent;
use synaptic::core::{Message, SynapticError};

#[before_agent]
async fn setup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    println!("Agent 即将启动，当前有 {} 条消息", messages.len());
    Ok(())
}

let mw = setup(); // Arc<dyn AgentMiddleware>
```

### `#[before_model]`

在每次**模型调用前**执行。函数签名：`async fn(request: &mut ModelRequest) -> Result<(), SynapticError>`

```rust
use synaptic::before_model;
use synaptic::middleware::ModelRequest;
use synaptic::core::SynapticError;

#[before_model]
async fn add_context(request: &mut ModelRequest) -> Result<(), SynapticError> {
    request.system_prompt = Some("请用中文回答".into());
    Ok(())
}

let mw = add_context(); // Arc<dyn AgentMiddleware>
```

### `#[after_model]`

在每次**模型调用后**执行。函数签名：`async fn(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError>`

注意 `request` 是不可变引用，`response` 是可变引用，允许修改模型响应。

```rust
use synaptic::after_model;
use synaptic::middleware::{ModelRequest, ModelResponse};
use synaptic::core::SynapticError;

#[after_model]
async fn log_response(
    request: &ModelRequest,
    response: &mut ModelResponse,
) -> Result<(), SynapticError> {
    println!("模型返回: {}", response.message.content());
    Ok(())
}

let mw = log_response(); // Arc<dyn AgentMiddleware>
```

### `#[after_agent]`

在 Agent 循环**结束后**执行。函数签名与 `#[before_agent]` 相同：`async fn(messages: &mut Vec<Message>) -> Result<(), SynapticError>`

```rust
use synaptic::after_agent;
use synaptic::core::{Message, SynapticError};

#[after_agent]
async fn cleanup(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    println!("Agent 执行完毕，共产生 {} 条消息", messages.len());
    Ok(())
}

let mw = cleanup(); // Arc<dyn AgentMiddleware>
```

### `#[wrap_model_call]`

**包装模型调用**，可用于实现重试、降级、缓存等模式。函数签名：`async fn(request: ModelRequest, next: &dyn ModelCaller) -> Result<ModelResponse, SynapticError>`

必须调用 `next.call(request)` 来执行真正的模型调用，也可以选择不调用（短路）。

```rust
use synaptic::wrap_model_call;
use synaptic::middleware::{ModelRequest, ModelResponse, ModelCaller};
use synaptic::core::SynapticError;

#[wrap_model_call]
async fn retry_on_failure(
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    match next.call(request.clone()).await {
        Ok(response) => Ok(response),
        Err(_) => {
            // 第一次失败，重试一次
            next.call(request).await
        }
    }
}

let mw = retry_on_failure(); // Arc<dyn AgentMiddleware>
```

### `#[wrap_tool_call]`

**包装工具调用**，在工具执行前后插入自定义逻辑。函数签名：`async fn(request: ToolCallRequest, next: &dyn ToolCaller) -> Result<Value, SynapticError>`

```rust
use synaptic::wrap_tool_call;
use synaptic::middleware::{ToolCallRequest, ToolCaller};
use synaptic::core::SynapticError;
use serde_json::Value;

#[wrap_tool_call]
async fn log_tool(
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    println!("调用工具: {}", request.call.name);
    let result = next.call(request).await?;
    println!("工具返回: {:?}", result);
    Ok(result)
}

let mw = log_tool(); // Arc<dyn AgentMiddleware>
```

### `#[dynamic_prompt]`

根据当前消息上下文**动态生成系统提示词**。与其他中间件不同，此宏要求函数是**非异步的** (`fn` 而非 `async fn`)。

函数签名：`fn(messages: &[Message]) -> String`

生成的中间件会在 `before_model` 钩子中将返回的字符串设置为 `request.system_prompt`。

```rust
use synaptic::dynamic_prompt;
use synaptic::core::Message;

#[dynamic_prompt]
fn context_aware_prompt(messages: &[Message]) -> String {
    if messages.len() > 10 {
        "请简洁回答，对话已经很长了。".into()
    } else {
        "请详细回答用户的问题。".into()
    }
}

let mw = context_aware_prompt(); // Arc<dyn AgentMiddleware>
```

> **为什么 `#[dynamic_prompt]` 是同步的？**
>
> 与其他中间件宏不同，`#[dynamic_prompt]` 要求使用普通的 `fn` 而非 `async fn`。
> 这是一个刻意的设计选择：
>
> 1. **纯计算操作** — 动态提示词生成通常只涉及检查消息列表和拼接字符串，属于
>    纯 CPU 操作（模式匹配、字符串格式化），不涉及任何 I/O。将其定义为
>    async 会引入不必要的开销（Future 状态机、poll 机制），却毫无收益。
>
> 2. **简洁性** — 同步函数更容易编写和理解，无需 `.await`、无需处理 Pin 和
>    Send/Sync 约束。
>
> 3. **内部异步包装** — 宏在生成代码时会将你的同步函数包装在一个 `before_model`
>    异步钩子中调用。钩子本身是 async 的（这是 `AgentMiddleware` trait 的要求），
>    但你的函数不需要是 async 的。
>
> 如果你需要在提示词生成过程中执行异步操作（如从数据库获取上下文或调用外部 API），
> 请直接使用 `#[before_model]` 并手动设置 `request.system_prompt`：
>
> ```rust,ignore
> #[before_model]
> async fn async_prompt(request: &mut ModelRequest) -> Result<(), SynapticError> {
>     let context = fetch_from_database().await?;  // 异步 I/O
>     request.system_prompt = Some(format!("上下文: {}", context));
>     Ok(())
> }
> ```

> 所有中间件宏均不接受属性参数。但所有中间件宏都支持 `#[field]` 参数来构建**有状态的中间件**（参见下方 [有状态中间件与 `#[field]`](#有状态中间件与-field)）。

### 有状态中间件与 `#[field]`

所有中间件宏都支持 `#[field]` 参数——函数参数变为结构体字段而非 trait 方法参数。这使得你可以构建带有配置状态的中间件，与 `#[tool]` 工具中的 `#[field]` 用法一致。

Field 参数必须放在 trait 要求的参数**之前**。工厂函数将接受 field 值，生成的结构体会存储它们。

**示例：带可配置重试次数的重试中间件**

```rust,ignore
use std::time::Duration;
use synaptic::macros::wrap_tool_call;
use synaptic::middleware::{ToolCallRequest, ToolCaller};
use synaptic::core::SynapticError;
use serde_json::Value;

#[wrap_tool_call]
async fn tool_retry(
    #[field] max_retries: usize,
    #[field] base_delay: Duration,
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    let mut last_err = None;
    for attempt in 0..=max_retries {
        match next.call(request.clone()).await {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                if attempt < max_retries {
                    let delay = base_delay * 2u32.saturating_pow(attempt as u32);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

// 工厂函数接受 field 值：
let mw = tool_retry(3, Duration::from_millis(100));
```

**示例：带备选模型的模型降级中间件**

```rust,ignore
use std::sync::Arc;
use synaptic::macros::wrap_model_call;
use synaptic::middleware::{BaseChatModelCaller, ModelRequest, ModelResponse, ModelCaller};
use synaptic::core::{ChatModel, SynapticError};

#[wrap_model_call]
async fn model_fallback(
    #[field] fallbacks: Vec<Arc<dyn ChatModel>>,
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    match next.call(request.clone()).await {
        Ok(resp) => Ok(resp),
        Err(primary_err) => {
            for fallback in &fallbacks {
                let caller = BaseChatModelCaller::new(fallback.clone());
                if let Ok(resp) = caller.call(request.clone()).await {
                    return Ok(resp);
                }
            }
            Err(primary_err)
        }
    }
}

let mw = model_fallback(vec![backup_model]);
```

**示例：带品牌标识的动态提示词**

```rust,ignore
use synaptic::macros::dynamic_prompt;
use synaptic::core::Message;

#[dynamic_prompt]
fn branded_prompt(#[field] brand: String, messages: &[Message]) -> String {
    format!("[{}] 你有 {} 条消息", brand, messages.len())
}

let mw = branded_prompt("Acme Corp".into());
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

---

## 完整示例

以下七个端到端场景展示了各种宏在实际应用中的协作方式。

### 场景 A：带自定义工具的天气 Agent

本示例演示如何使用 `#[tool]` 定义一个带 `#[field]` API 密钥的工具，注册该工具并使用 `create_react_agent` 创建 ReAct Agent，然后执行查询。

```rust,ignore
use synaptic::core::{ChatModel, Message, SynapticError};
use synaptic::graph::{create_react_agent, MessageState, GraphResult};
use synaptic::models::ScriptedChatModel;
use std::sync::Arc;

/// 获取指定城市的当前天气。
#[tool]
async fn get_weather(
    #[field] api_key: String,
    /// 要查询的城市名称
    city: String,
) -> Result<String, SynapticError> {
    // 生产环境中，使用 api_key 调用真实的天气 API
    Ok(format!("{}：22°C，晴", city))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let tool = get_weather("sk-fake-key".into());
    let tools: Vec<Arc<dyn synaptic::core::Tool>> = vec![tool];

    let model: Arc<dyn ChatModel> = Arc::new(ScriptedChatModel::new(vec![/* ... */]));
    let agent = create_react_agent(model, tools).compile()?;

    let state = MessageState::from_messages(vec![
        Message::human("东京现在天气怎么样？"),
    ]);

    let result = agent.invoke(state, None).await?;
    println!("{:?}", result.into_state().messages);
    Ok(())
}
```

### 场景 B：使用 Chain 宏构建数据处理流水线

本示例将多个 `#[chain]` 步骤组合成一个处理流水线，依次执行文本提取、规范化和词数统计。

```rust,ignore
use synaptic::core::{RunnableConfig, SynapticError};
use synaptic::runnables::Runnable;
use serde_json::{json, Value};

#[chain]
async fn extract_text(input: Value) -> Result<Value, SynapticError> {
    let text = input["content"].as_str().unwrap_or("");
    Ok(json!(text.to_string()))
}

#[chain]
async fn normalize(input: Value) -> Result<Value, SynapticError> {
    let text = input.as_str().unwrap_or("").to_lowercase().trim().to_string();
    Ok(json!(text))
}

#[chain]
async fn word_count(input: Value) -> Result<Value, SynapticError> {
    let text = input.as_str().unwrap_or("");
    let count = text.split_whitespace().count();
    Ok(json!({"text": text, "word_count": count}))
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let pipeline = extract_text() | normalize() | word_count();
    let config = RunnableConfig::default();

    let input = json!({"content": "  Hello World  from Synaptic!  "});
    let result = pipeline.invoke(input, &config).await?;

    println!("结果: {}", result);
    // {"text": "hello world from synaptic!", "word_count": 4}
    Ok(())
}
```

### 场景 C：带中间件栈的 Agent

本示例展示如何将多个中间件宏组合成一个完整的 Agent 中间件栈，包含日志记录、重试和动态提示词功能。

```rust,ignore
use synaptic::core::{Message, SynapticError};
use synaptic::middleware::{AgentMiddleware, MiddlewareChain, ModelRequest, ModelResponse, ModelCaller};
use std::sync::Arc;

// 记录每次模型调用
#[after_model]
async fn log_response(request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError> {
    println!("[日志] 模型返回了 {} 个字符",
        response.message.content().len());
    Ok(())
}

// 模型调用失败时最多重试 2 次
#[wrap_model_call]
async fn retry_model(
    #[field] max_retries: usize,
    request: ModelRequest,
    next: &dyn ModelCaller,
) -> Result<ModelResponse, SynapticError> {
    let mut last_err = None;
    for _ in 0..=max_retries {
        match next.call(request.clone()).await {
            Ok(resp) => return Ok(resp),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap())
}

// 根据对话长度动态调整系统提示词
#[dynamic_prompt]
fn adaptive_prompt(messages: &[Message]) -> String {
    if messages.len() > 20 {
        "请简洁回答，总结而非展开。".into()
    } else {
        "你是一个有用的助手，请详细回答。".into()
    }
}

fn build_middleware_stack() -> Vec<Arc<dyn AgentMiddleware>> {
    vec![
        adaptive_prompt(),
        retry_model(2),
        log_response(),
    ]
}
```

### 场景 D：基于 Store 的笔记管理器（结合 schemars 类型化输入）

本示例将 `#[inject]` 运行时注入与 `schemars` 丰富 JSON Schema 生成结合使用。
`save_note` 工具接受一个自定义的 `NoteInput` 结构体，其完整 schema（标题、内容、标签）
对 LLM 可见；同时通过注入方式透明地获取共享 Store 和当前工具调用 ID。

**Cargo.toml** -- 启用 `agent`、`store` 和 `schemars` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["agent", "store", "schemars"] }
schemars = { version = "0.8", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

**完整示例：**

```rust,ignore
use std::sync::Arc;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use synaptic::core::{Store, SynapticError};
use synaptic::macros::tool;

// --- 使用 schemars 的自定义输入类型 ---
// 派生 JsonSchema 后，LLM 可以看到每个字段的完整描述，
// 包括嵌套的 Vec<String> 标签列表。

#[derive(Deserialize, JsonSchema)]
struct NoteInput {
    /// 笔记标题
    title: String,
    /// 笔记正文内容（支持 Markdown）
    content: String,
    /// 分类标签（例如 ["工作", "紧急"]）
    tags: Vec<String>,
}

// --- 启用 schemars 后 LLM 看到的 schema ---
//
// 生成的 `note` 参数 JSON Schema 如下：
//
// {
//   "type": "object",
//   "properties": {
//     "title":   { "type": "string", "description": "笔记标题" },
//     "content": { "type": "string", "description": "笔记正文内容（支持 Markdown）" },
//     "tags":    { "type": "array",  "items": { "type": "string" },
//                  "description": "分类标签（例如 [\"工作\", \"紧急\"]）" }
//   },
//   "required": ["title", "content", "tags"]
// }
//
// --- 未启用 schemars 时，同一参数只会生成： ---
//
// { "type": "object" }
//
// ...LLM 无法知道需要哪些字段。

/// 将笔记保存到共享 Store 中。
#[tool]
async fn save_note(
    /// 要保存的笔记（包含标题、内容和标签）
    note: NoteInput,
    /// 注入：持久化键值存储
    #[inject(store)]
    store: Arc<dyn Store>,
    /// 注入：当前工具调用 ID，用于追踪
    #[inject(tool_call_id)]
    call_id: String,
) -> Result<String, SynapticError> {
    // 使用工具调用 ID 构建唯一键
    let key = format!("note:{}", call_id);

    // 将笔记作为 JSON 条目持久化到 Store
    let value = json!({
        "title":   note.title,
        "content": note.content,
        "tags":    note.tags,
        "call_id": call_id,
    });

    store.put("notes", &key, value.clone()).await?;

    Ok(format!(
        "已保存笔记 '{}', 含 {} 个标签 [key={}]",
        note.title,
        note.tags.len(),
        key,
    ))
}

// 使用方式：
//   let tool = save_note();          // Arc<dyn RuntimeAwareTool>
//   assert_eq!(tool.name(), "save_note");
//
// LLM 只能看到 schema 中的 `note` 参数。
// `store` 和 `call_id` 由 ToolNode 在运行时自动注入。
```

**要点总结：**

- `NoteInput` 同时派生了 `Deserialize`（运行时反序列化）和 `JsonSchema`
  （编译期 schema 生成）。`Cargo.toml` 中必须启用 `schemars` feature，
  `#[tool]` 宏才能使用派生的 schema。
- `#[inject(store)]` 使工具可以直接访问共享的 `Store`，而不将其暴露给 LLM。
  `ToolNode` 在每次调用前从 `ToolRuntime` 中填充 store。
- `#[inject(tool_call_id)]` 提供当前调用的唯一标识符，可用于生成确定性的
  存储键或审计追踪。
- 由于使用了 `#[inject]`，宏生成的是 `RuntimeAwareTool`（而非普通 `Tool`）。
  工厂函数返回 `Arc<dyn RuntimeAwareTool>`。

### 场景 E：工作流入口、任务追踪与链路追踪

本示例展示 `#[entrypoint]`、`#[task]` 和 `#[traceable]` 三者如何协同工作，构建一个带链路追踪的数据处理工作流。

```rust,ignore
use synaptic::core::SynapticError;
use synaptic::macros::{entrypoint, task, traceable};
use serde_json::{json, Value};

// --- 带链路追踪的辅助函数 ---
// skip = "api_key" 确保密钥不会出现在 tracing span 中

/// 调用外部数据 API 获取原始数据。
#[traceable(name = "fetch_external_api", skip = "api_key")]
async fn call_external_api(url: String, api_key: String) -> Result<Value, SynapticError> {
    // 生产环境中使用 reqwest 发起真实请求
    Ok(json!({
        "source": url,
        "payload": [1, 2, 3, 4, 5]
    }))
}

// --- 任务步骤 ---
// #[task] 为每个步骤赋予稳定名称，便于流式输出和链路追踪识别

#[task(name = "fetch_data")]
async fn fetch_data(source_url: String, api_key: String) -> Result<Value, SynapticError> {
    let raw = call_external_api(source_url, api_key).await?;
    Ok(raw)
}

#[task(name = "transform_data")]
async fn transform_data(raw: Value) -> Result<Value, SynapticError> {
    // 提取 payload 并计算总和
    let items = raw["payload"].as_array()
        .ok_or_else(|| SynapticError::InvalidArgument("缺少 payload 字段".into()))?;
    let sum: i64 = items.iter().filter_map(|v| v.as_i64()).sum();
    Ok(json!({
        "source": raw["source"],
        "item_count": items.len(),
        "sum": sum,
    }))
}

// --- 工作流入口点 ---
// 将上述任务整合为一个入口，附带检查点支持

#[entrypoint(name = "data_pipeline", checkpointer = "memory")]
async fn data_pipeline(input: Value) -> Result<Value, SynapticError> {
    let url = input["url"].as_str().unwrap_or("https://api.example.com/data").to_string();
    let key = input["api_key"].as_str().unwrap_or("").to_string();

    let raw = fetch_data(url, key).await?;
    let result = transform_data(raw).await?;
    Ok(result)
}

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // 设置 tracing 订阅者以查看 span 输出
    // tracing_subscriber::fmt()
    //     .with_max_level(tracing::Level::INFO)
    //     .init();

    let ep = data_pipeline();
    println!("入口点名称: {}", ep.config.name);
    // => "data_pipeline"

    let input = json!({
        "url": "https://api.example.com/data",
        "api_key": "sk-secret-key"
    });

    // 调用入口点执行整个工作流
    let result = (ep.invoke_fn)(input).await?;
    println!("结果: {}", result);
    // => {"source": "https://api.example.com/data", "item_count": 5, "sum": 15}
    Ok(())
}
```

**要点：**

- `#[task]` 为每个步骤提供稳定的名称，便于流式输出和链路追踪识别
- `#[traceable]` 自动为函数创建 tracing span，`skip` 参数可隐藏敏感数据
- `#[entrypoint]` 将工作流整合为一个入口点，`checkpointer` 属性声明检查点后端
- 这些宏可自由组合使用——`#[task]` 步骤内部可调用 `#[traceable]` 函数，`#[entrypoint]` 可编排多个 `#[task]`

### 场景 F：工具权限控制与审计日志

本示例展示如何使用 `#[wrap_tool_call]` 配合 `#[field]` 实现工具白名单控制，以及 `#[before_agent]` 和 `#[after_agent]` 的生命周期审计日志。

```rust,ignore
use std::sync::Arc;
use synaptic::core::{Message, SynapticError};
use synaptic::macros::{wrap_tool_call, before_agent, after_agent};
use synaptic::middleware::{AgentMiddleware, ToolCallRequest, ToolCaller};
use serde_json::Value;

// --- 工具权限控制 ---
// #[field] allowed_tools 存储白名单，对 LLM 不可见
// 仅允许白名单中的工具执行，否则返回错误

#[wrap_tool_call]
async fn tool_permission_guard(
    #[field] allowed_tools: Vec<String>,
    request: ToolCallRequest,
    next: &dyn ToolCaller,
) -> Result<Value, SynapticError> {
    let tool_name = &request.call.name;
    if !allowed_tools.contains(tool_name) {
        return Err(SynapticError::InvalidArgument(
            format!("工具 '{}' 未在白名单中，拒绝执行", tool_name),
        ));
    }
    // 白名单通过，执行工具
    next.call(request).await
}

// --- Agent 启动审计 ---
// #[field] label 使中间件可配置，可在日志中标识不同的 Agent

#[before_agent]
async fn audit_start(
    #[field] label: String,
    messages: &mut Vec<Message>,
) -> Result<(), SynapticError> {
    println!("[审计] Agent 启动 (label={}, 初始消息数={})", label, messages.len());
    Ok(())
}

// --- Agent 结束审计 ---

#[after_agent]
async fn audit_end(messages: &mut Vec<Message>) -> Result<(), SynapticError> {
    println!("[审计] Agent 执行完毕 (最终消息数={})", messages.len());
    Ok(())
}

// --- 组装中间件栈 ---

fn build_secure_middleware_stack() -> Vec<Arc<dyn AgentMiddleware>> {
    vec![
        // 审计：记录 Agent 启动
        audit_start("生产环境 Agent".into()),
        // 权限：只允许 search 和 get_weather 两个工具
        tool_permission_guard(vec![
            "search".into(),
            "get_weather".into(),
        ]),
        // 审计：记录 Agent 结束
        audit_end(),
    ]
}
```

**要点：**

- `#[wrap_tool_call]` 可完全控制工具执行——批准、拒绝或转换参数均可
- `#[before_agent]` / `#[after_agent]` 包围整个 Agent 生命周期，适合审计日志和指标收集
- `#[field]` 使中间件可配置、可复用——同一个中间件可以为不同 Agent 配置不同的白名单或标签

### 场景 G：状态感知工具与原始参数转发

本示例展示 `#[inject(state)]` 如何让工具读取图状态，以及 `#[args]` 如何接收原始 JSON 参数。

```rust,ignore
use std::sync::Arc;
use synaptic::core::SynapticError;
use synaptic::macros::tool;
use serde_json::{json, Value};

// --- 状态感知工具 ---
// #[inject(state)] 让工具读取当前图状态（如对话轮次），
// 根据状态动态调整行为，而 LLM 无法感知状态的存在。

/// 根据对话轮次智能回复：轮次多时更简洁，轮次少时更详细。
#[tool]
async fn smart_reply(
    /// 回复主题
    topic: String,
    /// 注入：当前图状态
    #[inject(state)]
    state: Value,
) -> Result<String, SynapticError> {
    let turn_count = state["turn_count"].as_i64().unwrap_or(0);
    if turn_count > 10 {
        // 对话过长，返回简洁回复
        Ok(format!("[简洁] {}", topic))
    } else {
        // 对话初期，返回详细回复
        Ok(format!("[详细] 关于"{}"，以下是详细说明……", topic))
    }
}

// --- 原始 JSON 转发工具 ---
// #[args] 跳过 schema 生成，接受任意 JSON 负载。
// 适用于 webhook 转发、日志收集等不确定输入结构的场景。

/// 将任意 JSON 负载转发到 webhook 端点。
#[tool(name = "webhook_forward")]
async fn webhook_forward(#[args] payload: Value) -> Result<Value, SynapticError> {
    // 生产环境中使用 reqwest 发送 HTTP 请求
    println!("转发到 webhook: {:?}", payload);
    Ok(json!({
        "status": "forwarded",
        "payload_size": payload.to_string().len(),
    }))
}

// --- 可配置的 API 代理工具 ---
// #[field] 提供构造时配置，#[args] 接受运行时原始参数。
// 两者组合实现一个可复用的代理工具。

/// 将请求代理到可配置的 API 端点。
#[tool(name = "api_proxy")]
async fn api_proxy(
    #[field] endpoint: String,
    #[field] auth_header: String,
    #[args] body: Value,
) -> Result<Value, SynapticError> {
    // 生产环境中使用 endpoint 和 auth_header 发送请求
    println!("代理到 {} (auth={})", endpoint, auth_header);
    Ok(json!({
        "endpoint": endpoint,
        "status": "proxied",
        "body_keys": body.as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    }))
}

// 使用方式：
//
// smart_reply 是 RuntimeAwareTool（因为使用了 #[inject]）
// let reply_tool = smart_reply(); // Arc<dyn RuntimeAwareTool>
//
// webhook_forward 是普通 Tool（#[args] 不影响 trait 类型）
// let webhook = webhook_forward(); // Arc<dyn Tool>
//
// api_proxy 在构造时需要传入 field 参数
// let proxy = api_proxy(
//     "https://internal.api.example.com/v1".into(),
//     "Bearer sk-xxx".into(),
// ); // Arc<dyn Tool>
```

**要点：**

- `#[inject(state)]` 让工具读取图状态，而不将状态暴露给 LLM——适合根据对话进度动态调整行为
- `#[args]` 跳过 schema 生成，接受任意 JSON 负载——适用于转发、代理等不确定输入结构的场景
- 可与 `#[field]` 组合实现可配置的转发工具——`#[field]` 在构造时提供端点和认证信息，`#[args]` 在运行时接收任意请求体

---

## 与 Python LangChain 对比

下表展示了 Python LangChain 装饰器与 Synaptic Rust 宏的对应关系：

| Python LangChain | Synaptic (Rust) | 说明 |
|---|---|---|
| `@tool` | `#[tool]` | 定义工具。Python 用类型注解，Rust 用原生类型映射 JSON Schema |
| `RunnableLambda(fn)` | `#[chain]` | 创建可运行单元。Rust 宏自动生成 `RunnableLambda` 包装 |
| `@entrypoint` | `#[entrypoint]` | LangGraph 工作流入口 |
| `@task` | `#[task]` | LangGraph 可追踪任务 |
| 自定义 `RunnableMiddleware` | `#[before_agent]` 等 | Python 通常手写中间件类，Rust 用宏一行生成 |
| `langsmith.traceable` | `#[traceable]` | Python 装饰器 vs Rust 属性宏，均基于 span 概念 |
| `InjectedState` 类型注解 | `#[inject(state)]` | Python 用 `Annotated[T, InjectedState]`，Rust 用参数属性 |
| `InjectedStore` 类型注解 | `#[inject(store)]` | 同上 |
| `InjectedToolCallId` 类型注解 | `#[inject(tool_call_id)]` | 同上 |

**主要区别：**

- **类型安全**：Rust 宏在编译期生成 JSON Schema 并进行类型检查，Python 在运行时进行。
- **零成本抽象**：生成的结构体和 trait 实现在编译后没有额外的间接开销。
- **显式异步**：所有异步钩子需要标注 `async fn`，`#[dynamic_prompt]` 明确要求同步函数。
- **返回类型**：工厂函数返回 `Arc<dyn Trait>` 而非裸对象，便于在多线程运行时中共享。

---

## 工具定义如何到达 LLM

了解从 Rust 函数到 LLM 工具调用的完整链路，有助于调试 schema 问题和自定义行为。以下是完整的流程：

```text
#[tool] 宏
    │
    ▼
struct + impl Tool    （编译期生成）
    │
    ▼
tool.as_tool_definition() → ToolDefinition { name, description, parameters }
    │
    ▼
ChatRequest::with_tools(vec![...])    （工具定义附加到请求上）
    │
    ▼
模型适配器 (OpenAI / Anthropic / Gemini)
    │   将 ToolDefinition 转换为供应商特定的 JSON
    │   例如 OpenAI: {"type": "function", "function": {"name": ..., "parameters": ...}}
    ▼
HTTP POST → LLM API
    │
    ▼
LLM 返回 ToolCall { id, name, arguments }
    │
    ▼
ToolNode 分发 → tool.call(arguments)
    │
    ▼
Tool Message 回到对话中
```

**代码库中的关键文件：**

| 步骤 | 文件 |
|------|------|
| `#[tool]` 宏展开 | `crates/synaptic-macros/src/tool.rs` |
| `Tool` / `RuntimeAwareTool` trait | `crates/synaptic-core/src/lib.rs` |
| `ToolDefinition`、`ToolCall` 类型 | `crates/synaptic-core/src/lib.rs` |
| `ToolNode`（分发调用） | `crates/synaptic-graph/src/tool_node.rs` |
| OpenAI 适配器 | `crates/synaptic-models/src/openai.rs` |
| Anthropic 适配器 | `crates/synaptic-models/src/anthropic.rs` |
| Gemini 适配器 | `crates/synaptic-models/src/gemini.rs` |

## 测试宏生成的代码

`#[tool]` 生成的工具可以像任何其他 `Tool` 实现一样进行测试。调用 `as_tool_definition()` 检查 schema，调用 `call()` 验证行为：

```rust,ignore
use serde_json::json;
use synaptic::core::Tool;

/// 两数相加。
#[tool]
async fn add(
    /// 第一个数
    a: f64,
    /// 第二个数
    b: f64,
) -> Result<serde_json::Value, SynapticError> {
    Ok(json!({"result": a + b}))
}

#[tokio::test]
async fn test_add_tool() {
    let tool = add();

    // 验证元数据
    assert_eq!(tool.name(), "add");
    assert_eq!(tool.description(), "两数相加。");

    // 验证 schema
    let def = tool.as_tool_definition();
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("a")));
    assert!(required.contains(&json!("b")));

    // 验证执行
    let result = tool.call(json!({"a": 3.0, "b": 4.0})).await.unwrap();
    assert_eq!(result["result"], 7.0);
}
```

对于 `#[chain]` 宏，使用 `invoke()` 测试返回的 `BoxRunnable`：

```rust,ignore
use synaptic::core::RunnableConfig;
use synaptic::runnables::Runnable;

#[chain]
async fn to_upper(s: String) -> Result<String, SynapticError> {
    Ok(s.to_uppercase())
}

#[tokio::test]
async fn test_chain() {
    let runnable = to_upper();
    let config = RunnableConfig::default();
    let result = runnable.invoke("hello".into(), &config).await.unwrap();
    assert_eq!(result, "HELLO");
}
```

### 常见问题

1. **自定义类型未启用 `schemars`**：参数的 schema 为 `{"type": "object"}`，不包含任何字段细节。LLM 只能猜测（通常猜错）应该传什么。
   **解决方法**：启用 `schemars` feature 并派生 `JsonSchema`。

2. **遗漏 `as_tool_definition()` 调用**：如果手动用 `json!({})` 构建 `ToolDefinition` 的 parameters 而不调用 `tool.as_tool_definition()`，schema 将为空。
   **解决方法**：始终对你的 `Tool` / `RuntimeAwareTool` 使用 `as_tool_definition()`。

3. **OpenAI strict 模式**：OpenAI 的函数调用 strict 模式会拒绝缺少 `type` 字段的 schema。所有内置类型和 `Value` 现在都会生成包含 `"type"` 的有效 schema。
