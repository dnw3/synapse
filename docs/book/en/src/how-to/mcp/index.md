# MCP (Model Context Protocol)

The `synaptic_mcp` crate connects to external MCP-compatible tool servers, discovers their tools, and exposes them as standard `synaptic_core::Tool` implementations.

## What is MCP?

The Model Context Protocol is an open standard for connecting AI models to external tool servers. An MCP server advertises a set of tools via a JSON-RPC interface. Synaptic's MCP client discovers those tools at connection time and wraps each one as a native `Tool` that can be used with any agent, graph, or tool executor.

## Supported Transports

| Transport | Config Struct | Communication |
|-----------|--------------|---------------|
| **Stdio** | `StdioConnection` | Spawn a child process; JSON-RPC over stdin/stdout |
| **SSE** | `SseConnection` | HTTP POST with Server-Sent Events for streaming |
| **HTTP** | `HttpConnection` | Standard HTTP POST with JSON-RPC |

All transports use the same JSON-RPC `tools/list` method for discovery and `tools/call` method for invocation.

## Quick Start

```rust,ignore
use std::collections::HashMap;
use synaptic_mcp::{MultiServerMcpClient, McpConnection, StdioConnection};

// Configure a single MCP server
let mut servers = HashMap::new();
servers.insert(
    "my_server".to_string(),
    McpConnection::Stdio(StdioConnection {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@my/mcp-server".to_string()],
        env: HashMap::new(),
    }),
);

// Connect and discover tools
let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;

// Use discovered tools with an agent
let agent = create_react_agent(model, tools)?;
```

## Tool Name Prefixing

By default, discovered tool names are prefixed with the server name to avoid collisions (e.g., `my_server_search`). Disable this with:

```rust,ignore
let client = MultiServerMcpClient::new(servers).with_prefix(false);
```

## Convenience Function

The `load_mcp_tools` function combines `connect()` and `get_tools()` in a single call:

```rust,ignore
use synaptic_mcp::load_mcp_tools;

let tools = load_mcp_tools(&client).await?;
```

## Crate Imports

```rust,ignore
use synaptic_mcp::{
    MultiServerMcpClient,
    McpConnection,
    StdioConnection,
    SseConnection,
    HttpConnection,
    load_mcp_tools,
};
```

See the individual transport pages for detailed configuration examples.
