# SSE 传输

SSE（Server-Sent Events）传输通过 HTTP 连接到远程 MCP 服务器，使用协议的 SSE 传输变体。

## 配置

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

### 字段

| 字段 | 类型 | 描述 |
|------|------|------|
| `url` | `String` | MCP 服务器端点 URL |
| `headers` | `HashMap<String, String>` | 额外的 HTTP 头（例如认证令牌） |

## 工作原理

工具发现（`tools/list`）和工具调用（`tools/call`）都使用针对配置 URL 的 HTTP POST 请求，携带 JSON-RPC 载荷。`Content-Type: application/json` 头会自动添加。

## 完整示例

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

## 注意事项

- SSE 和 HTTP 传输对工具调用共享相同的底层 HTTP POST 机制。
- `headers` 映射应用于每个请求（包括发现和调用）。
- 服务器必须在给定的 URL 上实现 MCP JSON-RPC 接口。
