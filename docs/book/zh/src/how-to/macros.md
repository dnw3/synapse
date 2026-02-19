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

将一个异步函数转换为实现 `synaptic_core::Tool` trait 的结构体。宏会自动完成以下工作：

1. 生成一个名为 `{PascalCase}Tool` 的结构体（例如 `search` -> `SearchTool`）。
2. 根据函数签名自动构建 JSON Schema，供 LLM 调用时使用。
3. 生成与函数同名的工厂函数，返回 `Arc<dyn Tool>`。

### 基本用法

```rust
use synaptic::tool;
use synaptic_core::SynapticError;

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

### 运行时注入 (`#[inject(state)]`, `#[inject(store)]`, `#[inject(tool_call_id)]`)

使用 `#[inject(...)]` 标记的参数不会出现在 LLM 可见的 JSON Schema 中，而是在工具执行时由运行时自动注入。使用任意一个 `#[inject]` 属性后，宏生成的实现从 `Tool` trait 切换为 `RuntimeAwareTool` trait，工厂函数返回 `Arc<dyn RuntimeAwareTool>`。

支持三种注入源：

| 注入标记 | 注入内容 |
|---|---|
| `#[inject(state)]` | Agent 运行时的状态 (`Value`)，从 `ToolRuntime.state` 反序列化 |
| `#[inject(store)]` | 共享存储引用，从 `ToolRuntime.store` 获取 |
| `#[inject(tool_call_id)]` | 当前工具调用的 ID (`String`)，从 `ToolRuntime.tool_call_id` 获取 |

```rust
use synaptic_core::{SynapticError, ToolRuntime};
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
use synaptic_core::SynapticError;
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

定义 LangGraph 风格的工作流入口。宏将异步函数转换为返回 `synaptic_core::Entrypoint` 的工厂函数。

函数必须：
- 是 `async` 的
- 接受恰好一个参数（`serde_json::Value`）
- 返回 `Result<Value, SynapticError>`

### 基本用法

```rust
use synaptic::entrypoint;
use synaptic_core::SynapticError;
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
use synaptic_core::SynapticError;

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
2. 为该结构体实现 `synaptic_middleware::AgentMiddleware` trait，仅重写对应的钩子方法。
3. 生成与函数同名的工厂函数，返回 `Arc<dyn AgentMiddleware>`。

### `#[before_agent]`

在 Agent 循环**开始前**执行。函数签名：`async fn(messages: &mut Vec<Message>) -> Result<(), SynapticError>`

```rust
use synaptic::before_agent;
use synaptic_core::{Message, SynapticError};

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
use synaptic_middleware::ModelRequest;
use synaptic_core::SynapticError;

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
use synaptic_middleware::{ModelRequest, ModelResponse};
use synaptic_core::SynapticError;

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
use synaptic_core::{Message, SynapticError};

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
use synaptic_middleware::{ModelRequest, ModelResponse, ModelCaller};
use synaptic_core::SynapticError;

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
use synaptic_middleware::{ToolCallRequest, ToolCaller};
use synaptic_core::SynapticError;
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
use synaptic_core::Message;

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

> 所有中间件宏均不接受属性参数。

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
