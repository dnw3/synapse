# Human-in-the-Loop

Human-in-the-loop（HITL）允许你在特定位置暂停图的执行，让人类有机会在图继续之前审查、批准或修改状态。Synaptic 支持两种方式：

1. **`interrupt_before` / `interrupt_after`** —— 在 `StateGraph` 构建器上的声明式中断。
2. **`interrupt()` 函数** —— 在节点内部通过 `Command` 实现的编程式中断。

两种方式都需要 Checkpointer 来持久化状态以便后续恢复。

## Interrupt Before 和 After

`StateGraph` 构建器提供两种中断模式：

- **`interrupt_before(nodes)`** —— 在指定节点运行**之前**暂停执行。
- **`interrupt_after(nodes)`** —— 在指定节点运行**之后**暂停执行。

### 示例：工具执行前的审批

一个常见模式是在工具执行节点之前中断，以便人类可以审查 Agent 提出的工具调用：

```rust
use synaptic::graph::{StateGraph, FnNode, MessageState, StoreCheckpointer, CheckpointConfig, END};
use synaptic::store::InMemoryStore;
use synaptic::core::Message;
use std::sync::Arc;

let agent_node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("I want to call the delete_file tool."));
    Ok(state.into())
});

let tool_node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::tool("File deleted.", "call-1"));
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_edge("agent", "tools")
    .add_edge("tools", END)
    // Pause before the tools node executes
    .interrupt_before(vec!["tools".to_string()])
    .compile()?
    .with_checkpointer(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));

let config = CheckpointConfig::new("thread-1");
let initial = MessageState::with_messages(vec![Message::human("Delete old logs")]);
```

### 第一步：首次调用——中断

第一次 `invoke_with_config()` 运行 `agent` 节点，然后在 `tools` 之前停止：

```rust
let result = graph.invoke_with_config(initial, Some(config.clone())).await?;

// Returns GraphResult::Interrupted
assert!(result.is_interrupted());

// You can inspect the interrupt value
if let Some(iv) = result.interrupt_value() {
    println!("Interrupted: {iv}");
}
```

此时，Checkpointer 已保存了 `agent` 运行后的状态，`tools` 作为下一个节点。

### 第二步：人工审查

人类可以检查保存的状态，审查 Agent 提出的内容：

```rust
if let Some(state) = graph.get_state(&config).await? {
    for msg in &state.messages {
        println!("[{}] {}", msg.role(), msg.content());
    }
}
```

### 第三步：更新状态（可选）

如果人类想在恢复之前修改状态——例如添加批准消息或更改工具调用——使用 `update_state()`：

```rust
let approval = MessageState::with_messages(vec![
    Message::human("Approved -- go ahead and delete."),
]);

graph.update_state(&config, approval).await?;
```

`update_state()` 加载当前检查点，使用提供的更新调用 `State::merge()`，然后将合并结果保存回 Checkpointer。

### 第四步：恢复执行

使用相同的配置和默认（空）状态再次调用 `invoke_with_config()` 来恢复图的执行。图会加载检查点并从中断的节点继续：

```rust
let result = graph
    .invoke_with_config(MessageState::default(), Some(config))
    .await?;

// The graph executed "tools" and reached END
let state = result.into_state();
println!("Final messages: {}", state.messages.len());
```

## 使用 `interrupt()` 的编程式中断

为了获得更多控制，节点可以调用 `interrupt()` 函数以自定义值暂停执行。当中断决策取决于运行时状态时，这很有用：

```rust
use synaptic::graph::{interrupt, Node, NodeOutput, MessageState};

struct ApprovalNode;

#[async_trait]
impl Node<MessageState> for ApprovalNode {
    async fn process(&self, state: MessageState) -> Result<NodeOutput<MessageState>, SynapticError> {
        // Check if any tool call is potentially dangerous
        if let Some(msg) = state.last_message() {
            for call in msg.tool_calls() {
                if call.name == "delete_file" {
                    // Interrupt and ask for approval
                    return Ok(interrupt(serde_json::json!({
                        "question": "Approve file deletion?",
                        "tool_call": call.name,
                    })));
                }
            }
        }
        // No dangerous calls -- continue normally
        Ok(state.into())
    }
}
```

调用者会收到一个带有中断值的 `GraphResult::Interrupted`：

```rust
let result = graph.invoke_with_config(state, Some(config.clone())).await?;
if result.is_interrupted() {
    let question = result.interrupt_value().unwrap();
    println!("Agent asks: {}", question["question"]);
}
```

## 使用 `Command` 进行动态路由

节点也可以使用 `Command` 来覆盖正常的基于边的路由：

```rust
use synaptic::graph::{Command, NodeOutput};

// Route to a specific node, skipping normal edges
Ok(NodeOutput::Command(Command::goto("summary")))

// Route to a specific node with a state update
Ok(NodeOutput::Command(Command::goto_with_update("next", delta_state)))

// End the graph immediately
Ok(NodeOutput::Command(Command::end()))

// Update state without overriding routing
Ok(NodeOutput::Command(Command::update(delta_state)))
```

## `interrupt_after`

`interrupt_after` 的工作方式相同，但指定的节点会在中断**之前**运行。当你想在决定是否继续之前查看节点的输出时，这很有用：

```rust
let graph = StateGraph::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_edge("agent", "tools")
    .add_edge("tools", END)
    // Interrupt after the agent node runs (to review its output)
    .interrupt_after(vec!["agent".to_string()])
    .compile()?
    .with_checkpointer(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));
```

## `GraphResult`

`graph.invoke()` 返回 `Result<GraphResult<S>, SynapticError>`。`GraphResult` 是一个枚举：

- **`GraphResult::Complete(state)`** —— 图正常运行到 `END`。
- **`GraphResult::Interrupted { state, interrupt_value }`** —— 图已暂停。

关键方法：

| 方法 | 描述 |
|------|------|
| `is_complete()` | 如果图正常完成则返回 `true` |
| `is_interrupted()` | 如果图被中断则返回 `true` |
| `state()` | 借用状态（无论完成/中断） |
| `into_state()` | 消费并返回状态 |
| `interrupt_value()` | 如果被中断则返回 `Some(&Value)`，否则返回 `None` |

## 注意事项

- 中断需要 Checkpointer。没有它，图无法保存状态以便恢复。
- `interrupt_before` / `interrupt_after` 返回 `GraphResult::Interrupted`（不是错误）。
- 编程式 `interrupt()` 也返回 `GraphResult::Interrupted`，附带你传递的值。
- 你可以通过向 `interrupt_before()` 或 `interrupt_after()` 传递多个名称来在多个节点处中断。
- 你可以在同一个图中对不同节点组合使用 `interrupt_before` 和 `interrupt_after`。
