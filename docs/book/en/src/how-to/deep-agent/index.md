# Deep Agent

A Deep Agent is a high-level agent abstraction that combines a middleware stack, a backend for filesystem and state operations, and a factory for creating fully-configured agents in a single call. It is designed for tasks that require reading and writing files, spawning subagents, loading skills, and maintaining persistent memory -- the kinds of workflows typically associated with coding assistants and autonomous research agents.

## Architecture

A Deep Agent is assembled from layers that wrap a core ReAct agent graph:

```text
+-----------------------------------------------+
|              Deep Agent                        |
|  +------------------------------------------+ |
|  |  Middleware Stack                         | |
|  |  - DeepMemoryMiddleware (AGENTS.md)      | |
|  |  - SkillsMiddleware (SKILL.md injection) | |
|  |  - FilesystemMiddleware (tool eviction)  | |
|  |  - SubAgentMiddleware (task tool)        | |
|  |  - DeepSummarizationMiddleware           | |
|  |  - PatchToolCallsMiddleware              | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  Filesystem Tools                         | |
|  |  ls, read_file, write_file, edit_file,    | |
|  |  glob, grep (+execute if supported)       | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  Backend (State / Store / Filesystem)     | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  ReAct Agent Graph (agent + tools nodes)  | |
|  +------------------------------------------+ |
+-----------------------------------------------+
```

## Core Capabilities

| Capability | Description |
|------------|-------------|
| Filesystem tools | Read, write, edit, search, and list files through a pluggable backend. An `execute` tool is added when the backend supports it. |
| Subagents | Spawn child agents for isolated subtasks with recursion depth control (`max_subagent_depth`) |
| Skills | Load `SKILL.md` files from a configurable directory that inject domain-specific instructions into the system prompt |
| Memory | Persist learned context in `AGENTS.md` and reload it across sessions |
| Summarization | Auto-summarize conversation history when context length exceeds `summarization_threshold` of `max_input_tokens` |
| Backend abstraction | Swap between in-memory (`StateBackend`), persistent store (`StoreBackend`), and real filesystem (`FilesystemBackend`) backends |

## Minimal Example

```rust,ignore
use synaptic::deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;
use synaptic::core::Message;
use std::sync::Arc;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let backend = Arc::new(FilesystemBackend::new("/path/to/workspace"));
let options = DeepAgentOptions::new(backend);

let agent = create_deep_agent(model, options)?;

let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("List the Rust files in src/"),
])).await?;
println!("{}", result.into_state().last_message_content());
```

`create_deep_agent` returns a `CompiledGraph<MessageState>` -- the same graph type used by `create_react_agent`. You invoke it with a `MessageState` containing your input messages and receive a `GraphResult<MessageState>` back.

## Guides

- [Quickstart](quickstart.md) -- create and run your first Deep Agent
- [Backends](backends.md) -- choose between State, Store, and Filesystem backends
- [Filesystem Tools](filesystem-tools.md) -- reference for the built-in tools
- [Subagents](subagents.md) -- delegate subtasks to child agents
- [Skills](skills.md) -- extend agent behavior with SKILL.md files
- [Memory](memory.md) -- persistent agent memory via AGENTS.md
- [Customization](customization.md) -- full DeepAgentOptions reference

## When to Use a Deep Agent

Use a Deep Agent when your task involves **file manipulation**, **multi-step reasoning over project state**, or **spawning subtasks**. If you only need a simple question-answering loop, a plain `create_react_agent` is sufficient. Deep Agent adds the infrastructure layers that turn a basic ReAct loop into an autonomous coding or research assistant.
