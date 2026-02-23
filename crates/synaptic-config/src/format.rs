use std::path::Path;

use serde::de::DeserializeOwned;
use synaptic_core::SynapticError;

/// Supported configuration file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Toml,
    Json,
    Yaml,
}

impl ConfigFormat {
    /// Detect format from a file extension string (e.g. "toml", "json", "yaml", "yml").
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Detect format from a file path's extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }
}

/// Parse a configuration string in the given format into type `T`.
pub fn parse_config<T: DeserializeOwned>(
    content: &str,
    format: ConfigFormat,
) -> Result<T, SynapticError> {
    match format {
        ConfigFormat::Toml => toml::from_str(content)
            .map_err(|e| SynapticError::Config(format!("TOML parse error: {e}"))),
        ConfigFormat::Json => serde_json::from_str(content)
            .map_err(|e| SynapticError::Config(format!("JSON parse error: {e}"))),
        ConfigFormat::Yaml => serde_yml::from_str(content)
            .map_err(|e| SynapticError::Config(format!("YAML parse error: {e}"))),
    }
}
