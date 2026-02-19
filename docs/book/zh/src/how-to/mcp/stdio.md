# Stdio 传输

Stdio 传输启动一个子进程，并通过 stdin/stdout 使用 JSON-RPC 与其通信。

## 配置

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

### 字段

| 字段 | 类型 | 描述 |
|------|------|------|
| `command` | `String` | 要启动的可执行文件 |
| `args` | `Vec<String>` | 命令行参数 |
| `env` | `HashMap<String, String>` | 额外的环境变量（默认为空） |

## 工作原理

1. **发现**（`tools/list`）：Synaptic 启动进程，向 stdin 写入 JSON-RPC `tools/list` 请求，从 stdout 读取响应，然后终止进程。
2. **调用**（`tools/call`）：对于每个工具调用，Synaptic 启动一个新进程，写入 JSON-RPC `tools/call` 请求，读取响应，然后终止进程。

## 完整示例

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

## 无服务器测试

对于单元测试，你可以在不启动真实服务器的情况下测试 MCP 客户端类型。连接类型是可序列化的，客户端在连接之前可以被检查：

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

对于需要实际工具发现的集成测试，可以使用一个简单的 echo 脚本作为 MCP 服务器命令。

## 注意事项

- 每次工具调用都会启动一个新进程。这种方式简单但会为每次调用增加延迟。
- 确保命令在 `PATH` 上可用，或者提供绝对路径。
- `env` 映射与当前进程环境合并 -- 它不会替换当前环境。
- 子进程的 stderr 被丢弃（`Stdio::null()`）。
