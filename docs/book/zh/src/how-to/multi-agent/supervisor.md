# Supervisor 模式

Supervisor 模式使用一个中心模型将对话路由到专门的子 Agent。

## 工作原理

`create_supervisor` 构建一个以 `"supervisor"` 节点为中心的图。Supervisor 节点使用 Handoff 工具调用 ChatModel -- 每个子 Agent 对应一个工具。当模型发出 `transfer_to_<agent_name>` 工具调用时，图会路由到该子 Agent。当子 Agent 完成后，控制权返回到 Supervisor，Supervisor 可以再次委派或产生最终答案。

```text
         +------------+
         | supervisor |<-----+
         +-----+------+      |
           /       \          |
    agent_a     agent_b ------+
```

## API

```rust,ignore
use synaptic_graph::{create_supervisor, SupervisorOptions};

pub fn create_supervisor(
    model: Arc<dyn ChatModel>,
    agents: Vec<(String, CompiledGraph<MessageState>)>,
    options: SupervisorOptions,
) -> Result<CompiledGraph<MessageState>, SynapticError>;
```

### SupervisorOptions

| 字段 | 类型 | 描述 |
|------|------|------|
| `checkpointer` | `Option<Arc<dyn Checkpointer>>` | 跨调用持久化状态 |
| `store` | `Option<Arc<dyn Store>>` | 共享键值存储 |
| `system_prompt` | `Option<String>` | 覆盖默认的 Supervisor 提示词 |

如果未提供 `system_prompt`，则生成默认提示词：

> "You are a supervisor managing these agents: agent_a, agent_b. Use the transfer tools to delegate tasks to the appropriate agent. When the task is complete, respond directly to the user."

## 完整示例

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatModel, Message, Tool};
use synaptic_graph::{
    create_agent, create_supervisor, AgentOptions, MessageState, SupervisorOptions,
};

// Assume `model` implements ChatModel, `research_tools` and `writing_tools`
// are Vec<Arc<dyn Tool>>.

// 1. Create sub-agents
let researcher = create_agent(
    model.clone(),
    research_tools,
    AgentOptions {
        system_prompt: Some("You are a research assistant.".into()),
        ..Default::default()
    },
)?;

let writer = create_agent(
    model.clone(),
    writing_tools,
    AgentOptions {
        system_prompt: Some("You are a writing assistant.".into()),
        ..Default::default()
    },
)?;

// 2. Create the supervisor graph
let supervisor = create_supervisor(
    model,
    vec![
        ("researcher".to_string(), researcher),
        ("writer".to_string(), writer),
    ],
    SupervisorOptions {
        system_prompt: Some(
            "Route research questions to researcher, writing tasks to writer.".into(),
        ),
        ..Default::default()
    },
)?;

// 3. Invoke
let mut state = MessageState::new();
state.messages.push(Message::human("Write a summary of recent AI trends."));
let result = supervisor.invoke(state).await?.into_state();

println!("{}", result.messages.last().unwrap().content());
```

## 使用检查点

传入一个 checkpointer 以跨调用持久化 Supervisor 的状态：

```rust,ignore
use synaptic_graph::MemorySaver;

let supervisor = create_supervisor(
    model,
    agents,
    SupervisorOptions {
        checkpointer: Some(Arc::new(MemorySaver::new())),
        ..Default::default()
    },
)?;
```

## 使用 ScriptedChatModel 进行离线测试

你可以使用 `ScriptedChatModel` 在没有 API 密钥的情况下测试 Supervisor 图。编排 Supervisor 发出 Handoff 工具调用，并编排子 Agent 产生响应：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, Message, ToolCall};
use synaptic_models::ScriptedChatModel;
use synaptic_graph::{
    create_agent, create_supervisor, AgentOptions, MessageState, SupervisorOptions,
};

// Sub-agent model: responds directly (no tool calls)
let agent_model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("The research is complete."),
        usage: None,
    },
]);

// Supervisor model: first response transfers to researcher, second is final answer
let supervisor_model = ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "call_1".into(),
                name: "transfer_to_researcher".into(),
                arguments: "{}".into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("All done. Here is the summary."),
        usage: None,
    },
]);

let researcher = create_agent(
    Arc::new(agent_model),
    vec![],
    AgentOptions::default(),
)?;

let supervisor = create_supervisor(
    Arc::new(supervisor_model),
    vec![("researcher".to_string(), researcher)],
    SupervisorOptions::default(),
)?;

let mut state = MessageState::new();
state.messages.push(Message::human("Research AI trends."));
let result = supervisor.invoke(state).await?.into_state();
```

## 注意事项

- 每个子 Agent 被包装在一个 `SubAgentNode` 中，该节点调用 `graph.invoke(state)` 并将结果状态返回给 Supervisor。
- Supervisor 可以看到完整的消息历史，包括子 Agent 追加的消息。
- 当 Supervisor 产生没有工具调用的响应时，图终止。
