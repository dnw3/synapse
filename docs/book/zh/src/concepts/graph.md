# Graph

Synaptic 的 Graph 系统实现了 LangGraph 风格的状态机编排。它让你可以定义复杂的多步骤工作流，支持条件路由、循环、检查点、人机交互和可视化。

## 核心概念

### State trait

`State` 是 Graph 中流转的数据。它定义了状态如何合并（merge）和归约（reduce）：

```rust
pub trait State: Clone + Send + Sync + 'static {
    /// 将另一个状态合并到自身（归约模式）
    fn merge(&mut self, other: Self);
}
```

`merge()` 方法定义了当节点返回新状态时，如何将更新合并到现有状态中。这是 Graph 系统中最核心的设计——每个节点不需要知道完整的状态结构，只需要返回它修改的部分。

### MessageState

`MessageState` 是最常用的内置状态类型，包含一个消息列表，适用于聊天 Agent 场景：

```rust
use synaptic::graph::MessageState;

let state = MessageState::new();
// 或者带初始消息
let state = MessageState::with_messages(vec![
    Message::human("你好！"),
]);

// 访问最后一条消息
if let Some(last) = state.last_message() {
    println!("{}", last.content());
}
```

`MessageState` 的 `merge()` 实现是追加消息——新状态中的消息会被添加到现有消息列表的末尾。

### 自定义 State

对于更复杂的工作流，你可以定义自定义状态类型：

```rust
use serde::{Serialize, Deserialize};
use synaptic::graph::State;

#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    messages: Vec<Message>,
    step_count: usize,
    context: String,
}

impl State for MyState {
    fn merge(&mut self, other: Self) {
        self.messages.extend(other.messages);
        self.step_count = other.step_count;
        self.context = other.context;
    }
}
```

自定义状态需要实现 `Clone + Send + Sync + 'static`。如果需要使用 Checkpointer 持久化，还需要实现 `Serialize + Deserialize`。

### Node

`Node<S>` 是 Graph 中的处理单元。每个节点接收当前状态，执行某些操作，然后返回更新后的状态：

```rust
#[async_trait]
pub trait Node<S: State>: Send + Sync {
    async fn process(&self, state: S) -> Result<S, SynapticError>;
}
```

使用 `FnNode` 可以将异步闭包包装为节点，无需手动实现 trait：

```rust
use synaptic::graph::FnNode;

let greet_node = FnNode::new(|mut state: MessageState| async move {
    state.messages.push(Message::ai("你好！有什么可以帮你的？"));
    Ok(state)
});
```

内置的特殊节点：

- **`ToolNode`** -- 自动执行 AI 消息中的工具调用，将结果作为 `Tool` 消息添加到状态中

### Edge

Edge 定义节点之间的转换关系。有两种类型：

1. **固定边（Fixed Edge）** -- 无条件地从一个节点转到另一个节点
2. **条件边（Conditional Edge）** -- 根据当前状态动态决定下一个节点

## StateGraph 构建器

使用 `StateGraph` 构建器定义工作流的结构：

```rust
use synaptic::graph::{StateGraph, MessageState, FnNode, END};

let graph = StateGraph::<MessageState>::new()
    // 添加节点
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    // 设置入口点
    .set_entry_point("agent")
    // 添加条件边：根据 agent 的输出决定下一步
    .add_conditional_edges("agent", |state: &MessageState| {
        let last = state.last_message().unwrap();
        if last.tool_calls().is_empty() {
            END.to_string()  // 无工具调用，结束
        } else {
            "tools".to_string()  // 有工具调用，执行工具
        }
    })
    // 工具执行后返回 agent 重新推理
    .add_edge("tools", "agent");

// 编译为可执行的 Graph
let compiled = graph.compile()?;
```

`StateGraph` 提供的构建方法：

- **`add_node(name, node)`** -- 添加一个命名节点
- **`add_edge(source, target)`** -- 添加固定边
- **`add_conditional_edges(source, router_fn)`** -- 添加条件边，`router_fn` 接收状态引用并返回目标节点名称
- **`add_conditional_edges_with_path_map(source, router_fn, path_map)`** -- 添加条件边并提供路径映射，用于可视化时显示可能的路由目标
- **`set_entry_point(name)`** -- 设置 Graph 的起始节点
- **`interrupt_before(nodes)`** -- 标记在执行前中断的节点（人机交互）
- **`interrupt_after(nodes)`** -- 标记在执行后中断的节点（人机交互）
- **`compile()`** -- 编译为可执行的 `CompiledGraph`

特殊常量：
- **`START`** (`"__start__"`) -- 表示图的起始点
- **`END`** (`"__end__"`) -- 表示图的结束点。条件边路由到 `END` 时图停止执行

## CompiledGraph

编译后的 `CompiledGraph<S>` 是可执行的图。它提供以下方法：

### invoke -- 执行到完成

```rust
let initial_state = MessageState::with_messages(vec![
    Message::human("2 + 2 等于多少？"),
]);

let final_state = compiled.invoke(initial_state).await?;
println!("{}", final_state.last_message().unwrap().content());
```

### invoke_with_config -- 带检查点配置执行

```rust
use synaptic::graph::CheckpointConfig;

let config = CheckpointConfig {
    thread_id: "thread-1".to_string(),
};
let final_state = compiled.invoke_with_config(initial_state, Some(config)).await?;
```

### stream -- 流式执行

```rust
use synaptic::graph::StreamMode;
use futures::StreamExt;

let stream = compiled.stream(initial_state, StreamMode::Values);

while let Some(event) = stream.next().await {
    let event = event?;
    println!("[节点: {}] 状态: {:?}", event.node, event.state);
}
```

`StreamMode` 有两种模式：

- **`Values`** -- 每个节点执行后产出完整的当前状态快照
- **`Updates`** -- 每个节点执行后产出该节点处理后的状态

每次产出的 `GraphEvent<S>` 包含：
- `node: String` -- 刚执行完的节点名称
- `state: S` -- 状态快照

### update_state -- 更新中断的状态

用于人机交互场景，在图中断后更新状态：

```rust
compiled.update_state(&config, state_update).await?;
```

### get_state / get_state_history -- 查询状态

```rust
// 获取当前状态
let current = compiled.get_state(&config).await?;

// 获取完整历史
let history = compiled.get_state_history(&config).await?;
for (state, next_node) in &history {
    println!("下一个节点: {:?}", next_node);
}
```

## Checkpointer

`Checkpointer` trait 支持状态持久化，使 Graph 可以中断和恢复执行：

```rust
use std::sync::Arc;
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;

let checkpointer = Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())));
let compiled = graph.compile()?.with_checkpointer(checkpointer);
```

`StoreCheckpointer` 是基于 `Store` 的 `Checkpointer` 实现，使用 `InMemoryStore` 适用于开发和测试。每个检查点（`Checkpoint`）包含：
- 序列化的状态数据
- 下一个待执行节点的名称

检查点通过 `CheckpointConfig`（包含 `thread_id`）进行索引，不同的 `thread_id` 对应独立的执行线程。

## Human-in-the-Loop（人机交互）

通过 `interrupt_before` 或 `interrupt_after` 在特定节点暂停执行，等待人工干预：

```rust
let graph = StateGraph::<MessageState>::new()
    .add_node("agent", agent_node)
    .add_node("tools", tool_node)
    .set_entry_point("agent")
    .add_conditional_edges("agent", route_fn)
    .add_edge("tools", "agent")
    // 在执行工具前暂停，等待人工确认
    .interrupt_before(vec!["tools".to_string()]);

let compiled = graph.compile()?.with_checkpointer(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));

// 第一次执行会在 tools 节点前中断
let config = CheckpointConfig { thread_id: "t1".to_string() };
let result = compiled.invoke_with_config(initial_state, Some(config.clone())).await;
// result 是 Err，包含中断信息

// 人工审查后，更新状态并恢复执行
compiled.update_state(&config, approved_state).await?;
let final_state = compiled.invoke_with_config(
    MessageState::new(), // 会从检查点恢复
    Some(config),
).await?;
```

这对于需要人工确认的关键操作（如发送邮件、执行付款、危险工具调用）非常有用。

## 可视化

`CompiledGraph` 提供多种图可视化方法：

### Mermaid 格式

```rust
let mermaid = compiled.draw_mermaid();
println!("{}", mermaid);
```

输出类似：

```text
graph TD
    __start__(["__start__"])
    agent["agent"]
    tools["tools"]
    __end__(["__end__"])
    __start__ --> agent
    tools --> agent
    agent -.-> |continue| tools
    agent -.-> |end| __end__
```

### ASCII 文本格式

```rust
let ascii = compiled.draw_ascii();
println!("{}", ascii);
// 也可以使用 Display trait
println!("{}", compiled);
```

### Graphviz DOT 格式

```rust
let dot = compiled.draw_dot();
println!("{}", dot);
```

### 导出为图片

```rust
// 通过 mermaid.ink API 导出 PNG（需要网络）
compiled.draw_mermaid_png("graph.png").await?;

// 通过 mermaid.ink API 导出 SVG（需要网络）
compiled.draw_mermaid_svg("graph.svg").await?;

// 通过本地 Graphviz dot 命令导出 PNG（需要安装 Graphviz）
compiled.draw_png("graph.png")?;
```

## create_react_agent

`create_react_agent` 是一个便捷函数，自动构建包含 agent 节点和 tools 节点的 ReAct 循环 Graph：

```rust
use synaptic::graph::create_react_agent;

let graph = create_react_agent(model, tools)?;
```

它等价于手动构建以下 Graph 结构：

```text
        +----------+
        |  agent   | <-----------+
        +----+-----+             |
             |                   |
        has_tool_calls?          |
        /          \             |
      yes           no           |
       |             |           |
  +----v-----+   +--v--+        |
  |  tools   |   | END |        |
  +----+-----+   +-----+        |
       |                         |
       +-------------------------+
```

你也可以使用 `create_react_agent_with_options` 进行更细粒度的配置：

```rust
use synaptic::graph::{create_react_agent_with_options, ReactAgentOptions};

let options = ReactAgentOptions {
    // 自定义选项...
    ..Default::default()
};
let graph = create_react_agent_with_options(model, tools, options)?;
```

## 最佳实践

1. **使用 `MessageState`** -- 对于标准的聊天 Agent，`MessageState` 已经足够。只有在需要额外字段时才自定义状态。
2. **条件边应保持简单** -- 路由函数应只检查状态，不执行副作用。它接收不可变引用 `&S`。
3. **使用 `StreamMode::Updates` 减少数据传输** -- 当你只关心每个节点的变更时。
4. **使用 Checkpointer 支持长时间运行的工作流** -- 避免因超时或错误丢失中间状态。
5. **使用 `add_conditional_edges_with_path_map`** -- 提供 path_map 可以让可视化工具显示条件边的所有可能目标。
6. **图有最大迭代限制（100 次）** -- 防止无限循环。如果你的工作流确实需要更多迭代，请检查是否存在设计问题。
