# Multi-Server Client

`MultiServerMcpClient` connects to multiple MCP servers simultaneously and aggregates all discovered tools into a single collection.

## Why Multiple Servers?

Real-world agents often need tools from several sources: a filesystem server for local files, a web search server for internet queries, and a database server for structured data. `MultiServerMcpClient` lets you configure all of them in one place and get back a unified `Vec<Arc<dyn Tool>>`.

## Configuration

Pass a `HashMap<String, McpConnection>` where keys are server names and values are connection configs. You can mix transports freely:

```rust,ignore
use std::collections::HashMap;
use synaptic_mcp::{
    MultiServerMcpClient, McpConnection,
    StdioConnection, HttpConnection, SseConnection,
};

let mut servers = HashMap::new();

// Local filesystem server via stdio
servers.insert(
    "fs".to_string(),
    McpConnection::Stdio(StdioConnection {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@mcp/server-filesystem".to_string()],
        env: HashMap::new(),
    }),
);

// Remote search server via HTTP
servers.insert(
    "search".to_string(),
    McpConnection::Http(HttpConnection {
        url: "https://search.example.com/mcp".to_string(),
        headers: HashMap::from([
            ("Authorization".to_string(), "Bearer token".to_string()),
        ]),
    }),
);

// Analytics server via SSE
servers.insert(
    "analytics".to_string(),
    McpConnection::Sse(SseConnection {
        url: "http://localhost:8080/mcp".to_string(),
        headers: HashMap::new(),
    }),
);
```

## Connecting and Using Tools

```rust,ignore
let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;

// Tools from all three servers are combined:
// fs_read_file, fs_write_file, search_web_search, analytics_query, ...

// Pass directly to an agent
let agent = create_react_agent(model, tools)?;
```

## Tool Name Prefixing

By default, every tool name is prefixed with its server name to prevent collisions. For example, a tool named `read_file` from the `"fs"` server becomes `fs_read_file`.

To disable prefixing (when you know tool names are globally unique):

```rust,ignore
let client = MultiServerMcpClient::new(servers).with_prefix(false);
```

## load_mcp_tools Shorthand

The `load_mcp_tools` convenience function combines `connect()` and `get_tools()`:

```rust,ignore
use synaptic_mcp::load_mcp_tools;

let client = MultiServerMcpClient::new(servers);
let tools = load_mcp_tools(&client).await?;
```

## Notes

- `connect()` iterates over all servers sequentially. If any server fails, the entire call returns an error.
- Tools are stored in an `Arc<RwLock<Vec<...>>>` internally, so `get_tools()` is safe to call from multiple tasks.
- The server name is used only for prefixing tool names -- it does not need to match any value on the server side.
