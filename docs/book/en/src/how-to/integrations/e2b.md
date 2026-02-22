# E2B Sandbox

[E2B](https://e2b.dev/) provides cloud-based code execution sandboxes. The `E2BSandboxTool` allows LLM agents to safely execute code — each call creates an isolated sandbox, runs the code, and destroys the environment.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["e2b"] }
```

Get an API key from [e2b.dev](https://e2b.dev/).

## Usage

```rust,ignore
use synaptic::e2b::{E2BConfig, E2BSandboxTool};
use synaptic::core::Tool;
use serde_json::json;

let config = E2BConfig::new("your-api-key")
    .with_template("python")
    .with_timeout(30);
let tool = E2BSandboxTool::new(config);

let result = tool.call(json!({
    "code": "print(sum(range(1, 101)))",
    "language": "python"
})).await?;
// {"stdout": "5050\n", "stderr": "", "exit_code": 0}
```

## Configuration

| Field | Default | Description |
|---|---|---|
| `template` | `"base"` | Sandbox template (`"base"`, `"python"`, `"nodejs"`) |
| `timeout_secs` | `30` | Execution timeout in seconds |

## Supported Languages

| Language | Value |
|---|---|
| Python | `"python"` |
| JavaScript | `"javascript"` |
| Bash | `"bash"` |

## Notes

- Each tool call creates a fresh sandbox and destroys it after execution, ensuring isolation.
- The sandbox is always destroyed even if code execution fails.
- Network access inside the sandbox depends on the E2B template configuration.
