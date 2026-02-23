use serde::Deserialize;

/// Tool configuration for the agent.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolsConfig {
    /// Enable filesystem tools (default false).
    #[serde(default)]
    pub filesystem: bool,
    /// Sandbox root directory for filesystem operations.
    pub sandbox_root: Option<String>,
}
