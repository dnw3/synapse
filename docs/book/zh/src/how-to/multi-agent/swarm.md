# Swarm 模式

Swarm 模式创建一个去中心化的多 Agent 图，其中每个 Agent 都可以直接将控制权移交给任何其他 Agent。

## 工作原理

`create_swarm` 接收一个 `SwarmAgent` 定义列表。每个 Agent 都有自己的模型、工具和系统提示词。Synaptic 会自动为每个 Agent 生成指向其他所有 Agent 的 Handoff 工具（`transfer_to_<peer>`）并添加到该 Agent 的工具集中。一个共享的 `"tools"` 节点负责执行常规工具调用，并将 Handoff 工具调用路由到目标 Agent。

```text
    triage ----> tools ----> billing
       ^           |            |
       |           v            |
       +------- support <------+
```

列表中的第一个 Agent 是入口点。

## API

```rust,ignore
use synaptic_graph::{create_swarm, SwarmAgent, SwarmOptions};

pub fn create_swarm(
    agents: Vec<SwarmAgent>,
    options: SwarmOptions,
) -> Result<CompiledGraph<MessageState>, SynapticError>;
```

### SwarmAgent

| 字段 | 类型 | 描述 |
|------|------|------|
| `name` | `String` | 唯一的 Agent 标识符 |
| `model` | `Arc<dyn ChatModel>` | 该 Agent 使用的模型 |
| `tools` | `Vec<Arc<dyn Tool>>` | Agent 专用工具（Handoff 工具会自动添加） |
| `system_prompt` | `Option<String>` | 该 Agent 的可选系统提示词 |

### SwarmOptions

| 字段 | 类型 | 描述 |
|------|------|------|
| `checkpointer` | `Option<Arc<dyn Checkpointer>>` | 跨调用持久化状态 |
| `store` | `Option<Arc<dyn Store>>` | 共享键值存储 |

## 完整示例

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatModel, Message, Tool};
use synaptic_graph::{create_swarm, MessageState, SwarmAgent, SwarmOptions};

// Assume `model` implements ChatModel and *_tools are Vec<Arc<dyn Tool>>.

let swarm = create_swarm(
    vec![
        SwarmAgent {
            name: "triage".to_string(),
            model: model.clone(),
            tools: triage_tools,
            system_prompt: Some("You triage incoming requests.".into()),
        },
        SwarmAgent {
            name: "billing".to_string(),
            model: model.clone(),
            tools: billing_tools,
            system_prompt: Some("You handle billing questions.".into()),
        },
        SwarmAgent {
            name: "support".to_string(),
            model: model.clone(),
            tools: support_tools,
            system_prompt: Some("You provide technical support.".into()),
        },
    ],
    SwarmOptions::default(),
)?;

// The first agent ("triage") is the entry point.
let mut state = MessageState::new();
state.messages.push(Message::human("I need to update my payment method."));
let result = swarm.invoke(state).await?.into_state();

// The triage agent will call `transfer_to_billing`, routing to the billing agent.
println!("{}", result.messages.last().unwrap().content());
```

## 路由逻辑

1. 当一个 Agent 产生工具调用时，图会路由到 `"tools"` 节点。
2. tools 节点通过共享的 `SerialToolExecutor` 执行常规工具调用。
3. 对于 Handoff 工具（`transfer_to_<name>`），它会添加一条合成的工具响应消息并跳过执行。
4. tools 节点执行后，路由逻辑检查最后一条 AI 消息中的 Handoff 调用，并转移到目标 Agent。如果没有发生 Handoff，当前 Agent 继续执行。

## 使用 ScriptedChatModel 进行离线测试

通过为每个 Agent 的模型编排脚本，可以在没有 API 密钥的情况下测试 Swarm 图：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, Message, ToolCall};
use synaptic_models::ScriptedChatModel;
use synaptic_graph::{create_swarm, MessageState, SwarmAgent, SwarmOptions};

// Triage model: transfers to billing
let triage_model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "call_1".into(),
                name: "transfer_to_billing".into(),
                arguments: "{}".into(),
            }],
        ),
        usage: None,
    },
]));

// Billing model: responds directly
let billing_model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Your payment method has been updated."),
        usage: None,
    },
]));

let swarm = create_swarm(
    vec![
        SwarmAgent {
            name: "triage".to_string(),
            model: triage_model,
            tools: vec![],
            system_prompt: Some("Route requests to the right agent.".into()),
        },
        SwarmAgent {
            name: "billing".to_string(),
            model: billing_model,
            tools: vec![],
            system_prompt: Some("Handle billing questions.".into()),
        },
    ],
    SwarmOptions::default(),
)?;

let mut state = MessageState::new();
state.messages.push(Message::human("Update my payment method."));
let result = swarm.invoke(state).await?.into_state();
```

## 注意事项

- Swarm 至少需要一个 Agent。空列表会返回错误。
- 所有 Agent 的工具注册在一个共享的 `ToolRegistry` 中，因此工具名称必须在所有 Agent 之间唯一。
- 每个 Agent 有自己的模型，所以你可以混合使用不同的提供商（例如，用快速模型做分诊，用强大模型做支持）。
- Handoff 工具为所有对等方生成 -- 一个 Agent 不能将控制权移交给自己。
