# Middleware 概述

Middleware 系统在 Agent 生命周期的每个节点拦截和修改行为——Agent 运行前/后、每次模型调用前/后，以及每次工具调用前后。当你需要处理横切关注点（速率限制、重试、上下文管理）而不修改 Agent 逻辑时，可以使用 Middleware。

## AgentMiddleware Trait

所有方法都有默认的空实现。只需重写你需要的钩子方法即可。

```rust,ignore
#[async_trait]
pub trait AgentMiddleware: Send + Sync {
    async fn before_agent(&self, messages: &mut Vec<Message>) -> Result<(), SynapticError>;
    async fn after_agent(&self, messages: &mut Vec<Message>) -> Result<(), SynapticError>;
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError>;
    async fn after_model(&self, request: &ModelRequest, response: &mut ModelResponse) -> Result<(), SynapticError>;
    async fn wrap_model_call(&self, request: ModelRequest, next: &dyn ModelCaller) -> Result<ModelResponse, SynapticError>;
    async fn wrap_tool_call(&self, request: ToolCallRequest, next: &dyn ToolCaller) -> Result<Value, SynapticError>;
}
```

## 生命周期图

```text
before_agent(messages)
  loop {
    before_model(request)
      -> wrap_model_call(request, next)
    after_model(request, response)
    for each tool_call {
      wrap_tool_call(request, next)
    }
  }
after_agent(messages)
```

`before_agent` 和 `after_agent` 在每次调用中各执行一次。内部循环在每个 Agent 步骤（模型调用后跟工具执行）中重复执行。`before_model` / `after_model` 在每次模型调用前后执行，可以修改请求或响应。`wrap_model_call` 和 `wrap_tool_call` 是洋葱式包装器，接收一个 `next` 调用器以委托给下一层。

## MiddlewareChain

`MiddlewareChain` 组合多个 Middleware，对 `before_*` 钩子按注册顺序执行，对 `after_*` 钩子按反序执行。

```rust,ignore
use synaptic_middleware::MiddlewareChain;

let chain = MiddlewareChain::new(vec![
    Arc::new(ModelCallLimitMiddleware::new(10)),
    Arc::new(ToolRetryMiddleware::new(3)),
]);
```

## 在 `create_agent` 中使用 Middleware

通过 `AgentOptions::middleware` 传入 Middleware。Agent 图会自动将它们连接到模型节点和工具节点。

```rust,ignore
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::{ModelCallLimitMiddleware, ToolRetryMiddleware};

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(10)),
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 内置 Middleware

| Middleware | 使用的钩子 | 说明 |
|-----------|-----------|------|
| [`ModelCallLimitMiddleware`](model-call-limit.md) | `wrap_model_call` | 限制每次运行的模型调用次数 |
| [`ToolCallLimitMiddleware`](tool-call-limit.md) | `wrap_tool_call` | 限制每次运行的工具调用次数 |
| [`ToolRetryMiddleware`](tool-retry.md) | `wrap_tool_call` | 以指数退避重试失败的工具调用 |
| [`ModelFallbackMiddleware`](model-fallback.md) | `wrap_model_call` | 主模型失败时回退到备用模型 |
| [`SummarizationMiddleware`](summarization.md) | `before_model` | 上下文超过 token 限制时自动摘要 |
| [`TodoListMiddleware`](todo-list.md) | `before_model` | 向 Agent 上下文注入任务列表 |
| [`HumanInTheLoopMiddleware`](human-in-the-loop.md) | `wrap_tool_call` | 在工具执行前暂停以等待人工审批 |
| [`ContextEditingMiddleware`](context-editing.md) | `before_model` | 在模型调用前裁剪或过滤上下文 |

## 编写自定义 Middleware

为你的结构体实现 `AgentMiddleware`，并重写需要的钩子方法。

```rust,ignore
use synaptic_middleware::{AgentMiddleware, ModelRequest};

struct LoggingMiddleware;

#[async_trait]
impl AgentMiddleware for LoggingMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        println!("Model call with {} messages", request.messages.len());
        Ok(())
    }
}
```

然后将其添加到 Agent 中：

```rust,ignore
let options = AgentOptions {
    middleware: vec![Arc::new(LoggingMiddleware)],
    ..Default::default()
};
let graph = create_agent(model, tools, options)?;
```
