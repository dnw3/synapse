# HTTP Transport

The HTTP transport connects to an MCP server using standard HTTP POST requests with JSON-RPC payloads.

## Configuration

```rust,ignore
use synaptic_mcp::HttpConnection;
use std::collections::HashMap;

let connection = HttpConnection {
    url: "https://mcp.example.com/rpc".to_string(),
    headers: HashMap::from([
        ("X-Api-Key".to_string(), "my-api-key".to_string()),
    ]),
};
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | The MCP server endpoint URL |
| `headers` | `HashMap<String, String>` | Additional HTTP headers (e.g., API keys) |

## How It Works

Both tool discovery (`tools/list`) and tool invocation (`tools/call`) send a JSON-RPC POST request to the configured URL. The `Content-Type: application/json` header is added automatically. Custom headers from the config are included in every request.

## Full Example

```rust,ignore
use std::collections::HashMap;
use synaptic_mcp::{MultiServerMcpClient, McpConnection, HttpConnection};

let mut servers = HashMap::new();
servers.insert(
    "calculator".to_string(),
    McpConnection::Http(HttpConnection {
        url: "https://mcp.example.com/rpc".to_string(),
        headers: HashMap::from([
            ("X-Api-Key".to_string(), "my-api-key".to_string()),
        ]),
    }),
);

let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;
// tools might include: calculator_add, calculator_multiply, etc.
```

## Notes

- HTTP and SSE transports use identical request/response handling for tool calls. The distinction is in how the MCP server manages the connection.
- Use HTTPS in production to protect API keys and tool call payloads.
- The `headers` map is applied to every request, making it suitable for static authentication tokens.
