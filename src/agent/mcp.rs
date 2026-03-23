use std::collections::HashMap;
use std::sync::Arc;

use synaptic::core::Tool;
use synaptic::mcp::{
    HttpConnection, McpConnection, MultiServerMcpClient, SseConnection, StdioConnection,
};

use crate::config::SynapseConfig;

/// Convert an McpServerConfig into an McpConnection.
pub fn config_to_mcp_connection(mc: &synaptic::config::McpServerConfig) -> Option<McpConnection> {
    match mc.transport.as_str() {
        "stdio" => {
            let command = mc.command.clone()?;
            Some(McpConnection::Stdio(StdioConnection {
                command,
                args: mc.args.clone().unwrap_or_default(),
                env: mc.env.clone().unwrap_or_default(),
            }))
        }
        "sse" => {
            let url = mc.url.clone()?;
            Some(McpConnection::Sse(SseConnection {
                url,
                headers: mc.headers.clone().unwrap_or_default(),
                oauth: None,
            }))
        }
        "http" => {
            let url = mc.url.clone()?;
            Some(McpConnection::Http(HttpConnection {
                url,
                headers: mc.headers.clone().unwrap_or_default(),
                oauth: None,
            }))
        }
        _ => None,
    }
}

/// Build an MCP client from config and connect to all servers.
///
/// Returns the connected client (for shared state) and discovered tools.
/// On failure, returns a client with no tools (non-fatal).
pub async fn build_mcp_client(
    config: &SynapseConfig,
) -> (Arc<MultiServerMcpClient>, Vec<Arc<dyn Tool>>) {
    let mcp_configs = match config.mcp_configs() {
        Some(configs) if !configs.is_empty() => configs,
        _ => {
            return (
                Arc::new(MultiServerMcpClient::new(HashMap::new())),
                Vec::new(),
            )
        }
    };

    let mut servers = HashMap::new();
    for mc in mcp_configs {
        if let Some(conn) = config_to_mcp_connection(mc) {
            servers.insert(mc.name.clone(), conn);
        }
    }

    if servers.is_empty() {
        return (
            Arc::new(MultiServerMcpClient::new(HashMap::new())),
            Vec::new(),
        );
    }

    let count = servers.len();
    tracing::info!(server_count = count, "Connecting to MCP server(s)");

    let client = Arc::new(MultiServerMcpClient::new(servers));
    match synaptic::mcp::load_mcp_tools(&client).await {
        Ok(tools) => {
            tracing::info!(tool_count = tools.len(), "Loaded tools from MCP servers");
            (client, tools)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load MCP tools");
            (client, Vec::new())
        }
    }
}

/// Load MCP tools from config. Returns empty vec on failure (non-fatal).
///
/// Convenience wrapper over [`build_mcp_client`] for callers that don't
/// need the client reference (e.g., task/REPL mode).
pub async fn load_mcp_tools(config: &SynapseConfig) -> Vec<Arc<dyn Tool>> {
    let (_client, tools) = build_mcp_client(config).await;
    tools
}
