use serde::Deserialize;
use synaptic::deep::sandbox::{
    BindMount, DockerProviderConfig, SandboxResourceLimits, SandboxSecurityConfig,
    SshProviderConfig, WorkspaceAccess,
};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SandboxConfig {
    #[serde(default)]
    pub mode: SandboxMode,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default)]
    pub scope: SandboxScope,
    #[serde(default)]
    pub workspace_access: WorkspaceAccess,
    pub docker: Option<DockerProviderConfig>,
    pub ssh: Option<SshProviderConfig>,
    pub security: Option<SandboxSecurityConfig>,
    pub resources: Option<SandboxResourceLimits>,
    #[serde(default)]
    pub mounts: Vec<BindMount>,
    pub browser: Option<BrowserSandboxConfig>,
    #[serde(default)]
    pub prune: SandboxPruneConfig,
}

fn default_backend() -> String {
    "docker".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    #[default]
    Off,
    NonMain,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SandboxScope {
    #[default]
    Session,
    Agent,
    Shared,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SandboxPruneConfig {
    #[serde(default = "default_idle_hours")]
    pub idle_hours: u32,
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
}

impl Default for SandboxPruneConfig {
    fn default() -> Self {
        Self {
            idle_hours: 24,
            max_age_days: 7,
        }
    }
}

fn default_idle_hours() -> u32 {
    24
}
fn default_max_age_days() -> u32 {
    7
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct BrowserSandboxConfig {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    pub network: Option<String>,
    pub cdp_port: Option<u16>,
    pub vnc_port: Option<u16>,
}
