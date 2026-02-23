mod format;
mod mcp;
mod model;
mod paths;
mod source;
mod tools;

pub use format::{parse_config, ConfigFormat};
pub use mcp::McpServerConfig;
pub use model::ModelConfig;
pub use paths::PathsConfig;
pub use source::{
    discover_and_load, load_from_file, load_from_source, ConfigSource, FileConfigSource,
    StringConfigSource,
};
pub use tools::ToolsConfig;

use std::path::Path;

use serde::Deserialize;
use synaptic_core::SynapticError;

/// Top-level agent configuration, loaded from TOML / JSON / YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct SynapticAgentConfig {
    pub model: ModelConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub mcp: Option<Vec<McpServerConfig>>,
}

/// Agent behavior configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentConfig {
    pub system_prompt: Option<String>,
    pub max_turns: Option<usize>,
    #[serde(default)]
    pub tools: ToolsConfig,
}

impl SynapticAgentConfig {
    /// Load configuration from a file (TOML, JSON, or YAML).
    ///
    /// Search order:
    /// 1. Explicit path (if provided)
    /// 2. `./synaptic.{toml,json,yaml,yml}`
    /// 3. `~/.synaptic/config.{toml,json,yaml,yml}`
    pub fn load(path: Option<&Path>) -> Result<Self, SynapticError> {
        discover_and_load(path)
    }

    /// Load from any [`ConfigSource`].
    pub fn load_from(source: &dyn ConfigSource) -> Result<Self, SynapticError> {
        load_from_source(source)
    }

    /// Parse from a string in the given format.
    pub fn parse(content: &str, format: ConfigFormat) -> Result<Self, SynapticError> {
        parse_config(content, format)
    }

    /// Resolve the API key from the environment variable specified in `model.api_key_env`.
    pub fn resolve_api_key(&self) -> Result<String, SynapticError> {
        std::env::var(&self.model.api_key_env).map_err(|_| {
            SynapticError::Config(format!(
                "environment variable '{}' not set",
                self.model.api_key_env
            ))
        })
    }
}
