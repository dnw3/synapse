# Composite Callback

`CompositeCallback` 将每个 `RunEvent` 分发到多个 Callback Handler。这让你可以组合不同的可观测策略而不必只选择一种——例如，在内存中记录事件用于测试的同时，也通过 `tracing` 记录日志。

## 用法

```rust
use synaptic_callbacks::{CompositeCallback, RecordingCallback, TracingCallback};
use std::sync::Arc;

let recording = Arc::new(RecordingCallback::new());
let tracing_cb = Arc::new(TracingCallback::new());

let composite = CompositeCallback::new(vec![
    recording.clone(),
    tracing_cb,
]);
```

当调用 `composite.on_event(event)` 时，事件会按顺序转发给每个 Handler。如果任何 Handler 返回错误，Composite 会停止并传播该错误。

## 工作原理

`CompositeCallback` 持有一个 `Vec<Arc<dyn CallbackHandler>>`。对于每个事件：

1. 为每个 Handler 克隆事件（因为 `RunEvent` 实现了 `Clone`）。
2. 按顺序等待每个 Handler 的 `on_event()`。
3. 如果所有 Handler 都成功，返回 `Ok(())`。

```rust
// Pseudocode of the dispatch logic
async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
    for handler in &self.handlers {
        handler.on_event(event.clone()).await?;
    }
    Ok(())
}
```

## 示例：Recording + Tracing + 自定义

你可以混合使用内置和自定义 Handler：

```rust
use async_trait::async_trait;
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};
use synaptic_callbacks::{
    CompositeCallback, RecordingCallback, TracingCallback, StdOutCallbackHandler,
};
use std::sync::Arc;

struct ToolCounter {
    count: Arc<tokio::sync::RwLock<usize>>,
}

#[async_trait]
impl CallbackHandler for ToolCounter {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        if matches!(event, RunEvent::ToolCalled { .. }) {
            *self.count.write().await += 1;
        }
        Ok(())
    }
}

let counter = Arc::new(ToolCounter {
    count: Arc::new(tokio::sync::RwLock::new(0)),
});

let composite = CompositeCallback::new(vec![
    Arc::new(RecordingCallback::new()),
    Arc::new(TracingCallback::new()),
    Arc::new(StdOutCallbackHandler::new()),
    counter.clone(),
]);
```

## 何时使用

当你需要同时激活多个 Callback Handler 时，使用 `CompositeCallback`。常见组合：

- **开发环境**：`StdOutCallbackHandler` + `RecordingCallback` -- 在终端查看事件的同时可以程序化地检查它们。
- **测试环境**：单独使用 `RecordingCallback` 通常就足够了。
- **生产环境**：`TracingCallback` + 自定义指标 Handler -- 结构化日志加上应用程序特定的遥测数据。
