# Memory

A Deep Agent can persist learned context across sessions by reading and writing a memory file (default `AGENTS.md`) in the workspace. This gives the agent a form of long-term memory that survives restarts.

## How It Works

The `DeepMemoryMiddleware` implements `AgentMiddleware`. On every model call, its `before_model()` hook reads the configured memory file from the backend. If the file exists and is not empty, its contents are wrapped in `<agent_memory>` tags and appended to the system prompt:

```text
<agent_memory>
- The user prefers tabs over spaces.
- The project uses `thiserror 2.0` for error types.
- Always run `cargo fmt` after editing Rust files.
</agent_memory>
```

If the file does not exist or is empty, the middleware silently skips injection. The agent sees this context before processing each message, so it can apply learned preferences immediately.

## Writing to Memory

The agent can update its memory at any time by writing to the memory file using the built-in filesystem tools (e.g., `write_file` or `edit_file`). A typical pattern is for the agent to append a new line when it learns something important:

```text
Agent reasoning: "The user corrected me -- they want snake_case, not camelCase.
I should remember this for future sessions."

Tool call: edit_file({
  "path": "AGENTS.md",
  "old_string": "- Always run `cargo fmt` after editing Rust files.",
  "new_string": "- Always run `cargo fmt` after editing Rust files.\n- Use snake_case for all function names."
})
```

Because the middleware re-reads the file on every model call, updates take effect on the very next turn.

## Configuration

Memory is controlled by two fields on `DeepAgentOptions`:

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("AGENTS.md".to_string()); // default
options.enable_memory = true;                         // default

let agent = create_deep_agent(model, options)?;
```

- **`memory_file`** (`Option<String>`, default `Some("AGENTS.md")`) -- path to the memory file within the backend. You can point this at a different file if you prefer:

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("docs/MEMORY.md".to_string());
```

- **`enable_memory`** (`bool`, default `true`) -- when `true`, the `DeepMemoryMiddleware` is added to the middleware stack.

## Disabling Memory

To run without persistent memory, set `enable_memory` to `false`:

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = false;

let agent = create_deep_agent(model, options)?;
```

The `DeepMemoryMiddleware` is not added to the stack at all, so there is no overhead.

## DeepMemoryMiddleware Internals

The middleware struct is straightforward:

```rust,ignore
pub struct DeepMemoryMiddleware {
    backend: Arc<dyn Backend>,
    memory_file: String,
}

impl DeepMemoryMiddleware {
    pub fn new(backend: Arc<dyn Backend>, memory_file: String) -> Self;
}
```

It implements `AgentMiddleware` with a single hook:

- **`before_model()`** -- reads the memory file from the backend. If the content is non-empty, wraps it in `<agent_memory>` tags and appends to the system prompt. If the file is missing or empty, does nothing.

## Middleware Stack Position

`DeepMemoryMiddleware` runs first in the middleware stack (position 1 of 7), ensuring that memory context is available to all subsequent middleware and to the model itself. See the [Customization](customization.md) page for the full assembly order.
