# Quickstart

This guide walks you through creating and running a Deep Agent in three steps.

## Prerequisites

Add the required crates to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["deep", "openai"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Step 1: Create a Backend

The backend determines how the agent interacts with the outside world. For this quickstart we use `FilesystemBackend`, which reads and writes real files on your machine:

```rust,ignore
use synaptic::deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/tmp/my-workspace"));
```

For testing without touching the filesystem, swap in `StateBackend::new()` instead:

```rust,ignore
use synaptic::deep::backend::StateBackend;

let backend = Arc::new(StateBackend::new());
```

## Step 2: Create the Agent

Use `create_deep_agent` with a model and a `DeepAgentOptions`. The options struct has sensible defaults -- you only need to provide the backend:

```rust,ignore
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::openai::OpenAiChatModel;
use std::sync::Arc;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let options = DeepAgentOptions::new(backend);

let agent = create_deep_agent(model, options)?;
```

`create_deep_agent` wires up the full middleware stack (memory, skills, filesystem, subagents, summarization, tool-call patching), registers the filesystem tools, and compiles the underlying ReAct graph. It returns a `CompiledGraph<MessageState>`.

## Step 3: Run the Agent

Build a `MessageState` with your prompt and call `invoke`. The agent will reason, call tools, and return a final result:

```rust,ignore
use synaptic::graph::MessageState;
use synaptic::core::Message;

let state = MessageState::with_messages(vec![
    Message::human("Create a file called hello.txt containing 'Hello, world!'"),
]);
let result = agent.invoke(state).await?;
println!("{}", result.into_state().last_message_content());
```

## What Happens Under the Hood

When you call `agent.invoke(state)`:

1. **Memory loading** -- The `DeepMemoryMiddleware` checks for an `AGENTS.md` file via the backend and injects any saved context into the system prompt.
2. **Skills injection** -- The `SkillsMiddleware` scans the `.skills/` directory for `SKILL.md` files and adds matching skill instructions to the system prompt.
3. **Agent loop** -- The underlying ReAct graph enters its reason-act-observe loop. The model sees the filesystem tools and decides which ones to call.
4. **Tool execution** -- Each tool call (e.g. `write_file`) is dispatched through the backend. `FilesystemBackend` performs real I/O; `StateBackend` operates on an in-memory map.
5. **Summarization** -- If the conversation grows beyond the configured token threshold (default: 85% of 128,000 tokens), the `DeepSummarizationMiddleware` compresses older messages into a summary before the next model call.
6. **Tool-call patching** -- The `PatchToolCallsMiddleware` fixes malformed tool calls before they reach the executor.
7. **Final answer** -- When the model responds without tool calls, the graph terminates and `invoke` returns the `GraphResult<MessageState>`.

## Customizing Options

`DeepAgentOptions` fields can be set directly before passing to `create_deep_agent`:

```rust,ignore
let mut options = DeepAgentOptions::new(backend);
options.system_prompt = Some("You are a Rust expert.".to_string());
options.max_input_tokens = 64_000;
options.enable_subagents = false;

let agent = create_deep_agent(model, options)?;
```

Key defaults:

| Field | Default |
|-------|---------|
| `max_input_tokens` | 128,000 |
| `summarization_threshold` | 0.85 |
| `eviction_threshold` | 20,000 |
| `max_subagent_depth` | 3 |
| `skills_dir` | `".skills"` |
| `memory_file` | `"AGENTS.md"` |
| `enable_subagents` | `true` |
| `enable_filesystem` | `true` |
| `enable_skills` | `true` |
| `enable_memory` | `true` |

## Full Working Example

```rust,ignore
use std::sync::Arc;
use synaptic::core::Message;
use synaptic::deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(FilesystemBackend::new("/tmp/demo"));
    let options = DeepAgentOptions::new(backend);

    let agent = create_deep_agent(model, options)?;

    let state = MessageState::with_messages(vec![
        Message::human("What files are in the current directory?"),
    ]);
    let result = agent.invoke(state).await?;
    println!("{}", result.into_state().last_message_content());
    Ok(())
}
```

## Next Steps

- [Backends](backends.md) -- learn about State, Store, and Filesystem backends
- [Filesystem Tools](filesystem-tools.md) -- see what each tool does
- [Customization](customization.md) -- tune every option with `DeepAgentOptions`
