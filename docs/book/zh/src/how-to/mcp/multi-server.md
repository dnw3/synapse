# 多服务器客户端

`MultiServerMcpClient` 同时连接到多个 MCP 服务器，并将所有发现的工具聚合到一个集合中。

## 为什么需要多个服务器？

实际应用中的 Agent 通常需要来自多个来源的工具：用于本地文件的文件系统服务器、用于互联网查询的网络搜索服务器，以及用于结构化数据的数据库服务器。`MultiServerMcpClient` 允许你在一个地方配置所有服务器，并获得一个统一的 `Vec<Arc<dyn Tool>>`。

## 配置

传入一个 `HashMap<String, McpConnection>`，其中键是服务器名称，值是连接配置。你可以自由混合不同的传输方式：

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

## 连接和使用工具

```rust,ignore
let client = MultiServerMcpClient::new(servers);
client.connect().await?;
let tools = client.get_tools().await;

// Tools from all three servers are combined:
// fs_read_file, fs_write_file, search_web_search, analytics_query, ...

// Pass directly to an agent
let agent = create_react_agent(model, tools)?;
```

## 工具名称前缀

默认情况下，每个工具名称都会以其服务器名称作为前缀，以防止冲突。例如，来自 `"fs"` 服务器的名为 `read_file` 的工具会变成 `fs_read_file`。

要禁用前缀（当你确定工具名称全局唯一时）：

```rust,ignore
let client = MultiServerMcpClient::new(servers).with_prefix(false);
```

## load_mcp_tools 简写

`load_mcp_tools` 便捷函数将 `connect()` 和 `get_tools()` 合并：

```rust,ignore
use synaptic_mcp::load_mcp_tools;

let client = MultiServerMcpClient::new(servers);
let tools = load_mcp_tools(&client).await?;
```

## 注意事项

- `connect()` 按顺序遍历所有服务器。如果任何服务器失败，整个调用将返回错误。
- 工具在内部存储在 `Arc<RwLock<Vec<...>>>` 中，因此 `get_tools()` 可以安全地从多个任务中调用。
- 服务器名称仅用于工具名称的前缀 -- 它不需要与服务器端的任何值匹配。
