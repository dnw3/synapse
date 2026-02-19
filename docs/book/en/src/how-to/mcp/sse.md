# SSE Transport

The SSE (Server-Sent Events) transport connects to a remote MCP server over HTTP, using the SSE transport variant of the protocol.

## Configuration

```rust,ignore
use synaptic_mcp::SseConnection;
use std::collections::HashMap;

let connection = SseConnection {
    url: "http://localhost:3001/mcp".to_string(),
    headers: HashMap::from([
        ("Authorization".to_string(), "Bearer my-token".to_string()),
    ]),
};
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | The MCP server endpoint URL |
| `headers` | `HashMap<String, String>` | Additional HTTP headers (e.g., auth tokens) |

## How It Works

Both tool discovery (`tools/list`) and tool invocation (`tools/call`) use HTTP POST requests with JSON-RPC payloads against the configured URL. The `Content-Type: application/json` header is added automatically.

## Full Example

```rust,ignore
use std::collections::HashMap;
use synaptic_mcp::{MultiServerMcpClient, McpConnection, SseConnection};

let mut servers = HashMap::new();
servers.insert(
    "search".to_string(),
    McpConnection::Sse(SseConnection {
        url: "http://localhost:3001/mcp".to_string(),
        headers: HashMap::from([
            ("Authorization".to_string(), "Bearer secret".to_string()),
        ]),
    }),
);

let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;
// tools might include: search_web_search, search_image_search, etc.
```

## Notes

- SSE and HTTP transports share the same underlying HTTP POST mechanism for tool calls.
- The `headers` map is applied to every request (both discovery and invocation).
- The server must implement the MCP JSON-RPC interface at the given URL.
