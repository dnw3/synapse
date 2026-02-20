# 中间件宏

Synaptic 提供了 7 个中间件宏，分别对应 Agent 执行生命周期中的不同钩子点。每个宏的生成模式一致：

1. 生成一个名为 `{PascalCase}Middleware` 的结构体（例如 `setup` -> `SetupMiddleware`）。
2. 为该结构体实现 `synaptic::middleware::AgentMiddleware` trait，仅重写对应的钩子方法。
3. 生成与函数同名的工厂函数，返回 `Arc<dyn AgentMiddleware>`。

## `#[before_agent]`

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

## `#[before_model]`

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

## `#[after_model]`

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

## `#[after_agent]`

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

## `#[wrap_model_call]`

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

## `#[wrap_tool_call]`

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

## `#[dynamic_prompt]`

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

## 有状态中间件与 `#[field]`

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
