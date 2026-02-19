# Edge（边）

Edge 定义了图中节点之间的执行流。Synaptic 支持两种边：始终路由到同一目标的**固定边**，以及根据当前状态动态路由的**条件边**。

## 固定边

固定边无条件地将执行从一个节点路由到另一个节点：

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, END};
use synaptic_core::Message;

let node_a = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step A"));
    Ok(state)
});

let node_b = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Step B"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("a", node_a)
    .add_node("b", node_b)
    .set_entry_point("a")
    .add_edge("a", "b")     // a always flows to b
    .add_edge("b", END)     // b always flows to END
    .compile()?;
```

使用 `END` 常量表示一个节点终止图的执行。每条执行路径最终都必须到达 `END`；否则图将触发 100 次迭代的安全限制。

## 入口点

每个图都需要一个入口点——第一个执行的节点：

```rust
let graph = StateGraph::new()
    .add_node("start", my_node)
    .set_entry_point("start")  // required
    // ...
```

在未设置入口点的情况下调用 `.compile()` 会返回错误。

## 条件边

条件边根据一个检查当前状态并返回下一个节点名称的函数来路由执行：

```rust
use synaptic_graph::{StateGraph, FnNode, MessageState, END};
use synaptic_core::Message;

let router = FnNode::new(|state: MessageState| async move {
    Ok(state)  // routing logic is in the edge, not the node
});

let handle_greeting = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Hello!"));
    Ok(state)
});

let handle_question = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Let me look that up."));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("router", router)
    .add_node("greeting", handle_greeting)
    .add_node("question", handle_question)
    .set_entry_point("router")
    .add_conditional_edges("router", |state: &MessageState| {
        let last = state.last_message().map(|m| m.content().to_string());
        match last.as_deref() {
            Some("hi") | Some("hello") => "greeting".to_string(),
            _ => "question".to_string(),
        }
    })
    .add_edge("greeting", END)
    .add_edge("question", END)
    .compile()?;
```

路由函数接收状态的不可变引用（`&S`）并返回一个 `String`——要执行的下一个节点的名称（或使用 `END` 终止）。

## 带路径映射的条件边

为了支持图可视化，你可以提供一个 `path_map` 来枚举所有可能的路由目标。这为可视化工具（Mermaid、DOT、ASCII）提供了绘制所有可能路径所需的信息：

```rust
use std::collections::HashMap;
use synaptic_graph::{StateGraph, MessageState, END};

let graph = StateGraph::new()
    .add_node("router", router_node)
    .add_node("path_a", node_a)
    .add_node("path_b", node_b)
    .set_entry_point("router")
    .add_conditional_edges_with_path_map(
        "router",
        |state: &MessageState| {
            if state.messages.len() > 3 {
                "path_a".to_string()
            } else {
                "path_b".to_string()
            }
        },
        HashMap::from([
            ("path_a".to_string(), "path_a".to_string()),
            ("path_b".to_string(), "path_b".to_string()),
        ]),
    )
    .add_edge("path_a", END)
    .add_edge("path_b", END)
    .compile()?;
```

`path_map` 是一个 `HashMap<String, String>`，其中键是标签，值是目标节点名称。编译步骤会验证所有路径映射目标是否引用了已存在的节点（或 `END`）。

## 验证

当你调用 `.compile()` 时，图会进行以下验证：

- 入口点已设置且引用了一个已存在的节点。
- 每条固定边的源和目标都引用了一个已存在的节点（或 `END`）。
- 每条条件边的源都引用了一个已存在的节点。
- 所有 `path_map` 目标都引用了已存在的节点（或 `END`）。

如果任何验证失败，`compile()` 将返回一个 `SynapticError::Graph` 并附带描述性消息。
