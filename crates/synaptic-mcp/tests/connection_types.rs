//! Tests for MCP connection type serialization and `MultiServerMcpClient` construction.

use std::collections::HashMap;

use serde_json::json;
use synaptic_mcp::{HttpConnection, McpConnection, MultiServerMcpClient, SseConnection, StdioConnection};

// ---------------------------------------------------------------------------
// Stdio connection serde
// ---------------------------------------------------------------------------

#[test]
fn stdio_connection_serde_roundtrip() {
    let conn = McpConnection::Stdio(StdioConnection {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@some/mcp-server".to_string()],
        env: HashMap::new(),
    });
    let json = serde_json::to_value(&conn).unwrap();
    assert_eq!(json["type"], "Stdio");
    assert_eq!(json["command"], "npx");

    let deserialized: McpConnection = serde_json::from_value(json).unwrap();
    match deserialized {
        McpConnection::Stdio(s) => {
            assert_eq!(s.command, "npx");
            assert_eq!(s.args.len(), 2);
            assert_eq!(s.args[0], "-y");
            assert_eq!(s.args[1], "@some/mcp-server");
        }
        _ => panic!("Expected Stdio variant"),
    }
}

#[test]
fn stdio_connection_default_env() {
    // `env` has `#[serde(default)]` so it should deserialize even when absent.
    let json = json!({
        "type": "Stdio",
        "command": "node",
        "args": ["server.js"]
    });
    let conn: McpConnection = serde_json::from_value(json).unwrap();
    match conn {
        McpConnection::Stdio(s) => {
            assert_eq!(s.command, "node");
            assert_eq!(s.args, vec!["server.js"]);
            assert!(s.env.is_empty());
        }
        _ => panic!("Expected Stdio variant"),
    }
}

#[test]
fn stdio_connection_with_env() {
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "secret123".to_string());
    let conn = McpConnection::Stdio(StdioConnection {
        command: "python".to_string(),
        args: vec!["server.py".to_string()],
        env,
    });
    let json = serde_json::to_value(&conn).unwrap();
    assert_eq!(json["env"]["API_KEY"], "secret123");

    let deserialized: McpConnection = serde_json::from_value(json).unwrap();
    match deserialized {
        McpConnection::Stdio(s) => {
            assert_eq!(s.env.get("API_KEY").unwrap(), "secret123");
        }
        _ => panic!("Expected Stdio variant"),
    }
}

// ---------------------------------------------------------------------------
// SSE connection serde
// ---------------------------------------------------------------------------

#[test]
fn sse_connection_serde_roundtrip() {
    let conn = McpConnection::Sse(SseConnection {
        url: "http://localhost:8080/sse".to_string(),
        headers: HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
    });
    let json = serde_json::to_value(&conn).unwrap();
    assert_eq!(json["type"], "Sse");
    assert_eq!(json["url"], "http://localhost:8080/sse");

    let deserialized: McpConnection = serde_json::from_value(json).unwrap();
    match deserialized {
        McpConnection::Sse(s) => {
            assert_eq!(s.url, "http://localhost:8080/sse");
            assert_eq!(s.headers.get("Authorization").unwrap(), "Bearer token");
        }
        _ => panic!("Expected Sse variant"),
    }
}

#[test]
fn sse_connection_default_headers() {
    let json = json!({
        "type": "Sse",
        "url": "http://example.com/sse"
    });
    let conn: McpConnection = serde_json::from_value(json).unwrap();
    match conn {
        McpConnection::Sse(s) => {
            assert_eq!(s.url, "http://example.com/sse");
            assert!(s.headers.is_empty());
        }
        _ => panic!("Expected Sse variant"),
    }
}

// ---------------------------------------------------------------------------
// HTTP connection serde
// ---------------------------------------------------------------------------

#[test]
fn http_connection_serde_roundtrip() {
    let conn = McpConnection::Http(HttpConnection {
        url: "http://localhost:8080/mcp".to_string(),
        headers: HashMap::new(),
    });
    let json = serde_json::to_value(&conn).unwrap();
    assert_eq!(json["type"], "Http");
    assert_eq!(json["url"], "http://localhost:8080/mcp");

    let deserialized: McpConnection = serde_json::from_value(json).unwrap();
    match deserialized {
        McpConnection::Http(h) => {
            assert_eq!(h.url, "http://localhost:8080/mcp");
            assert!(h.headers.is_empty());
        }
        _ => panic!("Expected Http variant"),
    }
}

#[test]
fn http_connection_with_headers() {
    let json = json!({
        "type": "Http",
        "url": "https://api.example.com/mcp",
        "headers": {
            "Authorization": "Bearer abc",
            "X-Custom": "value"
        }
    });
    let conn: McpConnection = serde_json::from_value(json).unwrap();
    match conn {
        McpConnection::Http(h) => {
            assert_eq!(h.url, "https://api.example.com/mcp");
            assert_eq!(h.headers.len(), 2);
            assert_eq!(h.headers.get("Authorization").unwrap(), "Bearer abc");
            assert_eq!(h.headers.get("X-Custom").unwrap(), "value");
        }
        _ => panic!("Expected Http variant"),
    }
}

// ---------------------------------------------------------------------------
// MultiServerMcpClient construction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_tools_empty_before_connect() {
    let client = MultiServerMcpClient::new(HashMap::new());
    let tools = client.get_tools().await;
    assert!(tools.is_empty());
}

#[tokio::test]
async fn multi_server_client_with_prefix_builder() {
    // Constructing with prefix disabled should succeed; tools still empty.
    let client = MultiServerMcpClient::new(HashMap::new()).with_prefix(false);
    let tools = client.get_tools().await;
    assert!(tools.is_empty());
}

#[tokio::test]
async fn connect_with_no_servers_succeeds() {
    // Connecting with an empty server map should not error.
    let client = MultiServerMcpClient::new(HashMap::new());
    let result = client.connect().await;
    assert!(result.is_ok());
    assert!(client.get_tools().await.is_empty());
}
