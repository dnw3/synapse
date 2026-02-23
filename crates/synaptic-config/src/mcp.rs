use serde::Deserialize;
use std::collections::HashMap;

/// Configuration for an MCP server connection.
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Server name identifier.
    pub name: String,
    /// Transport type: "stdio", "sse", or "http".
    pub transport: String,
    /// Command to launch (for stdio transport).
    pub command: Option<String>,
    /// Command arguments (for stdio transport).
    pub args: Option<Vec<String>>,
    /// URL endpoint (for sse/http transport).
    pub url: Option<String>,
    /// Additional headers (for sse/http transport).
    pub headers: Option<HashMap<String, String>>,
}
