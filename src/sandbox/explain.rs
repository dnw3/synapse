use serde::Serialize;
use synaptic::deep::sandbox::SandboxSecurityConfig;

#[derive(Debug, Clone, Serialize)]
pub struct SandboxExplanation {
    pub agent_id: String,
    pub session_key: String,
    pub mode: String,
    pub scope: String,
    pub workspace_access: String,
    pub backend: String,
    pub is_sandboxed: bool,
    pub scope_key: String,
    pub security: SandboxSecuritySummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct SandboxSecuritySummary {
    pub cap_drop: Vec<String>,
    pub read_only_root: bool,
    pub network_mode: String,
}

impl SandboxSecuritySummary {
    pub fn from_config(config: &SandboxSecurityConfig) -> Self {
        Self {
            cap_drop: config.cap_drop.clone(),
            read_only_root: config.read_only_root,
            network_mode: format!("{:?}", config.network_mode),
        }
    }
}

impl std::fmt::Display for SandboxExplanation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Agent:            {}", self.agent_id)?;
        writeln!(f, "Session:          {}", self.session_key)?;
        writeln!(
            f,
            "Runtime status:   {}",
            if self.is_sandboxed {
                "SANDBOXED"
            } else {
                "HOST"
            }
        )?;
        writeln!(f, "Mode:             {}", self.mode)?;
        writeln!(f, "Scope:            {}", self.scope)?;
        writeln!(f, "Workspace access: {}", self.workspace_access)?;
        writeln!(f, "Backend:          {}", self.backend)?;
        writeln!(f, "Network:          {}", self.security.network_mode)?;
        write!(
            f,
            "Security:         capDrop={:?}, readOnlyRoot={}",
            self.security.cap_drop, self.security.read_only_root
        )
    }
}
