//! MCP (Model Context Protocol) adapters for connecting to external tool servers.
//!
//! This crate provides a [`MultiServerMcpClient`] that can connect to one or more
//! MCP-compatible servers over Stdio, SSE, or HTTP transports, discover their
//! advertised tools, and expose each tool as a [`synaptic_core::Tool`] implementor.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use synaptic_core::{SynapticError, Tool};

// ---------------------------------------------------------------------------
// Connection types
// ---------------------------------------------------------------------------

/// Stdio transport connection config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdioConnection {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// SSE (Server-Sent Events) transport connection config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseConnection {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// HTTP (Streamable HTTP) transport connection config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConnection {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// MCP server connection type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpConnection {
    Stdio(StdioConnection),
    Sse(SseConnection),
    Http(HttpConnection),
}

// ---------------------------------------------------------------------------
// McpTool
// ---------------------------------------------------------------------------

/// A tool loaded from an MCP server.
struct McpTool {
    tool_name: &'static str,
    tool_description: &'static str,
    tool_parameters: Value,
    #[allow(dead_code)]
    server_name: String,
    connection: McpConnection,
    client: reqwest::Client,
}

/// Leak a `String` into a `&'static str`.
///
/// MCP tool definitions live for the entire program lifetime, so this
/// small, bounded leak is acceptable and avoids lifetime gymnastics.
fn leak_string(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &'static str {
        self.tool_name
    }

    fn description(&self) -> &'static str {
        self.tool_description
    }

    fn parameters(&self) -> Option<Value> {
        Some(self.tool_parameters.clone())
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        match &self.connection {
            McpConnection::Http(conn) => {
                call_http(
                    &self.client,
                    &conn.url,
                    &conn.headers,
                    self.tool_name,
                    &args,
                )
                .await
            }
            McpConnection::Sse(conn) => {
                // SSE uses the same HTTP POST for tool calls.
                call_http(
                    &self.client,
                    &conn.url,
                    &conn.headers,
                    self.tool_name,
                    &args,
                )
                .await
            }
            McpConnection::Stdio(conn) => call_stdio(conn, self.tool_name, &args).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Transport helpers
// ---------------------------------------------------------------------------

/// Issue a JSON-RPC `tools/call` over HTTP(S) and return the `result` field.
async fn call_http(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
    tool_name: &str,
    args: &Value,
) -> Result<Value, SynapticError> {
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": args,
        },
        "id": 1
    });

    let mut builder = client.post(url);
    for (key, value) in headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    builder = builder.header("Content-Type", "application/json");

    let resp = builder
        .json(&request_body)
        .send()
        .await
        .map_err(|e| SynapticError::Mcp(format!("HTTP request failed: {}", e)))?;

    let body: Value = resp
        .json()
        .await
        .map_err(|e| SynapticError::Mcp(format!("Failed to parse response: {}", e)))?;

    if let Some(error) = body.get("error") {
        return Err(SynapticError::Mcp(format!("MCP error: {}", error)));
    }

    body.get("result")
        .cloned()
        .ok_or_else(|| SynapticError::Mcp("No result in MCP response".to_string()))
}

/// Spawn a child process, send a JSON-RPC `tools/call` over stdin, and read
/// the response from stdout.
async fn call_stdio(
    conn: &StdioConnection,
    tool_name: &str,
    args: &Value,
) -> Result<Value, SynapticError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": args,
        },
        "id": 1
    });

    let mut child = Command::new(&conn.command)
        .args(&conn.args)
        .envs(&conn.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| SynapticError::Mcp(format!("Failed to spawn process: {}", e)))?;

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| SynapticError::Mcp("Failed to open stdin".to_string()))?;

    let msg =
        serde_json::to_string(&request_body).map_err(|e| SynapticError::Mcp(e.to_string()))?;

    stdin
        .write_all(msg.as_bytes())
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    // Drop stdin so the child can see EOF if it needs to.
    drop(child.stdin.take());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SynapticError::Mcp("Failed to open stdout".to_string()))?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    let body: Value = serde_json::from_str(&line)
        .map_err(|e| SynapticError::Mcp(format!("Failed to parse response: {}", e)))?;

    let _ = child.kill().await;

    if let Some(error) = body.get("error") {
        return Err(SynapticError::Mcp(format!("MCP error: {}", error)));
    }

    body.get("result")
        .cloned()
        .ok_or_else(|| SynapticError::Mcp("No result in MCP response".to_string()))
}

/// Issue a JSON-RPC `tools/list` over HTTP(S) and return the array of tool
/// definitions from `result.tools`.
async fn list_tools_http(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
) -> Result<Value, SynapticError> {
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {},
        "id": 1
    });

    let mut builder = client.post(url);
    for (key, value) in headers {
        builder = builder.header(key.as_str(), value.as_str());
    }
    builder = builder.header("Content-Type", "application/json");

    let resp = builder
        .json(&request_body)
        .send()
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    let body: Value = resp
        .json()
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    Ok(body
        .get("result")
        .and_then(|r| r.get("tools"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

/// Spawn a child process, send a JSON-RPC `tools/list` over stdin, and read
/// the response from stdout to discover available tools.
async fn list_tools_stdio(conn: &StdioConnection) -> Result<Value, SynapticError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "params": {},
        "id": 1
    });

    let mut child = Command::new(&conn.command)
        .args(&conn.args)
        .envs(&conn.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| SynapticError::Mcp(format!("Failed to spawn process: {}", e)))?;

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| SynapticError::Mcp("Failed to open stdin".to_string()))?;

    let msg =
        serde_json::to_string(&request_body).map_err(|e| SynapticError::Mcp(e.to_string()))?;

    stdin
        .write_all(msg.as_bytes())
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    // Drop stdin so the child can see EOF if it needs to.
    drop(child.stdin.take());

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SynapticError::Mcp("Failed to open stdout".to_string()))?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| SynapticError::Mcp(e.to_string()))?;

    let body: Value = serde_json::from_str(&line)
        .map_err(|e| SynapticError::Mcp(format!("Failed to parse response: {}", e)))?;

    let _ = child.kill().await;

    if let Some(error) = body.get("error") {
        return Err(SynapticError::Mcp(format!("MCP error: {}", error)));
    }

    Ok(body
        .get("result")
        .and_then(|r| r.get("tools"))
        .cloned()
        .unwrap_or(Value::Array(vec![])))
}

// ---------------------------------------------------------------------------
// MultiServerMcpClient
// ---------------------------------------------------------------------------

/// Client that connects to one or more MCP servers and aggregates their tools.
pub struct MultiServerMcpClient {
    servers: HashMap<String, McpConnection>,
    prefix_tool_names: bool,
    tools: Arc<RwLock<Vec<Arc<dyn Tool>>>>,
}

impl MultiServerMcpClient {
    /// Create a new client with the given server map.
    pub fn new(servers: HashMap<String, McpConnection>) -> Self {
        Self {
            servers,
            prefix_tool_names: true,
            tools: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// When `true` (the default), discovered tool names are prefixed with the
    /// server name (e.g. `"myserver_tool_name"`).
    pub fn with_prefix(mut self, prefix: bool) -> Self {
        self.prefix_tool_names = prefix;
        self
    }

    /// Connect to all servers and discover available tools.
    pub async fn connect(&self) -> Result<(), SynapticError> {
        let client = reqwest::Client::new();
        let mut all_tools = Vec::new();

        for (server_name, connection) in &self.servers {
            let tools = self
                .discover_tools(server_name, connection, &client)
                .await?;
            all_tools.extend(tools);
        }

        *self.tools.write().await = all_tools;
        Ok(())
    }

    /// Discover tools from a single MCP server.
    async fn discover_tools(
        &self,
        server_name: &str,
        connection: &McpConnection,
        client: &reqwest::Client,
    ) -> Result<Vec<Arc<dyn Tool>>, SynapticError> {
        let tools_list = match connection {
            McpConnection::Http(conn) => list_tools_http(client, &conn.url, &conn.headers).await?,
            McpConnection::Sse(conn) => list_tools_http(client, &conn.url, &conn.headers).await?,
            McpConnection::Stdio(conn) => list_tools_stdio(conn).await?,
        };

        let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

        if let Value::Array(tool_arr) = tools_list {
            for tool_def in tool_arr {
                let name = tool_def
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = tool_def
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let parameters = tool_def
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or(serde_json::json!({"type": "object"}));

                let tool_name = if self.prefix_tool_names {
                    format!("{}_{}", server_name, name)
                } else {
                    name
                };

                tools.push(Arc::new(McpTool {
                    tool_name: leak_string(tool_name),
                    tool_description: leak_string(description),
                    tool_parameters: parameters,
                    server_name: server_name.to_string(),
                    connection: connection.clone(),
                    client: client.clone(),
                }));
            }
        }

        Ok(tools)
    }

    /// Get all discovered tools.
    pub async fn get_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.read().await.clone()
    }
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

/// Convenience function to connect to all servers and return the discovered
/// tools in a single call.
pub async fn load_mcp_tools(
    client: &MultiServerMcpClient,
) -> Result<Vec<Arc<dyn Tool>>, SynapticError> {
    client.connect().await?;
    Ok(client.get_tools().await)
}
