# Deferred 节点

`add_deferred_node()` 注册一个延迟节点，该节点旨在等待所有入边被遍历后才执行。在使用 `Command::send()` 进行并行扇出后，可以将 `deferred` 节点用作扇入聚合点，确保多个上游分支全部完成后聚合器才运行。

## 添加 Deferred 节点

在 `StateGraph` 上使用 `add_deferred_node()` 代替 `add_node()`：

```rust,ignore
use synaptic_graph::{FnNode, State, StateGraph, END};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AggState { values: Vec<String> }

impl State for AggState {
    fn merge(&mut self, other: Self) { self.values.extend(other.values); }
}

let worker_a = FnNode::new(|mut state: AggState| async move {
    state.values.push("from_a".into());
    Ok(state.into())
});

let worker_b = FnNode::new(|mut state: AggState| async move {
    state.values.push("from_b".into());
    Ok(state.into())
});

let aggregator = FnNode::new(|state: AggState| async move {
    println!("Collected {} results", state.values.len());
    Ok(state.into())
});

let graph = StateGraph::new()
    .add_node("worker_a", worker_a)
    .add_node("worker_b", worker_b)
    .add_deferred_node("aggregator", aggregator)
    .add_edge("worker_a", "aggregator")
    .add_edge("worker_b", "aggregator")
    .add_edge("aggregator", END)
    .set_entry_point("worker_a")
    .compile()?;
```

## 查询 Deferred 状态

编译完成后，可以使用 `is_deferred()` 检查某个节点是否为 `deferred` 节点：

```rust,ignore
assert!(graph.is_deferred("aggregator"));
assert!(!graph.is_deferred("worker_a"));
```

## 计算入边数量

`incoming_edge_count()` 返回指向某个节点的固定边和条件边的总数。可以用它来验证 `deferred` 节点是否具有预期数量的上游依赖：

```rust,ignore
assert_eq!(graph.incoming_edge_count("aggregator"), 2);
assert_eq!(graph.incoming_edge_count("worker_a"), 0);
```

计数包括固定边（`add_edge`）和条件边路径映射中引用该节点的条目。没有路径映射的条件边不会被计入，因为其目标无法静态确定。

## 与 `Command::send()` 结合使用

`Deferred` 节点被设计为 `Command::send()` 扇出工作后的聚合目标：

```rust,ignore
use synaptic_graph::{Command, NodeOutput, Send};

let dispatcher = FnNode::new(|_state: AggState| async move {
    let targets = vec![
        Send::new("worker", serde_json::json!({"chunk": "A"})),
        Send::new("worker", serde_json::json!({"chunk": "B"})),
    ];
    Ok(NodeOutput::Command(Command::send(targets)))
});

let graph = StateGraph::new()
    .add_node("dispatch", dispatcher)
    .add_node("worker", worker_node)
    .add_deferred_node("collect", collector_node)
    .add_edge("worker", "collect")
    .add_edge("collect", END)
    .set_entry_point("dispatch")
    .compile()?;
```

> **注意：** `Command::send()` 的完整并行扇出尚未实现。目标当前按顺序处理。`deferred` 节点基础设施已就绪，待并行执行功能添加后即可使用。

## 线性图

在线性链中的 `deferred` 节点可以正常编译和执行。`deferred` 标记仅在多条边汇聚到同一节点时才有实际意义：

```rust,ignore
let graph = StateGraph::new()
    .add_node("step1", step1)
    .add_deferred_node("step2", step2)
    .add_edge("step1", "step2")
    .add_edge("step2", END)
    .set_entry_point("step1")
    .compile()?;

let result = graph.invoke(AggState::default()).await?.into_state();
// 在线性链中运行效果与非 deferred 节点完全相同
```

## 注意事项

- **Deferred 是一个标记。** 当前执行引擎不会阻塞等待入边完成——它按边/命令的顺序运行节点。该标记是面向未来并行扇出支持的前瞻性基础设施。
- **`is_deferred()` 和 `incoming_edge_count()` 仅用于内省。** 它们允许你在测试中验证图的拓扑结构，不会影响执行行为。
