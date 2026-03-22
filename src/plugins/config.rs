use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin system configuration in `synapse.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsConfig {
    /// Exclusive slot assignments: slot_name → plugin_id.
    #[serde(default)]
    pub slots: HashMap<String, String>,

    /// Per-plugin configuration entries.
    #[serde(default)]
    pub entries: HashMap<String, PluginEntryConfig>,

    /// Optional allowlist — if set, only these plugins are loaded.
    #[serde(default)]
    pub allow: Option<Vec<String>>,

    /// Optional denylist — these plugins are never loaded.
    #[serde(default)]
    pub deny: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_true() -> bool {
    true
}

impl Default for PluginEntryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            config: serde_json::Value::Null,
        }
    }
}
