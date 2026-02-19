# State 与 Node

Synaptic 中的图在一个 **state** 值上运行，该值在 **node** 之间流转。每个 Node 接收当前 State，处理后返回更新的 State。`State` trait 定义了状态如何合并，`Node<S>` trait 定义了节点如何处理状态。

## `State` Trait

任何用作图状态的类型都必须实现 `State` trait：

```rust
pub trait State: Clone + Send + Sync + 'static {
    /// Merge another state into this one (reducer pattern).
    fn merge(&mut self, other: Self);
}
```

`merge()` 方法在合并状态更新时被调用——例如，在人机交互流程中使用 `update_state()` 时。合并语义由你决定：追加、替换或任何自定义逻辑。

## `MessageState` —— 内置 State

对于常见的对话式 Agent 场景，Synaptic 提供了 `MessageState`：

```rust
use synaptic_graph::MessageState;
use synaptic_core::Message;

// Create an empty state
let state = MessageState::new();

// Create with initial messages
let state = MessageState::with_messages(vec![
    Message::human("Hello"),
    Message::ai("Hi there!"),
]);

// Access the last message
if let Some(msg) = state.last_message() {
    println!("Last: {}", msg.content());
}
```

`MessageState` 通过在合并时追加消息来实现 `State`：

```rust
fn merge(&mut self, other: Self) {
    self.messages.extend(other.messages);
}
```

这种只追加的行为对于对话式工作流来说是正确的默认策略，每个节点都将新消息添加到历史记录中。

## 自定义 State

你可以为非对话式图定义自己的状态类型：

```rust
use synaptic_graph::State;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PipelineState {
    input: String,
    steps_completed: Vec<String>,
    result: Option<String>,
}

impl State for PipelineState {
    fn merge(&mut self, other: Self) {
        self.steps_completed.extend(other.steps_completed);
        if other.result.is_some() {
            self.result = other.result;
        }
    }
}
```

如果你计划使用检查点功能，你的状态还必须实现 `Serialize` 和 `Deserialize`。

## `Node<S>` Trait

Node 是任何实现了 `Node<S>` 的类型：

```rust
use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_graph::{Node, NodeOutput, MessageState};
use synaptic_core::Message;

struct GreeterNode;

#[async_trait]
impl Node<MessageState> for GreeterNode {
    async fn process(&self, mut state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
        state.messages.push(Message::ai("Hello! How can I help?"));
        Ok(state.into()) // NodeOutput::State(state)
    }
}
```

Node 返回 `NodeOutput<S>`，它是一个枚举：

- **`NodeOutput::State(S)`** —— 常规的状态更新（现有行为）。`From<S>` 实现让你可以写 `Ok(state.into())`。
- **`NodeOutput::Command(Command<S>)`** —— 控制流命令（goto、interrupt、扇出）。有关 interrupt 示例，请参阅 [Human-in-the-Loop](human-in-the-loop.md)。

Node 是 `Send + Sync` 的，因此可以安全地持有共享引用（例如 `Arc<dyn ChatModel>`）并在异步任务间使用。

## `FnNode` —— 基于闭包的 Node

对于简单逻辑，`FnNode` 将异步闭包包装为节点，无需定义单独的结构体：

```rust
use synaptic_graph::{FnNode, MessageState};
use synaptic_core::Message;

let greeter = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Hello from a closure!"));
    Ok(state.into())
});
```

`FnNode` 接受签名为 `Fn(S) -> Future<Output = Result<NodeOutput<S>, SynapticError>>` 的任何函数，其中 `S: State`。

## 将 Node 添加到图中

Node 通过字符串名称添加到 `StateGraph` 中。名称用于在边和条件路由中引用该节点：

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, END};
use synaptic_core::Message;

let node_a = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step A"));
    Ok(state.into())
});

let node_b = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step B"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("a", node_a)
    .add_node("b", node_b)
    .set_entry_point("a")
    .add_edge("a", "b")
    .add_edge("b", END)
    .compile()?;
```

基于结构体的节点（实现 `Node<S>`）和 `FnNode` 闭包都可以互换地传递给 `add_node()`。
