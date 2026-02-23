# Interrupt 与 Resume

`interrupt(value)` 暂停图的执行并将控制返回给调用者，附带一个 JSON 值，从而实现人机交互工作流——节点在运行时决定是否暂停。需要 Checkpointer 来持久化状态以便后续恢复。

有关声明式中断（`interrupt_before`/`interrupt_after`），请参阅 [Human-in-the-Loop](human-in-the-loop.md)。

## `interrupt()` 函数

```rust,ignore
use synaptic::graph::{interrupt, Node, NodeOutput, MessageState};
use synaptic::core::SynapticError;
use async_trait::async_trait;

struct ApprovalGate;

#[async_trait]
impl Node<MessageState> for ApprovalGate {
    async fn process(
        &self,
        state: MessageState,
    ) -> Result<NodeOutput<MessageState>, SynapticError> {
        if let Some(msg) = state.last_message() {
            for call in msg.tool_calls() {
                if call.name == "delete_database" {
                    return Ok(interrupt(serde_json::json!({
                        "question": "Approve database deletion?",
                        "tool_call": call.name,
                    })));
                }
            }
        }
        Ok(state.into()) // continue normally
    }
}
```

## 使用 `GraphResult` 检测中断

`graph.invoke()` 返回 `GraphResult<S>`——要么是 `Complete(state)` 要么是 `Interrupted { state, interrupt_value }`：

```rust,ignore
let result = graph.invoke_with_config(state, Some(config.clone())).await?;

if result.is_interrupted() {
    println!("Paused: {}", result.interrupt_value().unwrap());
} else {
    println!("Done: {:?}", result.into_state());
}
```

## 完整的往返示例

```rust,ignore
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use serde_json::json;
use synaptic::graph::{
    interrupt, CheckpointConfig, FnNode, StoreCheckpointer,
    NodeOutput, State, StateGraph, END,
};
use synaptic::store::InMemoryStore;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ReviewState {
    proposal: String,
    approved: bool,
    done: bool,
}

impl State for ReviewState {
    fn merge(&mut self, other: Self) {
        if !other.proposal.is_empty() { self.proposal = other.proposal; }
        self.approved = self.approved || other.approved;
        self.done = self.done || other.done;
    }
}

let propose = FnNode::new(|mut state: ReviewState| async move {
    state.proposal = "Delete all temporary files".into();
    Ok(state.into())
});

let gate = FnNode::new(|state: ReviewState| async move {
    Ok(interrupt(json!({"question": "Approve?", "proposal": state.proposal})))
});

let execute = FnNode::new(|mut state: ReviewState| async move {
    state.done = true;
    Ok(state.into())
});

let saver = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
let graph = StateGraph::new()
    .add_node("propose", propose)
    .add_node("gate", gate)
    .add_node("execute", execute)
    .set_entry_point("propose")
    .add_edge("propose", "gate")
    .add_edge("gate", "execute")
    .add_edge("execute", END)
    .compile()?
    .with_checkpointer(saver);

let config = CheckpointConfig::new("review-thread");

// Step 1: Invoke -- graph pauses at the gate
let result = graph
    .invoke_with_config(ReviewState::default(), Some(config.clone()))
    .await?;
assert!(result.is_interrupted());

// Step 2: Review saved state
let saved = graph.get_state(&config).await?.unwrap();
println!("Proposal: {}", saved.proposal);

// Step 3: Optionally update state before resuming
graph.update_state(&config, ReviewState {
    proposal: String::new(), approved: true, done: false,
}).await?;

// Step 4: Resume execution
let result = graph
    .invoke_with_config(ReviewState::default(), Some(config))
    .await?;
assert!(result.is_complete());
assert!(result.into_state().done);
```

## 注意事项

- **需要 Checkpointer。** 没有它，状态无法在中断和恢复之间保存。`StoreCheckpointer` 配合 `InMemoryStore` 适用于开发；生产环境请使用持久化的 Store 实现。
- **中断时不合并状态。** 当节点返回 `interrupt()` 时，该节点的状态更新不会被应用——只保留之前已执行节点的状态。
- **`Command::resume(value)`** 在恢复时向图传递一个值，可通过 Command 的 `resume_value` 字段获取。
- **状态历史。** 调用 `graph.get_state_history(&config)` 可以查看某个线程的所有检查点。
