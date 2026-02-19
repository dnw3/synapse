# 多 Agent 模式

Synaptic 提供了预构建的多 Agent 编排模式，可以将多个独立的 Agent 组合成协作工作流。

## 模式对比

| 模式 | 协调者 | 路由方式 | 适用场景 |
|------|--------|----------|----------|
| **Supervisor** | 中心 Supervisor 模型 | Supervisor 决定下一个运行哪个子 Agent | 具有清晰任务边界的结构化委派 |
| **Swarm** | 无（去中心化） | 每个 Agent 直接移交给对等方 | 任何 Agent 都可以发起升级的有机协作 |
| **Handoff Tools** | 自定义 | 由你来连接拓扑结构 | 不适合 Supervisor 或 Swarm 的任意图结构 |

## 何时使用每种模式

**Supervisor** -- 当你有明确的层级关系时使用。一个模型读取对话并决定哪个专家 Agent 应该处理下一步。Supervisor 可以看到完整的消息历史，并且可以在完成时路由回自身。

**Swarm** -- 当 Agent 之间是对等关系时使用。每个 Agent 都有自己的模型、工具和一组 Handoff 工具，可以将控制权转移给任何其他 Agent。没有中心协调者；任何 Agent 都可以随时决定转移。

**Handoff Tools** -- 当你需要自定义拓扑结构时使用。`create_handoff_tool` 生成一个 `Tool`，用于表示转移到另一个 Agent 的意图。你可以在手动设计的任何图结构中注册这些工具。

## 核心类型

所有多 Agent 函数都位于 `synaptic_graph` 中：

```rust,ignore
use synaptic_graph::{
    create_supervisor, SupervisorOptions,
    create_swarm, SwarmAgent, SwarmOptions,
    create_handoff_tool,
    create_agent, AgentOptions,
    MessageState,
};
```

## 最小示例

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{
    create_agent, create_supervisor, AgentOptions, SupervisorOptions, MessageState,
};
use synaptic_core::Message;

// Build two sub-agents
let agent_a = create_agent(model.clone(), tools_a, AgentOptions::default())?;
let agent_b = create_agent(model.clone(), tools_b, AgentOptions::default())?;

// Wire them under a supervisor
let graph = create_supervisor(
    model,
    vec![
        ("agent_a".to_string(), agent_a),
        ("agent_b".to_string(), agent_b),
    ],
    SupervisorOptions::default(),
)?;

let mut state = MessageState::new();
state.messages.push(Message::human("Hello, delegate this."));
let result = graph.invoke(state).await?.into_state();
```

有关每种模式的详细用法，请参阅各个页面。
