//! E2B code execution sandbox integration for Synaptic.
//!
//! Provides [`E2BSandboxTool`] which executes code in isolated E2B cloud environments.
//! Each tool call creates a fresh sandbox, runs the code, and destroys the environment.
//!
//! # Example
//!
//! ```rust,ignore
//! use synaptic_e2b::{E2BConfig, E2BSandboxTool};
//! use synaptic_core::Tool;
//!
//! let config = E2BConfig::new("your-api-key")
//!     .with_template("python")
//!     .with_timeout(30);
//! let tool = E2BSandboxTool::new(config);
//!
//! let result = tool.call(serde_json::json!({
//!     "code": "print(sum(range(1, 101)))",
//!     "language": "python"
//! })).await?;
//! // {"stdout": "5050\n", "stderr": "", "exit_code": 0}
//! ```

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

/// Configuration for the E2B sandbox.
#[derive(Debug, Clone)]
pub struct E2BConfig {
    /// E2B API key.
    pub api_key: String,
    /// Sandbox template (e.g. `"base"`, `"python"`, `"nodejs"`).
    pub template: String,
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
}

impl E2BConfig {
    /// Create a new config with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            template: "base".to_string(),
            timeout_secs: 30,
        }
    }

    /// Set the sandbox template.
    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.template = template.into();
        self
    }

    /// Set the execution timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// E2B sandbox tool for executing code in isolated cloud environments.
///
/// Creates an ephemeral E2B sandbox per invocation, executes the provided code,
/// then destroys the sandbox. Supports Python, JavaScript, and Bash.
pub struct E2BSandboxTool {
    config: E2BConfig,
    client: reqwest::Client,
}

impl E2BSandboxTool {
    /// Create a new `E2BSandboxTool` with the given configuration.
    pub fn new(config: E2BConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for E2BSandboxTool {
    fn name(&self) -> &'static str {
        "e2b_code_executor"
    }

    fn description(&self) -> &'static str {
        "Execute code in an isolated E2B cloud sandbox. Supports Python, JavaScript, and other \
         languages. Returns stdout, stderr, and exit code."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "The code to execute"
                },
                "language": {
                    "type": "string",
                    "enum": ["python", "javascript", "bash"],
                    "description": "The programming language of the code"
                }
            },
            "required": ["code", "language"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let code = args["code"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'code' parameter".to_string()))?;
        let language = args["language"].as_str().unwrap_or("python");

        // Step 1: Create sandbox
        let create_resp = self
            .client
            .post("https://api.e2b.dev/sandboxes")
            .header("X-API-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({
                "template": self.config.template,
                "timeout": self.config.timeout_secs,
            }))
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("E2B create sandbox: {e}")))?;

        let create_status = create_resp.status().as_u16();
        let create_body: Value = create_resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("E2B create parse: {e}")))?;

        if create_status != 200 && create_status != 201 {
            return Err(SynapticError::Tool(format!(
                "E2B create sandbox error ({}): {}",
                create_status, create_body
            )));
        }

        let sandbox_id = create_body["sandboxId"]
            .as_str()
            .or_else(|| create_body["sandbox_id"].as_str())
            .ok_or_else(|| SynapticError::Tool("E2B: missing sandbox ID in response".to_string()))?
            .to_string();

        // Step 2: Execute code
        let exec_resp = self
            .client
            .post(format!(
                "https://api.e2b.dev/sandboxes/{}/process",
                sandbox_id
            ))
            .header("X-API-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({
                "cmd": get_cmd(language, code),
                "timeout": self.config.timeout_secs,
            }))
            .send()
            .await;

        // Step 3: Always destroy sandbox (best-effort)
        let _ = self
            .client
            .delete(format!("https://api.e2b.dev/sandboxes/{}", sandbox_id))
            .header("X-API-Key", &self.config.api_key)
            .send()
            .await;

        let exec_resp = exec_resp.map_err(|e| SynapticError::Tool(format!("E2B execute: {e}")))?;
        let exec_body: Value = exec_resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("E2B execute parse: {e}")))?;

        Ok(json!({
            "stdout": exec_body["stdout"],
            "stderr": exec_body["stderr"],
            "exit_code": exec_body["exitCode"]
                .as_i64()
                .unwrap_or_else(|| exec_body["exit_code"].as_i64().unwrap_or(0)),
        }))
    }
}

fn get_cmd(language: &str, code: &str) -> Vec<String> {
    match language {
        "python" => vec!["python3".to_string(), "-c".to_string(), code.to_string()],
        "javascript" | "js" => vec!["node".to_string(), "-e".to_string(), code.to_string()],
        _ => vec!["bash".to_string(), "-c".to_string(), code.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = E2BConfig::new("test-key");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.template, "base");
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn config_builder() {
        let config = E2BConfig::new("key")
            .with_template("python")
            .with_timeout(60);
        assert_eq!(config.template, "python");
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn tool_name() {
        let tool = E2BSandboxTool::new(E2BConfig::new("key"));
        assert_eq!(tool.name(), "e2b_code_executor");
    }

    #[test]
    fn tool_description_contains_sandbox() {
        let tool = E2BSandboxTool::new(E2BConfig::new("key"));
        assert!(tool.description().contains("sandbox") || tool.description().contains("E2B"));
    }

    #[test]
    fn tool_parameters() {
        let tool = E2BSandboxTool::new(E2BConfig::new("key"));
        let params = tool.parameters().unwrap();
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["code"].is_object());
        assert!(params["properties"]["language"].is_object());
    }

    #[test]
    fn get_cmd_python() {
        let cmd = get_cmd("python", "print('hi')");
        assert_eq!(cmd[0], "python3");
        assert_eq!(cmd[1], "-c");
        assert_eq!(cmd[2], "print('hi')");
    }

    #[test]
    fn get_cmd_javascript() {
        let cmd = get_cmd("javascript", "console.log('hi')");
        assert_eq!(cmd[0], "node");
        assert_eq!(cmd[1], "-e");
    }

    #[test]
    fn get_cmd_bash() {
        let cmd = get_cmd("bash", "echo hi");
        assert_eq!(cmd[0], "bash");
        assert_eq!(cmd[1], "-c");
    }

    #[tokio::test]
    async fn missing_code_returns_error() {
        let tool = E2BSandboxTool::new(E2BConfig::new("key"));
        let result = tool.call(json!({"language": "python"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("code"));
    }
}
