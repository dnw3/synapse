# ContextEditingMiddleware

在每次模型调用前裁剪或过滤对话上下文。当不需要完整摘要但需要保持上下文窗口可控时使用此 Middleware——例如，丢弃旧消息或从历史记录中去除工具调用噪声。

## 构造函数

该 Middleware 接受一个 `ContextStrategy` 来定义消息的编辑方式：

```rust,ignore
use synaptic_middleware::{ContextEditingMiddleware, ContextStrategy};

// Keep only the last 10 non-system messages
let mw = ContextEditingMiddleware::new(ContextStrategy::LastN(10));

// Remove tool call/result pairs, keeping only human/AI content messages
let mw = ContextEditingMiddleware::new(ContextStrategy::StripToolCalls);

// Strip tool calls first, then keep last N
let mw = ContextEditingMiddleware::new(ContextStrategy::StripAndTruncate(10));
```

### 便捷构造函数

```rust,ignore
let mw = ContextEditingMiddleware::last_n(10);
let mw = ContextEditingMiddleware::strip_tool_calls();
```

## 策略说明

| 策略 | 行为 |
|------|------|
| `LastN(n)` | 保留开头的系统消息，然后保留最后 `n` 条非系统消息 |
| `StripToolCalls` | 移除 `Tool` 消息和仅包含工具调用（无文本）的 AI 消息 |
| `StripAndTruncate(n)` | 先应用 `StripToolCalls`，再应用 `LastN(n)` |

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ContextEditingMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ContextEditingMiddleware::last_n(20)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `before_model`
- 在每次模型调用前，Middleware 对 `request.messages` 应用配置的策略。
- **LastN：** 列表开头的系统消息始终保留。从剩余消息中只保留最后 `n` 条。更早的消息被丢弃。
- **StripToolCalls：** `is_tool() == true` 的消息被移除。有工具调用但文本内容为空的 AI 消息也被移除。这清理了工具调用/工具结果的配对，同时保留了对话内容。
- **StripAndTruncate：** 按顺序执行两个过滤器——先去除工具调用，再截断到最后 N 条。

Agent 状态中的原始消息列表不会被修改；只有发送给模型的请求被裁剪。

## 示例：与摘要组合使用

为了最大化上下文效率，先去除工具调用，然后对剩余内容进行摘要：

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ContextEditingMiddleware::strip_tool_calls()),
        Arc::new(SummarizationMiddleware::new(model.clone(), 4000, |msg| msg.content().len() / 4)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

上下文编辑器在摘要运行前去除工具噪声，从而产生更干净的摘要。
