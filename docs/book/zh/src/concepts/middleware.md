# 中间件

中间件在定义明确的生命周期节点拦截和转换智能体行为。中间件不直接修改智能体逻辑，而是包裹在模型调用和工具调用外层，添加横切关注点，如速率限制、人工审批、摘要生成和上下文管理。本页介绍中间件抽象、生命周期钩子以及可用的中间件类别。

## AgentMiddleware Trait

所有中间件实现一个包含六个钩子的 trait：

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

每个钩子都有默认实现，直接透传不做修改。中间件只需覆盖它需要的钩子即可。

## 生命周期

单次智能体轮次遵循以下顺序：

```
before_agent → before_model → wrap_model_call → after_model → wrap_tool_call（每个工具） → after_agent
```

1. **`before_agent`** -- 在每次智能体轮次开始时调用一次。用于初始化、日志记录或状态检查。
2. **`before_model`** -- 在 LLM 请求之前调用。可以修改消息（如注入上下文、裁剪历史记录）。
3. **`wrap_model_call`** -- 包裹实际的模型调用。可以进行重试、添加降级方案，或完全替换调用。
4. **`after_model`** -- 在 LLM 响应之后调用。可以修改响应（如修复工具调用、添加元数据）。
5. **`wrap_tool_call`** -- 包裹每个工具调用。可以审批/拒绝、添加日志，或修改参数。
6. **`after_agent`** -- 在每次智能体轮次结束时调用一次。用于清理或状态持久化。

## MiddlewareChain

多个中间件实例组合成一个 `MiddlewareChain`。链对 "before" 钩子按顺序应用，对 "after" 钩子按逆序应用（洋葱模型）：

```rust
use synaptic::middleware::MiddlewareChain;

let chain = MiddlewareChain::new(vec![
    Arc::new(ToolCallLimitMiddleware::new(10)),
    Arc::new(HumanInTheLoopMiddleware::new(callback)),
    Arc::new(SummarizationMiddleware::new(model, 4000)),
]);
```

## 可用中间件

### ToolCallLimitMiddleware

限制每个智能体会话中工具调用的总次数。当达到上限时，后续的工具调用会返回错误而不执行。

- **使用场景**：防止智能体在无限循环中反复调用工具导致失控。
- **配置**：`ToolCallLimitMiddleware::new(max_calls)`

### HumanInTheLoopMiddleware

在工具调用执行前通过审批回调进行路由。回调接收工具名称和参数，并返回审批决定。

- **使用场景**：需要人工审核的高风险操作（数据库写入、外部 API 调用）。
- **配置**：`HumanInTheLoopMiddleware::new(callback)` 或 `.for_tools(vec!["dangerous_tool"])` 仅保护特定工具。

### SummarizationMiddleware

监控消息历史长度，当超过 token 阈值时对较早的消息进行摘要。用摘要替换较远的消息，同时保留最近的消息。

- **使用场景**：积累大量消息历史的长期运行智能体。
- **配置**：`SummarizationMiddleware::new(summarizer_model, token_threshold)`

### ContextEditingMiddleware

在每次模型调用前使用可配置策略转换消息历史：

- **`ContextStrategy::LastN(n)`** -- 仅保留最后 N 条消息（保留开头的系统消息）。
- **`ContextStrategy::StripToolCalls`** -- 移除工具调用/结果消息，仅保留人类和 AI 的内容消息。

### ModelRetryMiddleware

用重试逻辑包裹模型调用，在遇到临时故障时多次尝试调用。

### ModelFallbackMiddleware

在主模型失败时提供降级模型。按顺序尝试备选模型，直到有一个成功。

## 中间件与图特性的对比

中间件和图特性（检查点、中断）服务于不同的目的：

| 关注点 | 中间件 | 图 |
|---------|--------|-----|
| 工具审批 | HumanInTheLoopMiddleware | interrupt_before("tools") |
| 上下文管理 | ContextEditingMiddleware | 自定义节点逻辑 |
| 速率限制 | ToolCallLimitMiddleware | 不适用 |
| 状态持久化 | 不适用 | Checkpointer |

中间件在单个智能体节点内运行。图特性在整个图上运行。对于每轮次的关注点使用中间件，对于工作流级别的关注点使用图特性。

## 另请参阅

- [中间件使用指南](../how-to/middleware/index.md) -- 每种中间件的详细用法
- [工具调用限制](../how-to/middleware/tool-call-limit.md) -- 限制工具调用次数
- [人机协作](../how-to/middleware/human-in-the-loop.md) -- 审批工作流
- [摘要生成](../how-to/middleware/summarization.md) -- 自动上下文摘要
- [上下文编辑](../how-to/middleware/context-editing.md) -- 消息历史策略
