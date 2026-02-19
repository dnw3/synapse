# Callbacks

Synaptic 提供了一个事件驱动的 Callback 系统，用于观察 Agent 的执行过程。`CallbackHandler` trait 在关键生命周期节点接收 `RunEvent` 值——当运行开始时、当 LLM 被调用时、当工具被执行时、以及当运行完成或失败时。

## `CallbackHandler` Trait

该 trait 定义在 `synaptic_core` 中：

```rust
#[async_trait]
pub trait CallbackHandler: Send + Sync {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError>;
}
```

一个方法接收所有事件类型。Handler 是 `Send + Sync` 的，因此它们可以在异步任务之间共享。

## `RunEvent` 变体

`RunEvent` 枚举覆盖了完整的 Agent 生命周期：

| 变体 | 字段 | 触发时机 |
|------|------|----------|
| `RunStarted` | `run_id`, `session_id` | Agent 运行开始时 |
| `RunStep` | `run_id`, `step` | Agent 循环的每次迭代时 |
| `LlmCalled` | `run_id`, `message_count` | 当 LLM 被调用并传入消息时 |
| `ToolCalled` | `run_id`, `tool_name` | 当工具被执行时 |
| `RunFinished` | `run_id`, `output` | 当 Agent 产生最终答案时 |
| `RunFailed` | `run_id`, `error` | 当 Agent 运行因错误而失败时 |

`RunEvent` 实现了 `Clone`，因此 Handler 可以存储事件的副本以供后续检查。

## 内置 Handler

Synaptic 附带四个 Callback Handler：

| Handler | 用途 |
|---------|------|
| [RecordingCallback](recording.md) | 将所有事件记录在内存中以供后续检查 |
| [TracingCallback](tracing.md) | 发出结构化的 `tracing` span 和事件 |
| [StdOutCallbackHandler](stdout.md) | 将事件打印到标准输出（可选详细模式） |
| [CompositeCallback](composite.md) | 将事件分发到多个 Handler |

## 实现自定义 Handler

你可以实现 `CallbackHandler` 来添加自己的可观测性：

```rust
use async_trait::async_trait;
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};

struct MetricsCallback;

#[async_trait]
impl CallbackHandler for MetricsCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        match event {
            RunEvent::LlmCalled { message_count, .. } => {
                // Record to your metrics system
                println!("LLM called with {message_count} messages");
            }
            RunEvent::ToolCalled { tool_name, .. } => {
                println!("Tool executed: {tool_name}");
            }
            _ => {}
        }
        Ok(())
    }
}
```

## 指南

- [Recording Callback](recording.md) -- 在内存中捕获事件，用于测试和检查
- [Tracing Callback](tracing.md) -- 与 Rust `tracing` 生态系统集成
- [Composite Callback](composite.md) -- 同时将事件分发到多个 Handler
