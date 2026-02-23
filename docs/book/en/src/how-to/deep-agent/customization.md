# Customization

Every aspect of a Deep Agent can be tuned through `DeepAgentOptions`. This page is a field-by-field reference with examples.

## DeepAgentOptions Reference

`DeepAgentOptions` uses direct field assignment rather than a builder pattern. Create an instance with `DeepAgentOptions::new(backend)` to get sensible defaults, then override fields as needed:

```rust,ignore
use std::sync::Arc;
use synaptic::deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend.clone());
options.system_prompt = Some("You are a senior Rust engineer.".into());
options.max_subagent_depth = 2;

let agent = create_deep_agent(model, options)?;
```

### Full Field List

```rust,ignore
pub struct DeepAgentOptions {
    pub backend: Arc<dyn Backend>,                    // required
    pub system_prompt: Option<String>,                // None
    pub tools: Vec<Arc<dyn Tool>>,                    // empty
    pub middleware: Vec<Arc<dyn AgentMiddleware>>,     // empty
    pub checkpointer: Option<Arc<dyn Checkpointer>>,  // None
    pub store: Option<Arc<dyn Store>>,                // None
    pub max_input_tokens: usize,                      // 128_000
    pub summarization_threshold: f64,                  // 0.85
    pub eviction_threshold: usize,                     // 20_000
    pub max_subagent_depth: usize,                     // 3
    pub skills_dir: Option<String>,                    // Some(".skills")
    pub memory_file: Option<String>,                   // Some("AGENTS.md")
    pub subagents: Vec<SubAgentDef>,                   // empty
    pub enable_subagents: bool,                        // true
    pub enable_filesystem: bool,                       // true
    pub enable_skills: bool,                           // true
    pub enable_memory: bool,                           // true
}
```

## Field Details

### backend

The backend provides filesystem operations for the agent. This is the only required argument to `DeepAgentOptions::new()`. All other fields have defaults.

```rust,ignore
use synaptic::deep::backend::FilesystemBackend;

let backend = Arc::new(FilesystemBackend::new("/home/user/project"));
let options = DeepAgentOptions::new(backend);
```

### system_prompt

Override the default system prompt entirely. When `None`, the agent uses a built-in prompt that describes the filesystem tools and expected behavior.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.system_prompt = Some("You are a Rust expert. Use the provided tools to help.".into());
```

### tools

Additional tools beyond the built-in filesystem tools. These are added to the agent's tool registry and made available to the model.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.tools = vec![
    Arc::new(MyCustomTool),
    Arc::new(DatabaseQueryTool::new(db_pool)),
];
```

### middleware

Custom middleware layers that run after the entire built-in stack. See [Middleware Stack](#middleware-stack) for ordering details.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.middleware = vec![
    Arc::new(AuditLogMiddleware::new(log_file)),
];
```

### checkpointer

Optional checkpointer for graph state persistence. When provided, the agent can resume from checkpoints.

```rust,ignore
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;

let mut options = DeepAgentOptions::new(backend.clone());
options.checkpointer = Some(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));
```

### store

Optional store for runtime tool injection via `ToolRuntime`.

```rust,ignore
use synaptic::store::InMemoryStore;

let mut options = DeepAgentOptions::new(backend.clone());
options.store = Some(Arc::new(InMemoryStore::new()));
```

### max_input_tokens

Maximum input tokens before summarization is considered (default `128_000`). The `DeepSummarizationMiddleware` uses this together with `summarization_threshold` to decide when to compress context.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.max_input_tokens = 200_000; // for models with larger context windows
```

### summarization_threshold

Fraction of `max_input_tokens` at which summarization triggers (default `0.85`). When context exceeds `max_input_tokens * summarization_threshold` tokens, the middleware summarizes older messages.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.summarization_threshold = 0.70; // summarize earlier
```

### eviction_threshold

Token count above which tool results are evicted to files by the `FilesystemMiddleware` (default `20_000`). Large tool outputs are written to a file and replaced with a reference.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.eviction_threshold = 10_000; // evict smaller results
```

### max_subagent_depth

Maximum recursion depth for nested subagent spawning (default `3`). Prevents runaway agent chains.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.max_subagent_depth = 2;
```

### skills_dir

Directory path within the backend to scan for skill files (default `Some(".skills")`). Set to `None` to disable skill scanning even when `enable_skills` is true.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.skills_dir = Some("my-skills".into());
```

### memory_file

Path to the persistent memory file within the backend (default `Some("AGENTS.md")`). See the [Memory](memory.md) page for details.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("docs/MEMORY.md".into());
```

### subagents

Custom subagent definitions for the task tool. Each `SubAgentDef` describes a specialized subagent that can be spawned.

```rust,ignore
use synaptic::deep::SubAgentDef;

let mut options = DeepAgentOptions::new(backend.clone());
options.subagents = vec![
    SubAgentDef {
        name: "researcher".into(),
        description: "Searches the web for information".into(),
        // ...
    },
];
```

### enable_subagents

Toggle the `task` tool for child agent spawning (default `true`). When `false`, the SubAgentMiddleware and its task tool are not added.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_subagents = false;
```

### enable_filesystem

Toggle the built-in filesystem tools and `FilesystemMiddleware` (default `true`). When `false`, no filesystem tools are registered.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_filesystem = false;
```

### enable_skills

Toggle the `SkillsMiddleware` for progressive skill disclosure (default `true`).

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_skills = false;
```

### enable_memory

Toggle the `DeepMemoryMiddleware` for persistent memory (default `true`). See the [Memory](memory.md) page for details.

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = false;
```

## Middleware Stack

`create_deep_agent` assembles the middleware stack in a fixed order. Each layer can be individually enabled or disabled:

| Order | Middleware | Controlled by |
|-------|-----------|---------------|
| 1 | `DeepMemoryMiddleware` | `enable_memory` |
| 2 | `SkillsMiddleware` | `enable_skills` |
| 3 | `FilesystemMiddleware` + filesystem tools | `enable_filesystem` |
| 4 | SubAgentMiddleware's `task` tool | `enable_subagents` |
| 5 | `DeepSummarizationMiddleware` | always added |
| 6 | `PatchToolCallsMiddleware` | always added |
| 7 | User-provided middleware | `middleware` field |

The `DeepSummarizationMiddleware` and `PatchToolCallsMiddleware` are always present regardless of configuration.

## Return Type

`create_deep_agent` returns `Result<CompiledGraph<MessageState>, SynapticError>`. The resulting graph is used like any other Synaptic graph:

```rust,ignore
use synaptic::core::Message;
use synaptic::graph::MessageState;

let agent = create_deep_agent(model, options)?;
let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("Refactor the error handling in src/lib.rs"),
])).await?;
```

## Full Example

```rust,ignore
use std::sync::Arc;
use synaptic::core::Message;
use synaptic::deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let backend = Arc::new(FilesystemBackend::new("/home/user/project"));

let mut options = DeepAgentOptions::new(backend);
options.system_prompt = Some("You are a senior Rust engineer.".into());
options.summarization_threshold = 0.70;
options.enable_subagents = true;
options.max_subagent_depth = 2;

let agent = create_deep_agent(model, options)?;
let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("Refactor the error handling in src/lib.rs"),
])).await?;
```
