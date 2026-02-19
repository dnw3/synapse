# Recording Callback

`RecordingCallback` 在内存列表中捕获每个 `RunEvent`。这对于测试 Agent 行为、调试执行流程以及构建审计日志非常有用。

## 用法

```rust
use synaptic_callbacks::RecordingCallback;
use synaptic_core::RunEvent;

let callback = RecordingCallback::new();

// ... pass the callback to an agent or use it manually ...

// After the run, inspect all recorded events
let events = callback.events().await;
for event in &events {
    match event {
        RunEvent::RunStarted { run_id, session_id } => {
            println!("Run started: run_id={run_id}, session={session_id}");
        }
        RunEvent::RunStep { run_id, step } => {
            println!("Step {step} in run {run_id}");
        }
        RunEvent::LlmCalled { run_id, message_count } => {
            println!("LLM called with {message_count} messages (run {run_id})");
        }
        RunEvent::ToolCalled { run_id, tool_name } => {
            println!("Tool '{tool_name}' called (run {run_id})");
        }
        RunEvent::RunFinished { run_id, output } => {
            println!("Run {run_id} finished: {output}");
        }
        RunEvent::RunFailed { run_id, error } => {
            println!("Run {run_id} failed: {error}");
        }
    }
}
```

## 工作原理

`RecordingCallback` 将事件存储在 `Arc<RwLock<Vec<RunEvent>>>` 中。每次调用 `on_event()` 时，事件会被追加到列表中。`events()` 方法返回完整事件列表的克隆。

由于使用了 `Arc`，Callback 可以被克隆并在多个任务之间共享。所有克隆都引用相同的事件存储。

## 测试示例

`RecordingCallback` 在测试中特别有用，可以验证 Agent 是否遵循了预期的执行路径：

```rust
#[tokio::test]
async fn test_agent_calls_tool() {
    let callback = RecordingCallback::new();

    // ... run the agent with this callback ...

    let events = callback.events().await;

    // Verify the agent called the expected tool
    let tool_events: Vec<_> = events.iter()
        .filter_map(|e| match e {
            RunEvent::ToolCalled { tool_name, .. } => Some(tool_name.clone()),
            _ => None,
        })
        .collect();

    assert!(tool_events.contains(&"calculator".to_string()));
}
```

## 线程安全性

`RecordingCallback` 是 `Clone`、`Send` 和 `Sync` 的。你可以安全地在异步任务之间共享它，并从持有引用的任何任务中检查事件。
