# 图流式处理

你可以**流式**执行图，在每个节点完成后接收一个 `GraphEvent`，而不是等待整个图执行完毕。这对于进度报告、实时 UI 和调试非常有用。

## `stream()` 与 `StreamMode`

`CompiledGraph` 上的 `stream()` 方法返回一个 `GraphStream`——一个 `Pin<Box<dyn Stream>>`，它产出 `Result<GraphEvent<S>, SynapticError>` 值：

```rust
use synaptic::graph::{StateGraph, FnNode, MessageState, StreamMode, GraphEvent, END};
use synaptic::core::Message;
use futures::StreamExt;

let step_a = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step A done"));
    Ok(state)
});

let step_b = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step B done"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("a", step_a)
    .add_node("b", step_b)
    .set_entry_point("a")
    .add_edge("a", "b")
    .add_edge("b", END)
    .compile()?;

let initial = MessageState::with_messages(vec![Message::human("Start")]);

let mut stream = graph.stream(initial, StreamMode::Values);
while let Some(event) = stream.next().await {
    let event: GraphEvent<MessageState> = event?;
    println!(
        "Node '{}' completed -- {} messages in state",
        event.node,
        event.state.messages.len()
    );
}
// Output:
//   Node 'a' completed -- 2 messages in state
//   Node 'b' completed -- 3 messages in state
```

## `GraphEvent`

每个事件包含：

| 字段 | 类型 | 描述 |
|------|------|------|
| `node` | `String` | 刚刚执行完的节点名称 |
| `state` | `S` | 节点运行后的状态快照 |

## StreamMode

`StreamMode` 枚举控制 `state` 字段包含的内容：

| 模式 | 行为 |
|------|------|
| `StreamMode::Values` | 每个事件包含节点执行后的**完整累积状态** |
| `StreamMode::Updates` | 每个事件包含**节点执行前的状态**（用于计算每个节点的增量） |
| `StreamMode::Messages` | 与 Values 相同——调用者在聊天 UI 中过滤 AI 消息 |
| `StreamMode::Debug` | 与 Values 相同——用于详细的调试信息 |
| `StreamMode::Custom` | 通过 StreamWriter 在节点执行期间发出的事件 |

## 多模式流式处理

你可以使用 `stream_modes()` 同时请求多种流模式。每个事件被包装在一个带有模式标签的 `MultiGraphEvent` 中：

```rust
use synaptic::graph::{StreamMode, MultiGraphEvent};
use futures::StreamExt;

let mut stream = graph.stream_modes(
    initial_state,
    vec![StreamMode::Values, StreamMode::Updates],
);

while let Some(result) = stream.next().await {
    let event: MultiGraphEvent<MessageState> = result?;
    match event.mode {
        StreamMode::Values => {
            println!("Full state after '{}': {:?}", event.event.node, event.event.state);
        }
        StreamMode::Updates => {
            println!("State before '{}': {:?}", event.event.node, event.event.state);
        }
        _ => {}
    }
}
```

对于每次节点执行，每个请求的模式都会发出一个事件。使用两种模式和三个节点，你总共会得到六个事件。

## 带检查点的流式处理

你可以使用 `stream_with_config()` 将流式处理与检查点结合：

```rust
use synaptic::graph::{StoreCheckpointer, CheckpointConfig, StreamMode};
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let checkpointer = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
let graph = graph.with_checkpointer(checkpointer);

let config = CheckpointConfig::new("thread-1");

let mut stream = graph.stream_with_config(
    initial_state,
    StreamMode::Values,
    Some(config),
);

while let Some(event) = stream.next().await {
    let event = event?;
    println!("Node: {}", event.node);
}
```

与 `invoke()` 期间一样，流式处理时每个节点执行后都会保存检查点。如果图被中断（通过 `interrupt_before` 或 `interrupt_after`），流将产出中断错误并终止。

## 错误处理

流产出的是 `Result` 值。如果节点返回错误，流将产出该错误并终止。消费端代码应同时处理成功事件和错误：

```rust
while let Some(result) = stream.next().await {
    match result {
        Ok(event) => println!("Node '{}' succeeded", event.node),
        Err(e) => {
            eprintln!("Graph error: {e}");
            break;
        }
    }
}
```
