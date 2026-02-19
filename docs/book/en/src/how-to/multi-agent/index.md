# Multi-Agent Patterns

Synaptic provides prebuilt multi-agent orchestration patterns that compose individual agents into collaborative workflows.

## Pattern Comparison

| Pattern | Coordinator | Routing | Best For |
|---------|------------|---------|----------|
| **Supervisor** | Central supervisor model | Supervisor decides which sub-agent runs next | Structured delegation with clear task boundaries |
| **Swarm** | None (decentralized) | Each agent hands off to peers directly | Organic collaboration where any agent can escalate |
| **Handoff Tools** | Custom | You wire the topology | Arbitrary graphs that don't fit supervisor or swarm |

## When to Use Each

**Supervisor** -- Use when you have a clear hierarchy. A single model reads the conversation and decides which specialist agent should handle the next step. The supervisor sees the full message history and can route back to itself when done.

**Swarm** -- Use when agents are peers. Each agent has its own model, tools, and a set of handoff tools to transfer to any other agent. There is no central coordinator; any agent can decide to transfer at any time.

**Handoff Tools** -- Use when you need a custom topology. `create_handoff_tool` produces a `Tool` that signals an intent to transfer to another agent. You can register these in any graph structure you design manually.

## Key Types

All multi-agent functions live in `synaptic_graph`:

```rust,ignore
use synaptic_graph::{
    create_supervisor, SupervisorOptions,
    create_swarm, SwarmAgent, SwarmOptions,
    create_handoff_tool,
    create_agent, AgentOptions,
    MessageState,
};
```

## Minimal Example

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

See the individual pages for detailed usage of each pattern.
