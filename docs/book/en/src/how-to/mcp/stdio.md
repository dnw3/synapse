# Stdio Transport

The Stdio transport spawns a child process and communicates with it over stdin/stdout using JSON-RPC.

## Configuration

```rust,ignore
use synaptic_mcp::StdioConnection;
use std::collections::HashMap;

let connection = StdioConnection {
    command: "npx".to_string(),
    args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
    env: HashMap::from([
        ("HOME".to_string(), "/home/user".to_string()),
    ]),
};
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `command` | `String` | The executable to spawn |
| `args` | `Vec<String>` | Command-line arguments |
| `env` | `HashMap<String, String>` | Additional environment variables (empty by default) |

## How It Works

1. **Discovery** (`tools/list`): Synaptic spawns the process, writes a JSON-RPC `tools/list` request to stdin, reads the response from stdout, then kills the process.
2. **Invocation** (`tools/call`): For each tool call, Synaptic spawns a fresh process, writes a JSON-RPC `tools/call` request, reads the response, and kills the process.

## Full Example

```rust,ignore
use std::collections::HashMap;
use std::sync::Arc;
use synaptic_mcp::{MultiServerMcpClient, McpConnection, StdioConnection};
use synaptic_graph::create_react_agent;

// Configure an MCP server that provides filesystem tools
let mut servers = HashMap::new();
servers.insert(
    "filesystem".to_string(),
    McpConnection::Stdio(StdioConnection {
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
            "/allowed/path".to_string(),
        ],
        env: HashMap::new(),
    }),
);

// Connect and discover tools
let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;
// tools might include: filesystem_read_file, filesystem_write_file, etc.

// Wire into an agent
let agent = create_react_agent(model, tools)?;
```

## Testing Without a Server

For unit tests, you can test MCP client types without spawning a real server. The connection types are serializable and the client can be inspected before connecting:

```rust,ignore
use std::collections::HashMap;
use synaptic_mcp::{MultiServerMcpClient, McpConnection, StdioConnection};

// Create a client without connecting
let mut servers = HashMap::new();
servers.insert(
    "test".to_string(),
    McpConnection::Stdio(StdioConnection {
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        env: HashMap::new(),
    }),
);

let client = MultiServerMcpClient::new(servers);

// Before connect(), no tools are available
let tools = client.get_tools().await;
assert!(tools.is_empty());

// Connection types round-trip through serde
let json = serde_json::to_string(&McpConnection::Stdio(StdioConnection {
    command: "npx".to_string(),
    args: vec![],
    env: HashMap::new(),
}))?;
let _: McpConnection = serde_json::from_str(&json)?;
```

For integration tests that need actual tool discovery, use a simple echo script as the MCP server command.

## Notes

- Each tool call spawns a new process. This is simple but adds latency for each invocation.
- Ensure the command is available on `PATH` or provide an absolute path.
- The `env` map is merged with the current process environment -- it does not replace it.
- Stderr from the child process is discarded (`Stdio::null()`).
