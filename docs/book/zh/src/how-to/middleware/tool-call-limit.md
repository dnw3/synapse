# ToolCallLimitMiddleware

限制单次 Agent 运行中工具调用的次数。当 Agent 可能在循环中产生过多工具调用时，使用此 Middleware 来限制工具使用量。

## 构造函数

```rust,ignore
use synaptic_middleware::ToolCallLimitMiddleware;

let mw = ToolCallLimitMiddleware::new(20); // max 20 tool calls
```

该 Middleware 提供 `call_count()` 和 `reset()` 方法，用于查看和手动重置计数器。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ToolCallLimitMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ToolCallLimitMiddleware::new(20)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `wrap_tool_call`
- 每次分派工具调用时，Middleware 通过原子操作递增内部计数器。
- 如果计数器已达到或超过 `max_calls`，则返回 `SynapticError::MaxStepsExceeded`，不执行工具。
- 否则，正常委托给 `next.call(request)`。

计数器跟踪的是单个工具调用，而非 Agent 步骤。如果一次模型响应请求了三个工具调用，计数器会递增三次。这使你可以精确控制整个 Agent 运行过程中的工具使用总量。

## 组合模型和工具限制

两种限制可以同时应用，以防范不同的故障模式：

```rust,ignore
use synaptic_middleware::{ModelCallLimitMiddleware, ToolCallLimitMiddleware};

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(10)),
        Arc::new(ToolCallLimitMiddleware::new(30)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

任一限制触发时，Agent 都会停止。

## 处理错误

当超过限制时，Middleware 返回 `SynapticError::MaxStepsExceeded`。你可以捕获此错误以提供优雅的降级处理：

```rust,ignore
use synaptic_core::SynapticError;

let mut state = MessageState::new();
state.messages.push(Message::human("Do something complex."));

match graph.invoke(state).await {
    Ok(result) => println!("{}", result.into_state().messages.last().unwrap().content()),
    Err(SynapticError::MaxStepsExceeded(msg)) => {
        println!("Agent hit tool call limit: {msg}");
        // Retry with a higher limit, summarize progress, or inform the user
    }
    Err(e) => println!("Other error: {e}"),
}
```

## 查看和重置计数

该 Middleware 提供查看和重置计数器的方法：

```rust,ignore
let mw = ToolCallLimitMiddleware::new(10);

// After an agent run, check how many tool calls were made
println!("Tool calls used: {}", mw.call_count());

// Reset the counter for a new run
mw.reset();
assert_eq!(mw.call_count(), 0);
```
