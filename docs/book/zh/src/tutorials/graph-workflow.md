# 构建 Graph 工作流

本教程将引导你构建一个自定义的 Graph 工作流。Graph 让你可以定义多步骤的状态机，支持条件路由、循环、检查点和流式执行，适用于复杂的 Agent 编排场景。

## 你将学到什么

- 定义自定义状态（State）
- 使用 `StateGraph` 构建器添加节点（Node）和边（Edge）
- 添加条件边（Conditional Edge）实现动态路由
- 使用 `CompiledGraph` 执行和流式观察工作流
- 使用可视化方法输出图结构

## 核心术语

在开始之前，了解几个关键概念：

| 术语 | 说明 |
|---|---|
| 节点 (Node) | 图中的处理单元，接收状态并返回更新后的状态 |
| 边 (Edge) | 节点之间的固定转换关系 |
| 条件边 (Conditional Edge) | 根据当前状态动态决定下一个节点 |
| 状态 (State) | 在图中流转的数据，节点通过修改状态来传递信息 |
| 编译 (Compile) | 将 StateGraph 转换为可执行的 CompiledGraph |
| 流式执行 (Stream) | 逐节点观察执行过程 |
| 可视化 (Visualization) | 将图结构输出为 Mermaid、ASCII 或 DOT 格式 |

## 场景：多步骤处理工作流

我们将构建一个请求处理工作流：接收请求 -> 分类 -> 根据类别路由到不同的处理节点 -> 完成。

```text
+----------+          +---------+
| classify | -------> | process | -------> END
+----------+          +---------+
      |
      | (urgent)
      v
+----------+
| escalate | -------> END
+----------+
```

## 第一步：定义状态

首先定义工作流中流转的状态类型。它需要实现 `State` trait（以及 `Clone + Send + Sync`）。如果需要使用 Checkpointer，还需要实现 `Serialize + Deserialize`：

```rust
use serde::{Serialize, Deserialize};
use synaptic::core::Message;
use synaptic::graph::State;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowState {
    messages: Vec<Message>,
    category: String,
    result: String,
}

impl State for WorkflowState {
    fn merge(&mut self, other: Self) {
        self.messages.extend(other.messages);
        if !other.category.is_empty() {
            self.category = other.category;
        }
        if !other.result.is_empty() {
            self.result = other.result;
        }
    }
}
```

`merge()` 方法定义了状态更新的合并规则。这里我们选择追加消息列表，以及用非空值覆盖其他字段。

## 第二步：创建节点

使用 `FnNode` 将异步闭包包装为节点。每个节点接收当前状态，执行处理逻辑，返回更新后的状态：

```rust
use synaptic::graph::FnNode;

// 分类节点：根据消息内容确定请求类别
let classify_node = FnNode::new(|mut state: WorkflowState| async move {
    let last_content = state.messages.last()
        .map(|m| m.content().to_string())
        .unwrap_or_default();

    state.category = if last_content.contains("紧急") || last_content.contains("urgent") {
        "urgent".to_string()
    } else {
        "normal".to_string()
    };
    Ok(state)
});

// 常规处理节点
let process_node = FnNode::new(|mut state: WorkflowState| async move {
    state.result = format!("已处理常规请求");
    state.messages.push(Message::ai(&format!(
        "您的请求已处理完成。类别: {}",
        state.category
    )));
    Ok(state)
});

// 加急处理节点
let escalate_node = FnNode::new(|mut state: WorkflowState| async move {
    state.result = format!("已加急处理");
    state.messages.push(Message::ai(&format!(
        "您的紧急请求已优先处理。类别: {}",
        state.category
    )));
    Ok(state)
});
```

## 第三步：构建图

使用 `StateGraph` 构建器将节点和边组合成一个完整的图。使用链式 API 可以清晰地表达图的结构：

```rust
use std::collections::HashMap;
use synaptic::graph::{StateGraph, END};

let graph = StateGraph::<WorkflowState>::new()
    // 添加节点
    .add_node("classify", classify_node)
    .add_node("process", process_node)
    .add_node("escalate", escalate_node)
    // 设置入口点
    .set_entry_point("classify")
    // 添加条件边：根据分类结果路由
    .add_conditional_edges_with_path_map(
        "classify",
        |state: &WorkflowState| {
            if state.category == "urgent" {
                "escalate".to_string()
            } else {
                "process".to_string()
            }
        },
        HashMap::from([
            ("urgent".to_string(), "escalate".to_string()),
            ("normal".to_string(), "process".to_string()),
        ]),
    )
    // 处理完成后结束
    .add_edge("process", END)
    .add_edge("escalate", END);

// 编译为可执行图
let compiled = graph.compile()?;
```

说明：

- **`set_entry_point("classify")`** -- 指定 `classify` 为图的起始节点
- **`add_conditional_edges_with_path_map`** -- 添加条件边，`router` 函数根据状态返回目标节点名称。`path_map` 参数是可选的，用于可视化时显示所有可能的路由目标
- **`add_edge("process", END)`** -- 添加从 `process` 到 `END` 的固定边，表示处理完成后结束
- **`END`** -- 特殊常量 `"__end__"`，表示图的终止点

## 第四步：执行工作流

### 单次执行

使用 `invoke()` 执行图直到到达 `END`：

```rust
let initial_state = WorkflowState {
    messages: vec![Message::human("请处理这个常规请求")],
    category: String::new(),
    result: String::new(),
};

let final_state = compiled.invoke(initial_state).await?;
println!("类别: {}", final_state.category);    // "normal"
println!("结果: {}", final_state.result);       // "已处理常规请求"
println!("消息数: {}", final_state.messages.len()); // 2 (human + ai)
```

尝试紧急请求：

```rust
let urgent_state = WorkflowState {
    messages: vec![Message::human("这是一个紧急请求，需要立即处理")],
    category: String::new(),
    result: String::new(),
};

let final_state = compiled.invoke(urgent_state).await?;
println!("类别: {}", final_state.category);  // "urgent"
println!("结果: {}", final_state.result);     // "已加急处理"
```

### 流式执行

使用 `stream()` 观察每个节点的执行过程。每个节点执行完成后会产出一个 `GraphEvent`：

```rust
use synaptic::graph::StreamMode;
use futures::StreamExt;

let initial_state = WorkflowState {
    messages: vec![Message::human("请处理这个常规请求")],
    category: String::new(),
    result: String::new(),
};

let mut stream = std::pin::pin!(compiled.stream(initial_state, StreamMode::Values));

while let Some(event) = stream.next().await {
    let event = event?;
    println!(
        "[节点: {}] 类别={}, 结果={}",
        event.node,
        event.state.category,
        event.state.result
    );
}
```

输出：

```text
[节点: classify] 类别=normal, 结果=
[节点: process] 类别=normal, 结果=已处理常规请求
```

`StreamMode` 有两种模式：

- **`StreamMode::Values`** -- 每个节点执行后产出完整的状态快照
- **`StreamMode::Updates`** -- 每个节点执行后产出该节点处理后的状态

## 第五步：可视化

`CompiledGraph` 提供多种可视化方法，帮助你理解和调试图的结构。

### Mermaid 格式

```rust
let mermaid = compiled.draw_mermaid();
println!("{}", mermaid);
```

输出：

```text
graph TD
    __start__(["__start__"])
    classify["classify"]
    escalate["escalate"]
    process["process"]
    __end__(["__end__"])
    __start__ --> classify
    escalate --> __end__
    process --> __end__
    classify -.-> |normal| process
    classify -.-> |urgent| escalate
```

你可以将这段 Mermaid 文本粘贴到支持 Mermaid 的工具中查看图形化渲染。

### ASCII 文本格式

```rust
let ascii = compiled.draw_ascii();
println!("{}", ascii);
// 或者直接使用 Display trait
println!("{}", compiled);
```

### Graphviz DOT 格式

```rust
let dot = compiled.draw_dot();
println!("{}", dot);
```

### 导出为图片文件

```rust
// 通过 mermaid.ink API 导出（需要网络连接）
compiled.draw_mermaid_png("workflow.png").await?;
compiled.draw_mermaid_svg("workflow.svg").await?;

// 通过本地 Graphviz 导出（需要安装 dot 命令）
compiled.draw_png("workflow.png")?;
```

## 进阶：添加检查点

对于长时间运行或需要人机交互的工作流，使用 `Checkpointer` 保存中间状态。这样即使工作流中断，也可以从最后的检查点恢复：

```rust
use std::sync::Arc;
use synaptic::graph::{StoreCheckpointer, CheckpointConfig};
use synaptic::store::InMemoryStore;

// 创建基于 Store 的检查点器
let checkpointer = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
let compiled = graph.compile()?.with_checkpointer(checkpointer);

// 使用 thread_id 标识执行线程
let config = CheckpointConfig {
    thread_id: "workflow-001".to_string(),
};

// 执行（会自动保存检查点）
let final_state = compiled.invoke_with_config(initial_state, Some(config.clone())).await?;

// 查询状态历史
let history = compiled.get_state_history(&config).await?;
for (state, next_node) in &history {
    println!("状态 -> 下一个节点: {:?}", next_node);
}
```

## 进阶：Human-in-the-Loop

在关键操作前暂停执行，等待人工确认：

```rust
let graph = StateGraph::<WorkflowState>::new()
    .add_node("classify", classify_node)
    .add_node("escalate", escalate_node)
    .add_node("process", process_node)
    .set_entry_point("classify")
    .add_conditional_edges("classify", route_fn)
    .add_edge("process", END)
    .add_edge("escalate", END)
    // 在执行加急处理前暂停
    .interrupt_before(vec!["escalate".to_string()]);

let compiled = graph.compile()?.with_checkpointer(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));

let config = CheckpointConfig {
    thread_id: "t1".to_string(),
};

// 对于紧急请求，图会在 escalate 节点前中断
let result = compiled.invoke_with_config(urgent_state, Some(config.clone())).await;
// result 是 Err，表示图已中断

// 人工审查后，恢复执行
let final_state = compiled.invoke_with_config(
    WorkflowState { messages: vec![], category: String::new(), result: String::new() },
    Some(config),
).await?;
```

## 总结

在本教程中你学会了：

- 定义自定义 `State` 类型并实现 `merge()` 方法
- 使用 `FnNode` 创建节点，使用 `StateGraph` 构建器组装图
- 通过条件边实现基于状态的动态路由
- 使用 `invoke()` 执行图，使用 `stream()` 观察执行过程
- 使用 `draw_mermaid()`、`draw_ascii()`、`draw_dot()` 可视化图结构
- 使用 `Checkpointer` 和 `interrupt_before` 支持检查点和人机交互

## 下一步

- [Graph 概念](../concepts/graph.md) -- 深入了解 State、Node、Edge 和 Checkpointer
- [构建 ReAct Agent](react-agent.md) -- 使用 `create_react_agent` 预构建的 ReAct 模式
- [Runnables 与 LCEL](../concepts/runnables-lcel.md) -- 了解另一种组合方式：管道链
