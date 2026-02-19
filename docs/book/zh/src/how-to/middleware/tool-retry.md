# ToolRetryMiddleware

以指数退避策略重试失败的工具调用。当工具可能遇到瞬态故障（网络超时、速率限制、临时不可用）且你希望自动恢复而不将错误暴露给模型时，可以使用此 Middleware。

## 构造函数

```rust,ignore
use synaptic_middleware::ToolRetryMiddleware;

// Retry up to 3 times (4 total attempts including the first)
let mw = ToolRetryMiddleware::new(3);
```

### 配置

重试之间的基础延迟默认为 100ms，每次重试翻倍（指数退避）。你可以使用 `with_base_delay` 自定义：

```rust,ignore
use std::time::Duration;

let mw = ToolRetryMiddleware::new(3)
    .with_base_delay(Duration::from_millis(500));
// Delays: 500ms, 1000ms, 2000ms
```

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ToolRetryMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `wrap_tool_call`
- 当工具调用失败时，Middleware 等待 `base_delay * 2^attempt` 后重试。
- 重试最多进行 `max_retries` 次。如果所有重试均失败，返回最后一次错误。
- 如果工具调用在任一次尝试中成功，立即返回结果。

使用默认 100ms 基础延迟时的退避时间表：

| 尝试次数 | 重试前延迟 |
|---------|-----------|
| 第 1 次重试 | 100ms |
| 第 2 次重试 | 200ms |
| 第 3 次重试 | 400ms |

## 与工具调用限制组合

当两个 Middleware 同时生效时，重试 Middleware 在工具调用限制内运行。每次重试都计为一次单独的工具调用：

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolCallLimitMiddleware::new(30)),
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```
