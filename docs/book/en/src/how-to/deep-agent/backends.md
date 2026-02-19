# Backends

A Deep Agent backend controls how filesystem tools interact with the outside world. Synaptic provides three built-in backends. You choose the one that matches your deployment context.

## StateBackend

An entirely in-memory backend. Files are stored in a `HashMap<String, String>` keyed by normalized paths and never touch the real filesystem. Directories are inferred from path prefixes rather than stored as explicit entries. This is the default for tests and sandboxed demos.

```rust,ignore
use synaptic_deep::backend::StateBackend;
use std::sync::Arc;

let backend = Arc::new(StateBackend::new());

let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

// After the agent runs, inspect the virtual filesystem:
let entries = backend.ls("/").await?;
let content = backend.read_file("/hello.txt", 0, 2000).await?;
```

`StateBackend` does not support shell command execution -- `supports_execution()` returns `false` and `execute()` returns an error.

**When to use:** Unit tests, CI pipelines, sandboxed playgrounds where no real I/O should occur.

## StoreBackend

Persists files through Synaptic's `Store` trait. Each file is stored as an item with `key=path` and `value={"content": "..."}`. All items share a configurable namespace prefix. This lets you back the agent's workspace with any store implementation -- `InMemoryStore` for development, or a custom database-backed store for production.

```rust,ignore
use synaptic_deep::backend::StoreBackend;
use synaptic_store::InMemoryStore;
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let namespace = vec!["workspace".to_string(), "agent1".to_string()];
let backend = Arc::new(StoreBackend::new(store, namespace));

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;
```

The second argument is a `Vec<String>` namespace. All file keys are stored under this namespace, so multiple agents can share a single store without key collisions.

`StoreBackend` does not support shell command execution -- `supports_execution()` returns `false` and `execute()` returns an error.

**When to use:** Server deployments where you want persistence without granting direct filesystem access. Ideal for multi-tenant applications.

## FilesystemBackend

Reads and writes real files on the host operating system. This is the backend you want for coding assistants and local automation.

```rust,ignore
use synaptic_deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/home/user/project"));

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;
```

The path you provide becomes the agent's root directory. All tool paths are resolved relative to this root. The agent cannot escape the root directory -- paths containing `..` are rejected.

`FilesystemBackend` is the only built-in backend that supports shell command execution. Commands run via `sh -c` in the root directory with an optional timeout. When this backend is used, `create_filesystem_tools` automatically includes the `execute` tool.

> **Feature gate:** `FilesystemBackend` requires the `filesystem` Cargo feature:
>
> ```toml
> synaptic-deep = { path = "../crates/synaptic-deep", features = ["filesystem"] }
> ```

**When to use:** Local CLI tools, coding assistants, any scenario where the agent must interact with real files.

## Implementing a Custom Backend

All three backends implement the `Backend` trait from `synaptic_deep::backend`:

```rust,ignore
use synaptic_deep::backend::{Backend, DirEntry, ExecResult, GrepOutputMode};

#[async_trait]
pub trait Backend: Send + Sync {
    /// List entries in a directory.
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError>;

    /// Read file contents with line-based pagination.
    async fn read_file(&self, path: &str, offset: usize, limit: usize)
        -> Result<String, SynapticError>;

    /// Create or overwrite a file.
    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError>;

    /// Find-and-replace text in a file.
    async fn edit_file(&self, path: &str, old_text: &str, new_text: &str, replace_all: bool)
        -> Result<(), SynapticError>;

    /// Match file paths against a glob pattern within a base directory.
    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError>;

    /// Search file contents by regex pattern.
    async fn grep(&self, pattern: &str, path: Option<&str>, file_glob: Option<&str>,
        output_mode: GrepOutputMode) -> Result<String, SynapticError>;

    /// Execute a shell command. Returns error by default.
    async fn execute(&self, command: &str, timeout: Option<Duration>)
        -> Result<ExecResult, SynapticError> { /* default: error */ }

    /// Whether this backend supports shell command execution.
    fn supports_execution(&self) -> bool { false }
}
```

Supporting types:

- `DirEntry` -- `{ name: String, is_dir: bool, size: Option<u64> }`
- `ExecResult` -- `{ stdout: String, stderr: String, exit_code: i32 }`
- `GrepMatch` -- `{ file: String, line_number: usize, line: String }`
- `GrepOutputMode` -- `FilesWithMatches | Content | Count`

Implement this trait to back the agent with S3, a database, a remote server over SSH, or any other storage layer. Override `execute` and `supports_execution` if you want to enable the `execute` tool for your backend.

## Offline Testing

Use `StateBackend` with `ScriptedChatModel` to test deep agents without API keys or real filesystem access:

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, Message, ToolCall};
use synaptic_models::ScriptedChatModel;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_deep::backend::StateBackend;

// Script the model to write a file then finish
let model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "I'll create a file.",
            vec![ToolCall {
                id: "call_1".into(),
                name: "write_file".into(),
                arguments: r#"{"path": "/hello.txt", "content": "Hello from test!"}"#.into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("Done! I created hello.txt."),
        usage: None,
    },
]));

let backend = Arc::new(StateBackend::new());
let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

// Run the agent...
// Then inspect the virtual filesystem:
let content = backend.read_file("/hello.txt", 0, 2000).await?;
assert!(content.contains("Hello from test!"));
```

This pattern is ideal for CI pipelines and unit tests. The `StateBackend` is fully deterministic and requires no cleanup.

## Comparison

| Backend | Persistence | Real I/O | Execution | Feature gate | Best for |
|---------|-------------|----------|-----------|--------------|----------|
| `StateBackend` | None (in-memory) | No | No | None | Tests, sandboxing |
| `StoreBackend` | Via Store trait | No | No | None | Servers, multi-tenant |
| `FilesystemBackend` | Disk | Yes | Yes | `filesystem` | Local CLI, coding assistants |
