//! ExternalPluginBridge — JSON-RPC 2.0 bridge for OpenClaw JS plugins.
//!
//! Synapse spawns a `node` subprocess, sends JSON-RPC requests over stdin,
//! and reads newline-delimited JSON responses from stdout.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use synaptic::core::{SynapticError, Tool};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Metadata for a tool exposed by an external JS plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ExternalToolDef {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    #[allow(dead_code)]
    code: i64,
    message: String,
}

// ---------------------------------------------------------------------------
// ExternalPluginBridge
// ---------------------------------------------------------------------------

/// Subprocess-based JSON-RPC 2.0 bridge to an OpenClaw JS plugin.
#[allow(dead_code)]
pub struct ExternalPluginBridge {
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    process: Mutex<tokio::process::Child>,
    pub id: String,
    pub tool_defs: Vec<ExternalToolDef>,
    next_id: Mutex<u64>,
}

#[allow(dead_code)]
impl ExternalPluginBridge {
    /// Spawn a `node` subprocess for the plugin at `plugin_dir`, send an
    /// `initialize` RPC with `config`, and collect the advertised tool list.
    pub async fn spawn(plugin_dir: &Path, config: &Value) -> Result<Arc<Self>, SynapticError> {
        // Determine entry point from package.json
        let entry = Self::resolve_entry(plugin_dir)?;

        let mut child = tokio::process::Command::new("node")
            .arg(&entry)
            .current_dir(plugin_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| SynapticError::Tool(format!("failed to spawn node plugin: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SynapticError::Tool("could not open plugin stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SynapticError::Tool("could not open plugin stdout".into()))?;

        let plugin_id = plugin_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("external-plugin")
            .to_string();

        let bridge = Arc::new(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            process: Mutex::new(child),
            id: plugin_id,
            tool_defs: Vec::new(),
            next_id: Mutex::new(1),
        });

        // Send initialize and collect tool definitions
        let result = bridge
            .rpc("initialize", serde_json::json!({ "config": config }))
            .await?;

        let tool_defs: Vec<ExternalToolDef> = result
            .get("tools")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();

        // SAFETY: We exclusively own the Arc at this point (just created it),
        // so getting a mutable reference via Arc::get_mut is valid.
        let bridge_mut = Arc::into_raw(bridge);
        // Reconstruct as mutable to fill tool_defs
        // We use a safe pattern: rebuild from raw pointer with get_mut.
        let mut bridge = unsafe { Arc::from_raw(bridge_mut) };
        Arc::get_mut(&mut bridge)
            .expect("exclusive Arc at spawn time")
            .tool_defs = tool_defs;

        tracing::info!(
            plugin = %bridge.id,
            tools = %bridge.tool_defs.len(),
            "external plugin bridge ready"
        );

        Ok(bridge)
    }

    /// Call a named tool in the plugin process.
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, SynapticError> {
        let result = self
            .rpc(
                "tool/call",
                serde_json::json!({ "name": name, "args": args }),
            )
            .await?;

        Ok(result.get("output").cloned().unwrap_or(result))
    }

    /// Return `Tool` trait objects for every tool advertised by this plugin.
    pub fn tools(self: &Arc<Self>) -> Vec<Arc<dyn Tool>> {
        self.tool_defs
            .iter()
            .map(|def| {
                let tool: Arc<dyn Tool> = Arc::new(BridgedTool {
                    bridge: Arc::clone(self),
                    // SAFETY: Plugin tools live for the process lifetime.
                    // Box::leak produces a &'static str from the heap-allocated name.
                    name: Box::leak(def.name.clone().into_boxed_str()),
                    description: Box::leak(def.description.clone().into_boxed_str()),
                    parameters: def.parameters.clone(),
                });
                tool
            })
            .collect()
    }

    /// Kill the plugin subprocess.
    pub async fn shutdown(&self) {
        let mut child = self.process.lock().await;
        if let Err(e) = child.kill().await {
            tracing::warn!(plugin = %self.id, error = %e, "failed to kill plugin process");
        } else {
            tracing::info!(plugin = %self.id, "external plugin process terminated");
        }
    }

    // -----------------------------------------------------------------------
    // Internal RPC transport
    // -----------------------------------------------------------------------

    async fn next_id(&self) -> u64 {
        let mut guard = self.next_id.lock().await;
        let id = *guard;
        *guard += 1;
        id
    }

    /// Send a JSON-RPC 2.0 request and return the `result` value.
    async fn rpc(&self, method: &str, params: Value) -> Result<Value, SynapticError> {
        let id = self.next_id().await;
        let request = RpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let mut line =
            serde_json::to_string(&request).map_err(|e| SynapticError::Tool(e.to_string()))?;
        line.push('\n');

        // Write request
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| SynapticError::Tool(format!("plugin stdin write error: {e}")))?;
            stdin
                .flush()
                .await
                .map_err(|e| SynapticError::Tool(format!("plugin stdin flush error: {e}")))?;
        }

        // Read response
        let response_line = {
            let mut stdout = self.stdout.lock().await;
            let mut buf = String::new();
            stdout
                .read_line(&mut buf)
                .await
                .map_err(|e| SynapticError::Tool(format!("plugin stdout read error: {e}")))?;
            buf
        };

        let response: RpcResponse = serde_json::from_str(response_line.trim()).map_err(|e| {
            SynapticError::Tool(format!(
                "invalid JSON-RPC response from plugin '{}': {e} (raw: {response_line})",
                self.id
            ))
        })?;

        if let Some(err) = response.error {
            return Err(SynapticError::Tool(format!(
                "plugin '{}' returned error: {}",
                self.id, err.message
            )));
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    // -----------------------------------------------------------------------
    // Helper: resolve node entry point
    // -----------------------------------------------------------------------

    fn resolve_entry(plugin_dir: &Path) -> Result<std::path::PathBuf, SynapticError> {
        let pkg_path = plugin_dir.join("package.json");
        if let Ok(data) = std::fs::read_to_string(&pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<Value>(&data) {
                if let Some(main) = pkg.get("main").and_then(|v| v.as_str()) {
                    let entry = plugin_dir.join(main);
                    if entry.exists() {
                        return Ok(entry);
                    }
                }
            }
        }

        // Fallback candidates
        for candidate in &["index.js", "plugin.js", "main.js"] {
            let p = plugin_dir.join(candidate);
            if p.exists() {
                return Ok(p);
            }
        }

        Err(SynapticError::Tool(format!(
            "cannot find node entry point in plugin directory '{}'",
            plugin_dir.display()
        )))
    }
}

// ---------------------------------------------------------------------------
// BridgedTool
// ---------------------------------------------------------------------------

/// Thin wrapper that implements `Tool` by delegating to an `ExternalPluginBridge`.
#[allow(dead_code)]
struct BridgedTool {
    bridge: Arc<ExternalPluginBridge>,
    name: &'static str,
    description: &'static str,
    parameters: Option<Value>,
}

#[async_trait]
impl Tool for BridgedTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn parameters(&self) -> Option<Value> {
        self.parameters.clone()
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        self.bridge.call_tool(self.name, args).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_tool_def_roundtrip() {
        let def = ExternalToolDef {
            name: "my_tool".to_string(),
            description: "Does something".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": { "x": { "type": "string" } },
                "required": ["x"]
            })),
        };
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ExternalToolDef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "my_tool");
        assert_eq!(parsed.description, "Does something");
        assert!(parsed.parameters.is_some());
    }

    #[test]
    fn external_tool_def_no_params_roundtrip() {
        let def = ExternalToolDef {
            name: "no_params".to_string(),
            description: "No params".to_string(),
            parameters: None,
        };
        let json = serde_json::to_string(&def).unwrap();
        // parameters field should be absent
        assert!(!json.contains("parameters"));
        let parsed: ExternalToolDef = serde_json::from_str(&json).unwrap();
        assert!(parsed.parameters.is_none());
    }

    #[test]
    fn rpc_error_deserialization() {
        let raw =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"something failed"}}"#;
        let resp: RpcResponse = serde_json::from_str(raw).unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32000);
        assert_eq!(err.message, "something failed");
    }

    #[test]
    fn rpc_result_deserialization() {
        let raw = r#"{"jsonrpc":"2.0","id":2,"result":{"output":"hello"}}"#;
        let resp: RpcResponse = serde_json::from_str(raw).unwrap();
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["output"], "hello");
    }

    #[test]
    fn resolve_entry_fallback_index_js() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.js"), "// stub").unwrap();
        let entry = ExternalPluginBridge::resolve_entry(dir.path()).unwrap();
        assert!(entry.ends_with("index.js"));
    }

    #[test]
    fn resolve_entry_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"test","main":"src/plugin.js"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/plugin.js"), "// stub").unwrap();
        let entry = ExternalPluginBridge::resolve_entry(dir.path()).unwrap();
        assert!(entry.ends_with("src/plugin.js"));
    }

    #[test]
    fn resolve_entry_missing_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = ExternalPluginBridge::resolve_entry(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot find node entry point"));
    }
}
