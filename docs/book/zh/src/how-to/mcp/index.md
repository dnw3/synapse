# MCP（Model Context Protocol）

`synaptic_mcp` crate 连接到外部兼容 MCP 的工具服务器，发现其工具，并将它们作为标准的 `synaptic_core::Tool` 实现公开。

## 什么是 MCP？

Model Context Protocol 是一个用于将 AI 模型连接到外部工具服务器的开放标准。MCP 服务器通过 JSON-RPC 接口发布一组工具。Synaptic 的 MCP 客户端在连接时发现这些工具，并将每个工具包装为原生的 `Tool`，可以与任何 Agent、图或工具执行器一起使用。

## 支持的传输方式

| 传输方式 | 配置结构体 | 通信方式 |
|----------|------------|----------|
| **Stdio** | `StdioConnection` | 启动子进程；通过 stdin/stdout 进行 JSON-RPC 通信 |
| **SSE** | `SseConnection` | 使用 Server-Sent Events 进行流式传输的 HTTP POST |
| **HTTP** | `HttpConnection` | 使用 JSON-RPC 的标准 HTTP POST |

所有传输方式使用相同的 JSON-RPC `tools/list` 方法进行发现，使用 `tools/call` 方法进行调用。

## 快速开始

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

## 工具名称前缀

默认情况下，发现的工具名称会以服务器名称作为前缀，以避免冲突（例如 `my_server_search`）。通过以下方式禁用：

```rust,ignore
let client = MultiServerMcpClient::new(servers).with_prefix(false);
```

## 便捷函数

`load_mcp_tools` 函数将 `connect()` 和 `get_tools()` 合并为一次调用：

```rust,ignore
use synaptic_mcp::load_mcp_tools;

let tools = load_mcp_tools(&client).await?;
```

## Crate 导入

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

有关详细的配置示例，请参阅各传输方式的页面。
