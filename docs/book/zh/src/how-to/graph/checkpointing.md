# 检查点

检查点在调用之间持久化图的状态，支持可恢复执行、基于图的多轮对话以及人机交互工作流。`Checkpointer` trait 抽象了存储后端，`MemorySaver` 提供了用于开发和测试的内存实现。

## `Checkpointer` Trait

```rust
#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn put(&self, config: &CheckpointConfig, checkpoint: &Checkpoint) -> Result<(), SynapticError>;
    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError>;
    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError>;
}
```

`Checkpoint` 存储序列化的状态和下一个要执行的节点名称：

```rust
pub struct Checkpoint {
    pub state: serde_json::Value,
    pub next_node: Option<String>,
}
```

## `MemorySaver`

`MemorySaver` 是内置的内存检查点器。它将检查点存储在以线程 ID 为键的 `HashMap` 中：

```rust
use synaptic_graph::MemorySaver;
use std::sync::Arc;

let checkpointer = Arc::new(MemorySaver::new());
```

在生产环境中，你应该使用持久化后端（数据库、Redis、文件系统等）来实现 `Checkpointer`。

## 附加 Checkpointer

编译图之后，使用 `.with_checkpointer()` 附加检查点器：

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, MemorySaver, END};
use synaptic_core::Message;
use std::sync::Arc;

let node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Processed"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("process", node)
    .set_entry_point("process")
    .add_edge("process", END)
    .compile()?
    .with_checkpointer(Arc::new(MemorySaver::new()));
```

## `CheckpointConfig`

`CheckpointConfig` 标识一个用于检查点的线程（对话）：

```rust
use synaptic_graph::CheckpointConfig;

let config = CheckpointConfig::new("thread-1");
```

`thread_id` 字符串用于隔离不同的对话。每个线程维护自己的检查点历史。

## 带检查点的调用

使用 `invoke_with_config()` 在启用检查点的情况下运行图：

```rust
let config = CheckpointConfig::new("thread-1");
let initial = MessageState::with_messages(vec![Message::human("Hello")]);

let result = graph.invoke_with_config(initial, Some(config.clone())).await?;
```

每个节点执行后，当前状态和下一个节点会保存到检查点器。在使用相同 `CheckpointConfig` 的后续调用中，图将从上一个检查点恢复。

## 获取状态

你可以查看为某个线程保存的当前状态：

```rust
// Get the latest state for a thread
if let Some(state) = graph.get_state(&config).await? {
    println!("Messages: {}", state.messages.len());
}

// Get the full checkpoint history (oldest to newest)
let history = graph.get_state_history(&config).await?;
for (state, next_node) in &history {
    println!(
        "State with {} messages, next node: {:?}",
        state.messages.len(),
        next_node
    );
}
```

## 状态序列化

检查点要求你的状态类型实现 `Serialize` 和 `Deserialize`（来自 `serde`）。内置的 `MessageState` 已经有这些派生。对于自定义状态类型，需要添加派生：

```rust
use serde::{Serialize, Deserialize};
use synaptic_graph::State;

#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    data: Vec<String>,
}

impl State for MyState {
    fn merge(&mut self, other: Self) {
        self.data.extend(other.data);
    }
}
```
