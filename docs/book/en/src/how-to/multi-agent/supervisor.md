# Supervisor Pattern

The supervisor pattern uses a central model to route conversations to specialized sub-agents.

## How It Works

`create_supervisor` builds a graph with a `"supervisor"` node at the center. The supervisor node calls a ChatModel with handoff tools -- one per sub-agent. When the model emits a `transfer_to_<agent_name>` tool call, the graph routes to that sub-agent. When the sub-agent finishes, control returns to the supervisor, which can delegate again or produce a final answer.

```text
         +------------+
         | supervisor |<-----+
         +-----+------+      |
           /       \          |
    agent_a     agent_b ------+
```

## API

```rust,ignore
use synaptic::graph::{create_supervisor, SupervisorOptions};

pub fn create_supervisor(
    model: Arc<dyn ChatModel>,
    agents: Vec<(String, CompiledGraph<MessageState>)>,
    options: SupervisorOptions,
) -> Result<CompiledGraph<MessageState>, SynapticError>;
```

### SupervisorOptions

| Field | Type | Description |
|-------|------|-------------|
| `checkpointer` | `Option<Arc<dyn Checkpointer>>` | Persist state across invocations |
| `store` | `Option<Arc<dyn Store>>` | Shared key-value store |
| `system_prompt` | `Option<String>` | Override the default supervisor prompt |

If no `system_prompt` is provided, a default is generated:

> "You are a supervisor managing these agents: agent_a, agent_b. Use the transfer tools to delegate tasks to the appropriate agent. When the task is complete, respond directly to the user."

## Full Example

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, Message, Tool};
use synaptic::graph::{
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

## With Checkpointing

Pass a checkpointer to persist the supervisor's state across calls:

```rust,ignore
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;
use std::sync::Arc;

let supervisor = create_supervisor(
    model,
    agents,
    SupervisorOptions {
        checkpointer: Some(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new())))),
        ..Default::default()
    },
)?;
```

## Offline Testing with ScriptedChatModel

You can test supervisor graphs without an API key using `ScriptedChatModel`. Script the supervisor to emit a handoff tool call, and script the sub-agent to produce a response:

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatResponse, Message, ToolCall};
use synaptic::models::ScriptedChatModel;
use synaptic::graph::{
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

## Notes

- Each sub-agent is wrapped in a `SubAgentNode` that calls `graph.invoke(state)` and returns the resulting state back to the supervisor.
- The supervisor sees the full message history, including messages appended by sub-agents.
- The graph terminates when the supervisor produces a response with no tool calls.
