//! Exec approvals configuration with JSON file persistence.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    /// Deny all exec commands.
    Deny,
    /// Only allow commands matching the allowlist.
    Allowlist,
    /// Allow all commands.
    Full,
}

impl Default for SecurityMode {
    fn default() -> Self {
        Self::Allowlist
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AskPolicy {
    /// Never ask for approval.
    Off,
    /// Ask when a command is not in the allowlist.
    OnMiss,
    /// Always ask for approval.
    Always,
}

impl Default for AskPolicy {
    fn default() -> Self {
        Self::OnMiss
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecApprovalsConfig {
    pub mode: SecurityMode,
    pub ask: AskPolicy,
    pub allowlist: Vec<String>,
    /// SHA256 hash of the serialized config for CAS updates.
    #[serde(skip)]
    pub config_hash: String,
    /// Per-node overrides keyed by node_id.
    #[serde(default)]
    pub node_overrides: std::collections::HashMap<String, NodeExecConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecConfig {
    pub mode: Option<SecurityMode>,
    pub ask: Option<AskPolicy>,
    pub allowlist: Option<Vec<String>>,
}

impl Default for ExecApprovalsConfig {
    fn default() -> Self {
        Self {
            mode: SecurityMode::Allowlist,
            ask: AskPolicy::OnMiss,
            allowlist: vec![
                "ls".to_string(),
                "cat".to_string(),
                "echo".to_string(),
                "pwd".to_string(),
                "whoami".to_string(),
                "date".to_string(),
                "uname".to_string(),
            ],
            config_hash: String::new(),
            node_overrides: std::collections::HashMap::new(),
        }
    }
}

impl ExecApprovalsConfig {
    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config: Self = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        config.config_hash = config.compute_hash();
        config
    }

    pub fn save(&mut self) {
        self.config_hash = self.compute_hash();
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// CAS update: only applies if `expected_hash` matches the current hash.
    pub fn cas_update(
        &mut self,
        expected_hash: &str,
        new_mode: Option<SecurityMode>,
        new_ask: Option<AskPolicy>,
        new_allowlist: Option<Vec<String>>,
    ) -> Result<(), String> {
        if self.config_hash != expected_hash {
            return Err("Config hash mismatch (concurrent modification)".to_string());
        }
        if let Some(mode) = new_mode {
            self.mode = mode;
        }
        if let Some(ask) = new_ask {
            self.ask = ask;
        }
        if let Some(allowlist) = new_allowlist {
            self.allowlist = allowlist;
        }
        self.save();
        Ok(())
    }

    pub fn compute_hash(&self) -> String {
        let data = serde_json::json!({
            "mode": self.mode,
            "ask": self.ask,
            "allowlist": self.allowlist,
        });
        let mut hasher = Sha256::new();
        hasher.update(data.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".synapse")
            .join("exec-approvals.json")
    }
}
