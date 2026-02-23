use serde::Deserialize;

/// Path configuration for agent data.
#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    /// Directory for session data (default: ".sessions").
    #[serde(default = "default_sessions_dir")]
    pub sessions_dir: String,
    /// Path to memory file (default: "AGENTS.md").
    #[serde(default = "default_memory_file")]
    pub memory_file: String,
    /// Path to skills directory (default: ".skills").
    #[serde(default = "default_skills_dir")]
    pub skills_dir: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            sessions_dir: default_sessions_dir(),
            memory_file: default_memory_file(),
            skills_dir: default_skills_dir(),
        }
    }
}

fn default_sessions_dir() -> String {
    ".sessions".to_string()
}

fn default_memory_file() -> String {
    "AGENTS.md".to_string()
}

fn default_skills_dir() -> String {
    ".skills".to_string()
}
