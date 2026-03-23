//! RPC handlers for tool catalog.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// tools.catalog
// ---------------------------------------------------------------------------

pub async fn handle_catalog(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut groups = Vec::new();

    // 1. Filesystem tools
    groups.push(json!({
        "id": "filesystem",
        "label": "Filesystem",
        "tools": [
            { "name": "ls", "description": "List directory contents", "source": "filesystem" },
            { "name": "read_file", "description": "Read file contents with optional line-based pagination", "source": "filesystem" },
            { "name": "write_file", "description": "Create or overwrite a file with the given content", "source": "filesystem" },
            { "name": "edit_file", "description": "Find and replace text in a file", "source": "filesystem" },
            { "name": "glob", "description": "Find files matching a glob pattern", "source": "filesystem" },
            { "name": "grep", "description": "Search file contents by regex pattern", "source": "filesystem" },
            { "name": "execute", "description": "Execute a shell command", "source": "filesystem" },
        ],
    }));

    // 2. Core tools
    groups.push(json!({
        "id": "core",
        "label": "Core",
        "tools": [
            { "name": "apply_patch", "description": "Apply a unified diff patch to a file", "source": "core" },
            { "name": "read_pdf", "description": "Read and extract text from a PDF file", "source": "core" },
            { "name": "firecrawl", "description": "Crawl and extract content from web pages", "source": "core" },
            { "name": "analyze_image", "description": "Analyze image content using a vision model", "source": "core" },
        ],
    }));

    // 3. Agent tools
    groups.push(json!({
        "id": "agent",
        "label": "Agent",
        "tools": [
            { "name": "Skill", "description": "Execute a skill by name with arguments", "source": "agent" },
            { "name": "task", "description": "Spawn a sub-agent to handle a delegated task", "source": "agent" },
            { "name": "TaskOutput", "description": "Retrieve output from a background task", "source": "agent" },
            { "name": "llm_task", "description": "Lightweight LLM delegation for simple queries", "source": "agent" },
        ],
    }));

    // 4. Memory tools
    if ctx.state.core.config.memory.ltm_enabled {
        groups.push(json!({
            "id": "memory",
            "label": "Memory",
            "tools": [
                { "name": "memory_search", "description": "Search long-term memory by semantic query", "source": "memory" },
                { "name": "memory_get", "description": "Retrieve a specific memory entry by key", "source": "memory" },
            ],
        }));
    }

    // 5. Session tools
    groups.push(json!({
        "id": "session",
        "label": "Session",
        "tools": [
            { "name": "sessions_list", "description": "List active sessions", "source": "session" },
            { "name": "sessions_history", "description": "Get message history for a session", "source": "session" },
            { "name": "sessions_send", "description": "Send a message to another session", "source": "session" },
            { "name": "sessions_spawn", "description": "Spawn a new session with a prompt", "source": "session" },
        ],
    }));

    // 6. MCP tools
    if let Some(ref mcp_servers) = ctx.state.core.config.base.mcp {
        let mcp_tools: Vec<Value> = mcp_servers
            .iter()
            .map(|server| {
                let desc = server
                    .command
                    .as_deref()
                    .or(server.url.as_deref())
                    .unwrap_or("MCP server");
                json!({
                    "name": server.name,
                    "description": format!("MCP: {}", desc),
                    "source": "mcp",
                })
            })
            .collect();
        if !mcp_tools.is_empty() {
            groups.push(json!({
                "id": "mcp",
                "label": "MCP Servers",
                "tools": mcp_tools,
            }));
        }
    }

    Ok(json!(groups))
}
