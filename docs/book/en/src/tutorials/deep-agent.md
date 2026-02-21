# Build a Deep Agent

This tutorial walks you through building a Deep Agent step by step. You will start with a minimal agent that can read and write files, then progressively add skills, subagents, memory, and custom configuration. By the end you will understand every layer of the deep agent stack.

## What You Will Build

A Deep Agent that:

1. Uses filesystem tools to read, write, and search files.
2. Loads domain-specific skills from `SKILL.md` files.
3. Delegates subtasks to custom subagents.
4. Persists learned knowledge in an `AGENTS.md` memory file.
5. Auto-summarizes conversation history when context grows large.

## Prerequisites

Create a new binary crate:

```bash
cargo new deep-agent-tutorial
cd deep-agent-tutorial
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["deep", "openai"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Set your OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
```

## Step 1: Create a Backend

Every deep agent needs a **backend** that provides filesystem operations. The backend is the agent's view of the world -- it determines where files are read from and written to.

Synaptic ships three backend implementations:

- **`StateBackend`** -- in-memory `HashMap<String, String>`. Great for tests and sandboxed demos. No real files are touched.
- **`StoreBackend`** -- delegates to a Synaptic `Store` implementation. Useful when you already have a store with semantic search.
- **`FilesystemBackend`** -- reads and writes real files on disk, sandboxed to a root directory. Requires the `filesystem` feature flag.

For this tutorial we use `StateBackend` so everything runs in memory:

```rust,ignore
use std::sync::Arc;
use synaptic::deep::backend::{Backend, StateBackend};

let backend = Arc::new(StateBackend::new());
```

The deep agent wraps each backend operation as a tool that the model can call.

## Step 2: Create a Minimal Deep Agent

The `create_deep_agent` function assembles a full middleware stack and tool set in one call. It returns a `CompiledGraph<MessageState>` -- the same graph type used by `create_agent` and `create_react_agent`, so you run it with `invoke()`.

```rust,ignore
use std::sync::Arc;
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::deep::backend::StateBackend;
use synaptic::core::{ChatModel, Message};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(StateBackend::new());

    let options = DeepAgentOptions::new(backend.clone());
    let agent = create_deep_agent(model.clone(), options)?;

    let state = MessageState::with_messages(vec![
        Message::human("Create a file called hello.txt with 'Hello World!'"),
    ]);
    let result = agent.invoke(state).await?;
    let final_state = result.into_state();
    println!("{}", final_state.last_message().unwrap().content());

    Ok(())
}
```

What happens under the hood:

1. `DeepAgentOptions::new(backend)` configures sensible defaults -- filesystem tools enabled, skills enabled, memory enabled, subagents enabled.
2. `create_deep_agent` assembles 6 middleware layers and 6-7 tools, then calls `create_agent` to produce a compiled graph.
3. `agent.invoke(state)` runs the agent loop. The model sees the `write_file` tool and calls it to create `hello.txt` in the backend.
4. `result.into_state()` unwraps the `GraphResult` into the final `MessageState`.

Because we are using `StateBackend`, the file lives only in memory. You can verify it:

```rust,ignore
let content = backend.read_file("hello.txt", 0, 100).await?;
assert!(content.contains("Hello World!"));
```

## Step 3: Use Filesystem Tools

The deep agent automatically registers these tools: `ls`, `read_file`, `write_file`, `edit_file`, `glob`, `grep`, and `execute` (if the backend supports shell commands).

Let us seed the backend with a small Rust project and ask the agent to analyze it:

```rust,ignore
// Seed files into the in-memory backend
backend.write_file("src/main.rs", r#"fn main() {
    let items = vec![1, 2, 3, 4, 5];
    let mut total = 0;
    for i in items {
        total = total + i;
    }
    println!("Total: {}", total);
    // TODO: add error handling
    // TODO: extract into a function
}
"#).await?;

backend.write_file("Cargo.toml", r#"[package]
name = "sample"
version = "0.1.0"
edition = "2021"
"#).await?;

let state = MessageState::with_messages(vec![
    Message::human("Read src/main.rs. List all the TODO comments and suggest improvements."),
]);
let result = agent.invoke(state).await?;
let final_state = result.into_state();
println!("{}", final_state.last_message().unwrap().content());
```

The agent calls `read_file` to get the source, finds the TODO comments, and responds with suggestions. You can follow up with a write request:

```rust,ignore
let state = MessageState::with_messages(vec![
    Message::human(
        "Create src/lib.rs with a public function `sum_items(items: &[i32]) -> i32` \
         that uses iter().sum(). Then update src/main.rs to use it."
    ),
]);
let result = agent.invoke(state).await?;
```

The agent uses `write_file` and `edit_file` to make the changes.

## Step 4: Add Skills

Skills are domain-specific instructions stored as `SKILL.md` files in the backend. The `SkillsMiddleware` scans `{skills_dir}/*/SKILL.md` on each model call, parses YAML frontmatter for `name` and `description`, and injects a skill index into the system prompt. The agent can then `read_file` any skill for full details.

Write a skill file directly to the backend:

```rust,ignore
backend.write_file(
    ".skills/testing/SKILL.md",
    "---\nname: testing\ndescription: Write comprehensive tests\n---\n\
     # Testing Skill\n\n\
     When asked to test Rust code:\n\n\
     1. Create a `tests/` module with `#[cfg(test)]`.\n\
     2. Write at least one happy-path test and one edge-case test.\n\
     3. Use `assert_eq!` with descriptive messages.\n\
     4. Test error paths with `assert!(result.is_err())`.\n"
).await?;
```

Skills are enabled by default (`enable_skills = true`). When the agent processes a request, it sees the skill index in its system prompt:

```text
<available_skills>
- **testing**: Write comprehensive tests (read `.skills/testing/SKILL.md` for details)
</available_skills>
```

The agent can call `read_file` on `.skills/testing/SKILL.md` to get the full instructions. This is progressive disclosure -- the index is always small, and full skill content is loaded on demand.

You can add multiple skills:

```rust,ignore
backend.write_file(
    ".skills/refactoring/SKILL.md",
    "---\nname: refactoring\ndescription: Rust refactoring best practices\n---\n\
     # Refactoring Skill\n\n\
     1. Prefer `iter().sum()` over manual loops.\n\
     2. Add `#[must_use]` to pure functions.\n\
     3. Run clippy before and after changes.\n"
).await?;
```

## Step 5: Add Custom Subagents

The deep agent can spawn child agents via a `task` tool. Each child gets its own conversation, runs the same middleware stack, and returns a summary to the parent.

Define custom subagent types with `SubAgentDef`:

```rust,ignore
use synaptic::deep::SubAgentDef;

let mut options = DeepAgentOptions::new(backend.clone());
options.subagents = vec![SubAgentDef {
    name: "researcher".to_string(),
    description: "Research specialist".to_string(),
    system_prompt: "You are a research assistant. Use grep and read_file to \
                    find information in the codebase. Report findings concisely."
        .to_string(),
    tools: vec![], // inherits filesystem tools from the deep agent
}];
let agent = create_deep_agent(model.clone(), options)?;
```

When the model calls the `task` tool, it passes a `description` and an optional `agent_type`. If `agent_type` matches a `SubAgentDef` name, the child uses that definition's system prompt and extra tools. Otherwise a general-purpose child agent is spawned.

Subagent depth is bounded by `max_subagent_depth` (default 3) to prevent runaway recursion. You can disable subagents entirely:

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_subagents = false;
let agent = create_deep_agent(model.clone(), options)?;
```

## Step 6: Add Memory Persistence

The `DeepMemoryMiddleware` loads a memory file from the backend on each model call and injects it into the system prompt wrapped in `<agent_memory>` tags. Write an initial memory file:

```rust,ignore
backend.write_file(
    "AGENTS.md",
    "# Agent Memory\n\n\
     - Always use Rust idioms\n\
     - Prefer async/await over blocking I/O\n\
     - User prefers 4-space indentation\n"
).await?;

let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = true; // this is already the default
let agent = create_deep_agent(model.clone(), options)?;
```

The agent now sees this in its system prompt on every call:

```text
<agent_memory>
# Agent Memory

- Always use Rust idioms
- Prefer async/await over blocking I/O
- User prefers 4-space indentation
</agent_memory>
```

The memory file path defaults to `"AGENTS.md"`. You can change it:

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("project-notes.md".to_string());
```

The agent can update memory by calling `write_file` or `edit_file` on the memory file. Future sessions will pick up the changes automatically.

## Step 7: Customize Options

`DeepAgentOptions` gives you control over the entire agent stack:

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());

// System prompt prepended to all model calls
options.system_prompt = Some("You are a coding assistant.".to_string());

// Token budget and summarization
options.max_input_tokens = 128_000;       // default
options.summarization_threshold = 0.85;   // default (85% of max)
options.eviction_threshold = 20_000;      // evict large tool results (default)

// Subagent configuration
options.max_subagent_depth = 3;           // default
options.enable_subagents = true;          // default

// Feature toggles
options.enable_filesystem = true;         // default
options.enable_skills = true;             // default
options.enable_memory = true;             // default

// Paths in the backend
options.skills_dir = Some(".skills".to_string());    // default
options.memory_file = Some("AGENTS.md".to_string()); // default

// Extensibility: add your own tools, middleware, checkpointer, or store
options.tools = vec![];
options.middleware = vec![];
options.checkpointer = None;
options.store = None;
options.subagents = vec![];

let agent = create_deep_agent(model.clone(), options)?;
```

## Step 8: Putting It All Together

Here is a complete example that combines everything:

```rust,ignore
use std::sync::Arc;
use synaptic::deep::{create_deep_agent, DeepAgentOptions, SubAgentDef};
use synaptic::deep::backend::StateBackend;
use synaptic::core::{ChatModel, Message};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(StateBackend::new());

    // Seed the workspace
    backend.write_file("src/main.rs", "fn main() {\n    println!(\"hello\");\n}\n").await?;

    // Add a skill
    backend.write_file(
        ".skills/testing/SKILL.md",
        "---\nname: testing\ndescription: Write comprehensive tests\n---\n# Testing\nAlways write unit tests.\n"
    ).await?;

    // Add agent memory
    backend.write_file("AGENTS.md", "# Memory\n- Use Rust 2021 edition\n").await?;

    // Configure the deep agent
    let mut options = DeepAgentOptions::new(backend.clone());
    options.system_prompt = Some("You are a senior Rust engineer. Be concise.".to_string());
    options.max_input_tokens = 64_000;
    options.summarization_threshold = 0.80;
    options.max_subagent_depth = 2;
    options.subagents = vec![SubAgentDef {
        name: "researcher".to_string(),
        description: "Code research specialist".to_string(),
        system_prompt: "You research codebases and report findings.".to_string(),
        tools: vec![],
    }];

    let agent = create_deep_agent(model, options)?;

    // Run the agent
    let state = MessageState::with_messages(vec![
        Message::human(
            "Audit this project: read all source files, find TODOs, \
             and write a summary to REPORT.md."
        ),
    ]);
    let result = agent.invoke(state).await?;
    let final_state = result.into_state();
    println!("{}", final_state.last_message().unwrap().content());

    // Verify the report was created
    let report = backend.read_file("REPORT.md", 0, 100).await?;
    println!("--- REPORT.md ---\n{}", report);

    Ok(())
}
```

## How the Middleware Stack Works

`create_deep_agent` assembles this middleware stack in order:

1. **DeepMemoryMiddleware** -- reads `AGENTS.md` and appends it to the system prompt.
2. **SkillsMiddleware** -- scans `.skills/*/SKILL.md` and injects a skill index into the system prompt.
3. **FilesystemMiddleware** -- registers filesystem tools. Evicts results larger than `eviction_threshold` tokens to `.evicted/` files with a preview.
4. **SubAgentMiddleware** -- provides the `task` tool for spawning child agents.
5. **DeepSummarizationMiddleware** -- summarizes older messages when token count exceeds the threshold, saving full history to `.context/history_N.md`.
6. **PatchToolCallsMiddleware** -- fixes malformed tool calls (strips code fences, deduplicates IDs, removes empty names).
7. **User middleware** -- anything in `options.middleware` runs last.

## Using a Real Filesystem Backend

For production use, enable the `filesystem` feature to work with real files:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["deep", "openai"] }
synaptic-deep = { version = "0.2", features = ["filesystem"] }
```

> **Note:** The `filesystem` feature is on the `synaptic-deep` crate directly because the `synaptic` facade does not forward it. Add `synaptic-deep` as an explicit dependency when you need `FilesystemBackend`.

```rust,ignore
use synaptic::deep::backend::FilesystemBackend;

let backend = Arc::new(FilesystemBackend::new("/path/to/workspace"));
let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;
```

`FilesystemBackend` sandboxes all operations to the root directory. Path traversal via `..` is rejected. It also supports shell command execution via the `execute` tool.

## Offline Mode (No API Key Required)

For testing and CI, combine `StateBackend` with `ScriptedChatModel` to run the entire deep agent without network access:

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, ChatResponse, Message, ToolCall};
use synaptic::models::ScriptedChatModel;
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::deep::backend::StateBackend;
use synaptic::graph::MessageState;

let backend = Arc::new(StateBackend::new());

// Script the model to: 1) write a file, 2) respond
let model: Arc<dyn ChatModel> = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "Creating the file.",
            vec![ToolCall {
                id: "call_1".into(),
                name: "write_file".into(),
                arguments: r#"{"path": "/output.txt", "content": "Hello from offline test!"}"#.into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("Done! Created output.txt."),
        usage: None,
    },
]));

let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

let state = MessageState::with_messages(vec![
    Message::human("Create output.txt with a greeting."),
]);
let result = agent.invoke(state).await?.into_state();

// Verify the file was created in the virtual filesystem
let content = backend.read_file("/output.txt", 0, 100).await?;
assert!(content.contains("Hello from offline test!"));
```

This approach is ideal for:
- **Unit tests** -- deterministic, no API costs, fast execution
- **CI pipelines** -- no secrets required
- **Demos** -- runs anywhere without configuration

## What You Built

Over the course of this tutorial you:

1. Created a `StateBackend` as an in-memory filesystem for the agent.
2. Used `create_deep_agent` to assemble a full agent with tools and middleware.
3. Ran the agent with `invoke()` on a `MessageState` and extracted results with `into_state()`.
4. Registered built-in filesystem tools (`ls`, `read_file`, `write_file`, `edit_file`, `glob`, `grep`).
5. Added domain skills via `SKILL.md` files with YAML frontmatter.
6. Defined custom subagents with `SubAgentDef` for task delegation.
7. Enabled persistent memory via `AGENTS.md`.
8. Customized every option through `DeepAgentOptions`.

## Next Steps

- [Multi-Agent Patterns](../how-to/multi-agent/index.md) -- supervisor and swarm architectures
- [Middleware](../how-to/middleware/index.md) -- write custom middleware for the agent stack
- [Store](../how-to/store/index.md) -- persistent key-value storage with semantic search
