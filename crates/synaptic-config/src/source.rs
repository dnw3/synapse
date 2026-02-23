use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use synaptic_core::SynapticError;

use crate::format::{parse_config, ConfigFormat};

/// Configuration source abstraction.
///
/// Future implementations (Apollo, Nacos, etcd) can implement this trait
/// to provide configuration from remote sources.
pub trait ConfigSource: Send + Sync {
    /// Fetch the current configuration content and its format.
    fn fetch(&self) -> Result<(String, ConfigFormat), SynapticError>;
}

/// Load configuration from a local file, auto-detecting format by extension.
pub struct FileConfigSource {
    path: PathBuf,
    format: Option<ConfigFormat>,
}

impl FileConfigSource {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            format: None,
        }
    }

    /// Override the auto-detected format.
    pub fn with_format(mut self, format: ConfigFormat) -> Self {
        self.format = Some(format);
        self
    }
}

impl ConfigSource for FileConfigSource {
    fn fetch(&self) -> Result<(String, ConfigFormat), SynapticError> {
        let format = self
            .format
            .or_else(|| ConfigFormat::from_path(&self.path))
            .ok_or_else(|| {
                SynapticError::Config(format!(
                    "cannot detect config format from extension: {}",
                    self.path.display()
                ))
            })?;

        let content = std::fs::read_to_string(&self.path).map_err(|e| {
            SynapticError::Config(format!("failed to read {}: {e}", self.path.display()))
        })?;

        Ok((content, format))
    }
}

/// Load configuration from an in-memory string (useful for tests or config-center payloads).
pub struct StringConfigSource {
    content: String,
    format: ConfigFormat,
}

impl StringConfigSource {
    pub fn new(content: impl Into<String>, format: ConfigFormat) -> Self {
        Self {
            content: content.into(),
            format,
        }
    }
}

impl ConfigSource for StringConfigSource {
    fn fetch(&self) -> Result<(String, ConfigFormat), SynapticError> {
        Ok((self.content.clone(), self.format))
    }
}

/// Load and parse configuration from any [`ConfigSource`].
pub fn load_from_source<T: DeserializeOwned>(
    source: &dyn ConfigSource,
) -> Result<T, SynapticError> {
    let (content, format) = source.fetch()?;
    parse_config(&content, format)
}

/// Load and parse configuration from a file path (convenience wrapper).
pub fn load_from_file<T: DeserializeOwned>(path: &Path) -> Result<T, SynapticError> {
    load_from_source(&FileConfigSource::new(path))
}

/// File-discovery search order for config files.
const EXTENSIONS: &[&str] = &["toml", "json", "yaml", "yml"];

/// Discover a configuration file and load it as `T`.
///
/// Search order:
/// 1. Explicit `path` (if provided) — format detected by extension
/// 2. `./synaptic.{toml,json,yaml,yml}` in the current directory
/// 3. `~/.synaptic/config.{toml,json,yaml,yml}` in the home directory
pub fn discover_and_load<T: DeserializeOwned>(path: Option<&Path>) -> Result<T, SynapticError> {
    if let Some(p) = path {
        if p.exists() {
            return load_from_file(p);
        } else {
            return Err(SynapticError::Config(format!(
                "config file not found: {}",
                p.display()
            )));
        }
    }

    // Search current directory
    for ext in EXTENSIONS {
        let candidate = PathBuf::from(format!("./synaptic.{ext}"));
        if candidate.exists() {
            return load_from_file(&candidate);
        }
    }

    // Search home directory
    if let Some(home) = dirs::home_dir() {
        for ext in EXTENSIONS {
            let candidate = home.join(".synaptic").join(format!("config.{ext}"));
            if candidate.exists() {
                return load_from_file(&candidate);
            }
        }
    }

    Err(SynapticError::Config(
        "no config file found: tried ./synaptic.{toml,json,yaml} and ~/.synaptic/config.{toml,json,yaml}".to_string(),
    ))
}
