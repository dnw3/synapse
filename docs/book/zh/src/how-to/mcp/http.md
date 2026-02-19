# HTTP 传输

HTTP 传输使用标准的 HTTP POST 请求和 JSON-RPC 载荷连接到 MCP 服务器。

## 配置

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

### 字段

| 字段 | 类型 | 描述 |
|------|------|------|
| `url` | `String` | MCP 服务器端点 URL |
| `headers` | `HashMap<String, String>` | 额外的 HTTP 头（例如 API 密钥） |

## 工作原理

工具发现（`tools/list`）和工具调用（`tools/call`）都向配置的 URL 发送 JSON-RPC POST 请求。`Content-Type: application/json` 头会自动添加。配置中的自定义头包含在每个请求中。

## 完整示例

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

## 注意事项

- HTTP 和 SSE 传输对工具调用使用相同的请求/响应处理方式。区别在于 MCP 服务器管理连接的方式。
- 在生产环境中使用 HTTPS 以保护 API 密钥和工具调用载荷。
- `headers` 映射应用于每个请求，适合用于静态认证令牌。
