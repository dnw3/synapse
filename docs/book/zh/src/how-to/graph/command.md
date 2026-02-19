# Command 与路由

`Command<S>` 赋予节点对图执行的动态控制能力，允许它们覆盖基于边的路由、更新状态、扇出到多个节点或提前终止。当路由决策取决于运行时状态时使用它。

节点返回 `NodeOutput<S>`——要么是 `NodeOutput::State(S)` 表示常规状态更新（通过 `Ok(state.into())`），要么是 `NodeOutput::Command(Command<S>)` 表示动态控制流。

## Command 构造函数

| 构造函数 | 行为 |
|----------|------|
| `Command::goto("node")` | 路由到指定节点，跳过正常边 |
| `Command::goto_with_update("node", delta)` | 路由到指定节点并将 `delta` 合并到状态中 |
| `Command::update(delta)` | 将 `delta` 合并到状态中，然后遵循正常路由 |
| `Command::end()` | 立即终止图 |
| `Command::send(targets)` | 通过 [`Send`] 扇出到多个节点 |
| `Command::resume(value)` | 从先前的 interrupt 恢复（参见 [Interrupt & Resume](interrupt-resume.md)） |

## 使用 `goto` 的条件路由

一个"分诊"节点检查输入并路由到不同的处理器：

```rust,ignore
use synaptic_graph::{Command, FnNode, NodeOutput, State, StateGraph, END};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TicketState {
    category: String,
    resolved: bool,
}

impl State for TicketState {
    fn merge(&mut self, other: Self) {
        if !other.category.is_empty() { self.category = other.category; }
        self.resolved = self.resolved || other.resolved;
    }
}

let triage = FnNode::new(|state: TicketState| async move {
    let target = if state.category == "billing" {
        "billing_handler"
    } else {
        "support_handler"
    };
    Ok(NodeOutput::Command(Command::goto(target)))
});

let billing = FnNode::new(|mut state: TicketState| async move {
    state.resolved = true;
    Ok(state.into())
});

let support = FnNode::new(|mut state: TicketState| async move {
    state.resolved = true;
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("triage", triage)
    .add_node("billing_handler", billing)
    .add_node("support_handler", support)
    .set_entry_point("triage")
    .add_edge("billing_handler", END)
    .add_edge("support_handler", END)
    .compile()?;

let result = graph.invoke(TicketState {
    category: "billing".into(),
    resolved: false,
}).await?.into_state();
assert!(result.resolved);
```

## 带状态更新的路由

`goto_with_update` 在一步中完成路由和状态增量合并。增量通过 `State::merge()` 在目标节点运行之前合并：

```rust,ignore
Ok(NodeOutput::Command(Command::goto_with_update("escalation", delta)))
```

## 不带路由的更新

`Command::update(delta)` 合并状态但遵循正常边。当节点贡献部分更新而不覆盖下一步时很有用：

```rust,ignore
Ok(NodeOutput::Command(Command::update(delta)))
```

## 提前终止

`Command::end()` 立即停止图。不再执行后续节点：

```rust,ignore
let guard = FnNode::new(|state: TicketState| async move {
    if state.category == "spam" {
        return Ok(NodeOutput::Command(Command::end()));
    }
    Ok(state.into())
});
```

## 使用 `Send` 扇出

`Command::send()` 将工作分发到多个目标。每个 `Send` 携带一个节点名称和一个 JSON 负载：

```rust,ignore
use synaptic_graph::Send;

let targets = vec![
    Send::new("worker", serde_json::json!({"chunk": "part1"})),
    Send::new("worker", serde_json::json!({"chunk": "part2"})),
];
Ok(NodeOutput::Command(Command::send(targets)))
```

> **注意：** 完整的并行扇出尚未实现。目标当前按顺序处理。

## 流式模式中的 Command

Command 在流式处理中的行为完全相同。如果节点 "a" 发出 `Command::goto("c")`，流将产出 "a" 和 "c" 的事件，但跳过 "b"，即使存在 `a -> b` 的边。
