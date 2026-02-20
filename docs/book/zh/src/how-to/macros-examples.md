# 宏使用示例

以下七个端到端场景展示了各种宏在实际应用中的协作方式。

## 场景 A：带自定义工具的天气 Agent

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

## 场景 B：使用 Chain 宏构建数据处理流水线

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

## 场景 C：带中间件栈的 Agent

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

## 场景 D：基于 Store 的笔记管理器（结合 schemars 类型化输入）

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

## 场景 E：工作流入口、任务追踪与链路追踪

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

## 场景 F：工具权限控制与审计日志

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

## 场景 G：状态感知工具与原始参数转发

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
