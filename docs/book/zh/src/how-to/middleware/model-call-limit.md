# ModelCallLimitMiddleware

限制单次 Agent 运行中模型调用的次数，防止失控循环。当你需要对每次调用中 LLM 被调用的次数设置硬性上限时，可以使用此 Middleware。

## 构造函数

```rust,ignore
use synaptic_middleware::ModelCallLimitMiddleware;

let mw = ModelCallLimitMiddleware::new(10); // max 10 model calls
```

该 Middleware 还提供 `call_count()` 方法查看当前计数，以及 `reset()` 方法将计数归零。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::ModelCallLimitMiddleware;

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(5)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `wrap_model_call`
- 在委托给下一层之前，Middleware 通过原子操作递增内部计数器。
- 如果计数器已达到或超过 `max_calls`，则立即返回 `SynapticError::MaxStepsExceeded`，不再调用模型。
- 否则，正常委托给 `next.call(request)`。

这意味着一旦达到限制，Agent 循环将以错误终止。计数器在整个 Agent 调用过程中持续存在（Agent 循环的所有步骤），因此限制为 5 表示最多进行 5 次模型往返。

## 示例：与其他 Middleware 组合

```rust,ignore
let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelCallLimitMiddleware::new(10)),
        Arc::new(ToolRetryMiddleware::new(3)),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

无论其他 Middleware 是否修改了请求或响应，模型调用限制都会在每次模型调用时进行检查。
