# Middleware Overview

The middleware system intercepts and modifies agent behavior at every lifecycle point -- before/after the agent run, before/after each model call, and around each tool call. Use middleware when you need cross-cutting concerns (rate limiting, retries, context management) without modifying your agent logic.

## AgentMiddleware Trait

All methods have default no-op implementations. Override only the hooks you need.

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

## Lifecycle Diagram

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

`before_agent` and `after_agent` run once per invocation. The inner loop repeats for each agent step (model call followed by tool execution). `before_model` / `after_model` run around every model call and can mutate the request or response. `wrap_model_call` and `wrap_tool_call` are onion-style wrappers that receive a `next` caller to delegate to the next layer.

## MiddlewareChain

`MiddlewareChain` composes multiple middlewares and executes them in registration order for `before_*` hooks, and in reverse order for `after_*` hooks.

```rust,ignore
use synaptic_middleware::MiddlewareChain;

let chain = MiddlewareChain::new(vec![
    Arc::new(ModelCallLimitMiddleware::new(10)),
    Arc::new(ToolRetryMiddleware::new(3)),
]);
```

## Using Middleware with `create_agent`

Pass middlewares through `AgentOptions::middleware`. The agent graph wires them into both the model node and the tool node automatically.

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

## Built-in Middlewares

| Middleware | Hook Used | Description |
|-----------|-----------|-------------|
| [`ModelCallLimitMiddleware`](model-call-limit.md) | `wrap_model_call` | Limits model invocations per run |
| [`ToolCallLimitMiddleware`](tool-call-limit.md) | `wrap_tool_call` | Limits tool invocations per run |
| [`ToolRetryMiddleware`](tool-retry.md) | `wrap_tool_call` | Retries failed tools with exponential backoff |
| [`ModelFallbackMiddleware`](model-fallback.md) | `wrap_model_call` | Falls back to alternative models on failure |
| [`SummarizationMiddleware`](summarization.md) | `before_model` | Auto-summarizes when context exceeds token limit |
| [`TodoListMiddleware`](todo-list.md) | `before_model` | Injects a task list into the agent context |
| [`HumanInTheLoopMiddleware`](human-in-the-loop.md) | `wrap_tool_call` | Pauses for human approval before tool execution |
| [`ContextEditingMiddleware`](context-editing.md) | `before_model` | Trims or filters context before model calls |

## Writing a Custom Middleware

Implement `AgentMiddleware` for your struct and override the hooks you need.

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

Then add it to your agent:

```rust,ignore
let options = AgentOptions {
    middleware: vec![Arc::new(LoggingMiddleware)],
    ..Default::default()
};
let graph = create_agent(model, tools, options)?;
```
