# Graph

Synaptic 通过 `synaptic_graph` crate 提供 LangGraph 风格的图编排功能。`StateGraph` 是一个状态机，其中**节点**处理状态，**边**控制节点之间的流转。此架构支持固定路由、条件分支、检查点持久化、人工介入中断和流式执行。

## 核心概念

| 概念 | 说明 |
|---------|-------------|
| `State` trait | 定义节点产生更新时图状态如何合并 |
| `Node<S>` trait | 接收状态并返回更新后状态的处理单元 |
| `StateGraph` | 用于组装节点和边的图构建器 |
| `CompiledGraph` | 由 `StateGraph::compile()` 生成的可执行图 |
| `Checkpointer` | 用于跨调用持久化图状态的 trait |
| `ToolNode` | 预构建节点，自动分发 AI 消息中的工具调用 |

## 工作原理

1. 定义一个实现 `State` 的状态类型（或使用内置的 `MessageState`）。
2. 创建节点 -- 通过实现 `Node<S>` trait 或使用 `FnNode` 包装闭包。
3. 使用 `StateGraph::new()` 构建图，添加节点和边。
4. 调用 `.compile()` 验证图并生成 `CompiledGraph`。
5. 使用 `invoke()` 获取单次结果，或使用 `stream()` 获取逐节点的事件。

```rust
use synaptic_graph::{StateGraph, MessageState, FnNode, END};
use synaptic_core::Message;

let greet = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("Hello from the graph!"));
    Ok(state)
});

let graph = StateGraph::new()
    .add_node("greet", greet)
    .set_entry_point("greet")
    .add_edge("greet", END)
    .compile()?;

let initial = MessageState::with_messages(vec![Message::human("Hi")]);
let result = graph.invoke(initial).await?;
assert_eq!(result.messages.len(), 2);
```

## 指南

- [状态与节点](state-nodes.md) -- 定义状态类型和处理节点
- [边](edges.md) -- 使用固定边和条件边连接节点
- [图流式传输](streaming.md) -- 执行期间消费逐节点事件（单模式和多模式）
- [检查点](checkpointing.md) -- 持久化和恢复图状态
- [人工介入](human-in-the-loop.md) -- 中断执行以进行人工审核
- [工具节点](tool-node.md) -- 自动分发 AI 消息中的工具调用
- [可视化](visualization.md) -- 将图渲染为 Mermaid、ASCII、DOT 或 PNG 格式

## 高级功能

### 节点缓存

使用 `add_node_with_cache()` 基于输入状态缓存节点结果。缓存条目在指定 TTL 后过期：

```rust
use synaptic_graph::{StateGraph, CachePolicy, END};
use std::time::Duration;

let graph = StateGraph::new()
    .add_node_with_cache(
        "expensive",
        expensive_node,
        CachePolicy::new(Duration::from_secs(300)),
    )
    .add_edge("expensive", END)
    .set_entry_point("expensive")
    .compile()?;
```

当在 TTL 内再次遇到相同的输入状态时，将直接返回缓存结果而无需重新执行节点。

### 延迟节点

使用 `add_deferred_node()` 创建等待所有传入路径完成后才执行的节点。这对于 `Send` 并行扇出后的扇入聚合非常有用：

```rust
let graph = StateGraph::new()
    .add_node("branch_a", node_a)
    .add_node("branch_b", node_b)
    .add_deferred_node("aggregate", aggregator_node)
    .add_edge("branch_a", "aggregate")
    .add_edge("branch_b", "aggregate")
    .add_edge("aggregate", END)
    .set_entry_point("branch_a")
    .compile()?;
```

### 结构化输出（response_format）

使用 `create_agent()` 创建 agent 时，在 `AgentOptions` 中设置 `response_format` 可以将最终响应强制转换为特定的 JSON schema：

```rust
use synaptic_graph::{create_agent, AgentOptions};

let graph = create_agent(model, tools, AgentOptions {
    response_format: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" },
            "confidence": { "type": "number" }
        },
        "required": ["answer", "confidence"]
    })),
    ..Default::default()
})?;
```

当 agent 产生最终回答（无工具调用）时，它会使用与 schema 匹配的结构化输出指令重新调用模型。
