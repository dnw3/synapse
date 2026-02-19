# Swarm Pattern

The swarm pattern creates a decentralized multi-agent graph where every agent can hand off to any other agent directly.

## How It Works

`create_swarm` takes a list of `SwarmAgent` definitions. Each agent has its own model, tools, and system prompt. Synaptic automatically generates handoff tools (`transfer_to_<peer>`) for every other agent and adds them to each agent's tool set. A shared `"tools"` node executes regular tool calls and routes handoff tool calls to the target agent.

```text
    triage ----> tools ----> billing
       ^           |            |
       |           v            |
       +------- support <------+
```

The first agent in the list is the entry point.

## API

```rust,ignore
use synaptic_graph::{create_swarm, SwarmAgent, SwarmOptions};

pub fn create_swarm(
    agents: Vec<SwarmAgent>,
    options: SwarmOptions,
) -> Result<CompiledGraph<MessageState>, SynapticError>;
```

### SwarmAgent

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Unique agent identifier |
| `model` | `Arc<dyn ChatModel>` | The model this agent uses |
| `tools` | `Vec<Arc<dyn Tool>>` | Agent-specific tools (handoff tools are added automatically) |
| `system_prompt` | `Option<String>` | Optional system prompt for this agent |

### SwarmOptions

| Field | Type | Description |
|-------|------|-------------|
| `checkpointer` | `Option<Arc<dyn Checkpointer>>` | Persist state across invocations |
| `store` | `Option<Arc<dyn Store>>` | Shared key-value store |

## Full Example

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

## Routing Logic

1. When an agent produces tool calls, the graph routes to the `"tools"` node.
2. The tools node executes regular tool calls via the shared `SerialToolExecutor`.
3. For handoff tools (`transfer_to_<name>`), it adds a synthetic tool response message and skips execution.
4. After the tools node, routing inspects the last AI message for handoff calls and transfers to the target agent. If no handoff occurred, the current agent continues.

## Offline Testing with ScriptedChatModel

Test swarm graphs without API keys by scripting each agent's model:

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

## Notes

- The swarm requires at least one agent. An empty list returns an error.
- All agent tools are registered in a single shared `ToolRegistry`, so tool names must be unique across agents.
- Each agent has its own model, so you can mix providers (e.g., a fast model for triage, a powerful model for support).
- Handoff tools are generated for all peers -- an agent cannot hand off to itself.
