# Middleware

Middleware intercepts and transforms agent behavior at well-defined lifecycle points. Rather than modifying agent logic directly, middleware wraps around model calls and tool calls, adding cross-cutting concerns like rate limiting, human approval, summarization, and context management. This page explains the middleware abstraction, the lifecycle hooks, and the available middleware classes.

## The AgentMiddleware Trait

All middleware implements a single trait with six hooks:

```rust
#[async_trait]
pub trait AgentMiddleware: Send + Sync {
    async fn before_agent(&self, state: &MessageState) -> Result<(), SynapticError> { Ok(()) }
    async fn after_agent(&self, state: &MessageState) -> Result<(), SynapticError> { Ok(()) }
    async fn before_model(&self, messages: &mut Vec<Message>) -> Result<(), SynapticError> { Ok(()) }
    async fn after_model(&self, response: &mut ChatResponse) -> Result<(), SynapticError> { Ok(()) }
    async fn wrap_model_call(&self, messages: Vec<Message>, next: ModelCallFn) -> Result<ChatResponse, SynapticError>;
    async fn wrap_tool_call(&self, name: &str, args: &Value, next: ToolCallFn) -> Result<Value, SynapticError>;
}
```

Each hook has a default implementation that passes through unchanged. Middleware only overrides the hooks it needs.

## Lifecycle

A single agent turn follows this sequence:

```
before_agent → before_model → wrap_model_call → after_model → wrap_tool_call (per tool) → after_agent
```

1. **`before_agent`** -- called once at the start of each agent turn. Use for setup, logging, or state inspection.
2. **`before_model`** -- called before the LLM request. Can modify messages (e.g., inject context, trim history).
3. **`wrap_model_call`** -- wraps the actual model invocation. Can retry, add fallbacks, or replace the call entirely.
4. **`after_model`** -- called after the LLM responds. Can modify the response (e.g., fix tool calls, add metadata).
5. **`wrap_tool_call`** -- wraps each tool invocation. Can approve/reject, add logging, or modify arguments.
6. **`after_agent`** -- called once at the end of each agent turn. Use for cleanup or state persistence.

## MiddlewareChain

Multiple middleware instances are composed into a `MiddlewareChain`. The chain applies middleware in order for "before" hooks and in reverse order for "after" hooks (onion model):

```rust
use synaptic::middleware::MiddlewareChain;

let chain = MiddlewareChain::new(vec![
    Arc::new(ToolCallLimitMiddleware::new(10)),
    Arc::new(HumanInTheLoopMiddleware::new(callback)),
    Arc::new(SummarizationMiddleware::new(model, 4000)),
]);
```

## Available Middleware

### ToolCallLimitMiddleware

Limits the total number of tool calls per agent session. When the limit is reached, subsequent tool calls return an error instead of executing.

- **Use case**: Preventing runaway agents that call tools in an infinite loop.
- **Configuration**: `ToolCallLimitMiddleware::new(max_calls)`

### HumanInTheLoopMiddleware

Routes tool calls through an approval callback before execution. The callback receives the tool name and arguments and returns an approval decision.

- **Use case**: High-stakes operations (database writes, external API calls) that require human review.
- **Configuration**: `HumanInTheLoopMiddleware::new(callback)` or `.for_tools(vec!["dangerous_tool"])` to guard only specific tools.

### SummarizationMiddleware

Monitors message history length and summarizes older messages when a token threshold is exceeded. Replaces distant messages with a summary while preserving recent ones.

- **Use case**: Long-running agents that accumulate large message histories.
- **Configuration**: `SummarizationMiddleware::new(summarizer_model, token_threshold)`

### ContextEditingMiddleware

Transforms the message history before each model call using a configurable strategy:

- **`ContextStrategy::LastN(n)`** -- keep only the last N messages (preserving leading system messages).
- **`ContextStrategy::StripToolCalls`** -- remove tool call/result messages, keeping only human and AI content messages.

### ModelRetryMiddleware

Wraps the model call with retry logic, attempting the call multiple times on transient failures.

### ModelFallbackMiddleware

Provides fallback models when the primary model fails. Tries alternatives in order until one succeeds.

## Middleware vs. Graph Features

Middleware and graph features (checkpointing, interrupts) serve different purposes:

| Concern | Middleware | Graph |
|---------|-----------|-------|
| Tool approval | HumanInTheLoopMiddleware | interrupt_before("tools") |
| Context management | ContextEditingMiddleware | Custom node logic |
| Rate limiting | ToolCallLimitMiddleware | Not applicable |
| State persistence | Not applicable | Checkpointer |

Middleware operates within a single agent node. Graph features operate across the entire graph. Use middleware for per-turn concerns and graph features for workflow-level concerns.

## See Also

- [Middleware How-to Guides](../how-to/middleware/index.md) -- detailed usage for each middleware class
- [Tool Call Limit](../how-to/middleware/tool-call-limit.md) -- limiting tool calls
- [Human-in-the-Loop](../how-to/middleware/human-in-the-loop.md) -- approval workflows
- [Summarization](../how-to/middleware/summarization.md) -- automatic context summarization
- [Context Editing](../how-to/middleware/context-editing.md) -- message history strategies
