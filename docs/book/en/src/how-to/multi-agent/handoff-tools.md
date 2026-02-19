# Handoff Tools

Handoff tools signal an intent to transfer a conversation from one agent to another.

## create_handoff_tool

The `create_handoff_tool` function creates a `Tool` that, when called, returns a transfer message. The tool is named `transfer_to_<agent_name>` and routing logic uses this naming convention to detect handoffs.

```rust,ignore
use synaptic_graph::create_handoff_tool;

let handoff = create_handoff_tool("billing", "Transfer to the billing specialist");
// handoff.name()        => "transfer_to_billing"
// handoff.description() => "Transfer to the billing specialist"
```

When invoked, the tool returns:

```json
"Transferring to agent 'billing'."
```

## Using Handoff Tools in Custom Agents

You can register handoff tools alongside regular tools when building an agent:

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, create_handoff_tool, AgentOptions};

let escalate = create_handoff_tool("human_review", "Escalate to a human reviewer");

let mut all_tools: Vec<Arc<dyn Tool>> = my_tools;
all_tools.push(escalate);

let agent = create_agent(model, all_tools, AgentOptions::default())?;
```

The model will see `transfer_to_human_review` as an available tool. When it decides to call it, your graph's conditional edges can detect the handoff and route accordingly.

## Building Custom Topologies

For workflows that don't fit the supervisor or swarm patterns, combine handoff tools with a manual `StateGraph`:

```rust,ignore
use std::collections::HashMap;
use synaptic_graph::{
    create_handoff_tool, StateGraph, FnNode, MessageState, END,
};

// Create handoff tools
let to_reviewer = create_handoff_tool("reviewer", "Send to reviewer");
let to_publisher = create_handoff_tool("publisher", "Send to publisher");

// Build nodes (agent_node, reviewer_node, publisher_node defined elsewhere)

let graph = StateGraph::new()
    .add_node("drafter", drafter_node)
    .add_node("reviewer", reviewer_node)
    .add_node("publisher", publisher_node)
    .set_entry_point("drafter")
    .add_conditional_edges("drafter", |state: &MessageState| {
        if let Some(last) = state.last_message() {
            for tc in last.tool_calls() {
                if tc.name == "transfer_to_reviewer" {
                    return "reviewer".to_string();
                }
                if tc.name == "transfer_to_publisher" {
                    return "publisher".to_string();
                }
            }
        }
        END.to_string()
    })
    .add_edge("reviewer", "drafter")
    .add_edge("publisher", END)
    .compile()?;
```

## Naming Convention

The handoff tool is always named `transfer_to_<agent_name>`. Both `create_supervisor` and `create_swarm` rely on this convention internally when routing. If you build custom topologies, match against the same pattern in your conditional edges.

## Notes

- Handoff tools take no arguments. The model calls them with an empty object `{}`.
- The tool itself only returns a string message -- the actual routing is handled by the graph's conditional edges, not by the tool execution.
- You can create multiple handoff tools per agent to build complex routing graphs (e.g., an agent can hand off to three different specialists).
