# Subagents

A Deep Agent can spawn child agents -- called **subagents** -- to handle isolated subtasks. Subagents run in their own context, with their own conversation history, and return a result to the parent agent when they finish.

## Task Tool

When subagents are enabled, `create_deep_agent` adds a built-in **task** tool. When the parent agent calls the `task` tool, a new child deep agent is created via `create_deep_agent()` with the same model and backend, runs the requested subtask, and returns its final answer as the tool result.

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.enable_subagents = true; // enabled by default
let agent = create_deep_agent(model, options)?;

// The agent can now call the "task" tool in its reasoning loop.
// Example tool call the model might emit:
// { "name": "task", "arguments": { "description": "Refactor the parse module" } }
```

The `task` tool accepts two parameters:

| Parameter     | Required | Description |
|---------------|----------|-------------|
| `description` | yes      | A detailed description of the task for the sub-agent |
| `agent_type`  | no       | Name of a custom sub-agent type to spawn (defaults to `"general-purpose"`) |

## SubAgentDef

For more control, define named subagent types with `SubAgentDef`. Each definition specifies a name, description, system prompt, and an optional tool set. `SubAgentDef` is a plain struct -- create it with a struct literal:

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions, SubAgentDef};

let mut options = DeepAgentOptions::new(backend);
options.subagents = vec![
    SubAgentDef {
        name: "researcher".to_string(),
        description: "Research specialist".to_string(),
        system_prompt: "You are a research assistant. Find relevant files and summarize them.".to_string(),
        tools: vec![], // inherits default deep agent tools
    },
    SubAgentDef {
        name: "writer".to_string(),
        description: "Code writer".to_string(),
        system_prompt: "You are a code writer. Implement the requested changes.".to_string(),
        tools: vec![],
    },
];
let agent = create_deep_agent(model, options)?;
```

When the parent agent calls the `task` tool with `"agent_type": "researcher"`, the `TaskTool` finds the matching `SubAgentDef` by name and uses its `system_prompt` and `tools` for the child agent. If no matching definition is found, a general-purpose child agent is spawned with default settings.

## Recursion Depth Control

Subagents can themselves spawn further subagents. To prevent unbounded recursion, configure `max_subagent_depth`:

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.max_subagent_depth = 3; // default is 3
let agent = create_deep_agent(model, options)?;
```

The `SubAgentMiddleware` tracks the current depth with an `AtomicUsize` counter. When the depth limit is reached, the `task` tool returns an error instead of spawning a new agent. The parent agent sees this error as a tool result and can adjust its strategy.

## Context Isolation

Each subagent starts with a fresh conversation. The parent's message history is **not** forwarded. This keeps the subagent focused and avoids blowing the context window. The only information the subagent receives is:

1. Its own system prompt (from `SubAgentDef` or the default deep agent prompt).
2. The task description provided by the parent, sent as a `Message::human()`.
3. The shared backend -- subagents read and write the same workspace.

The child agent is a full deep agent created via `create_deep_agent()`, so it has access to the same filesystem tools, skills, and middleware stack as the parent (subject to the depth limit for further subagent spawning).

When the subagent finishes, only the content of its last AI message is returned to the parent as a tool result string. Intermediate reasoning and tool calls are discarded.

## Example: Delegating a Research Task

```rust,ignore
use std::sync::Arc;
use synaptic_core::Message;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_graph::MessageState;

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;

let state = MessageState::with_messages(vec![
    Message::human("Find all TODO comments in the codebase and write a summary to TODO_REPORT.md"),
]);
let result = agent.invoke(state).await?;
let final_state = result.into_state();

// Under the hood, the agent may call:
//   task({ "description": "Search for TODO comments in all .rs files" })
// The subagent runs, returns results, and the parent writes the report.
```
