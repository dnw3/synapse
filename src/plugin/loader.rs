//! External plugin process spawning and lifecycle management.
//!
//! This module handles loading external plugins that communicate over stdio or
//! HTTP transports. For stdio plugins a child process is spawned and its
//! stdin/stdout are captured for communication. HTTP plugins simply record the
//! endpoint URL for later use.
//!
//! # Manifest format
//!
//! External plugins are described by a `plugin.toml` file in their directory:
//!
//! ```toml
//! [plugin]
//! name = "my-plugin"
//! version = "0.1.0"
//! description = "Does something useful"
//!
//! [runtime]
//! command = "node"
//! args = ["index.js"]
//! transport = "stdio"          # or "http"
//! url = ""                     # only for http transport
//! env = { MY_VAR = "value" }   # optional extra env vars
//!
//! [capabilities]
//! tools = true
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use synaptic::events::EventBus;
use synaptic::plugin::PluginRegistry;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// ExternalPluginInfo
// ---------------------------------------------------------------------------

/// Transport mechanism used by an external plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginTransport {
    /// Plugin communicates via stdin/stdout (JSON-RPC over stdio).
    Stdio {
        /// Executable to run (e.g. `"node"`).
        command: String,
        /// Arguments to pass to the command (e.g. `["index.js"]`).
        args: Vec<String>,
        /// Optional extra environment variables.
        env: HashMap<String, String>,
        /// Working directory for the child process (defaults to plugin dir).
        cwd: PathBuf,
    },
    /// Plugin exposes an HTTP/SSE endpoint.
    Http {
        /// Base URL of the plugin server (e.g. `"http://localhost:8080"`).
        url: String,
    },
}

/// All information needed to load one external plugin.
#[derive(Debug, Clone)]
pub struct ExternalPluginInfo {
    /// Plugin identifier (from manifest `[plugin].name`).
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Transport configuration.
    pub transport: PluginTransport,
    /// Filesystem path to the plugin directory (contains `plugin.toml`).
    pub plugin_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// LoadedExternalPlugin
// ---------------------------------------------------------------------------

/// A running external plugin — holds the child-process handle (stdio) or the
/// stored URL (HTTP).
pub struct LoadedExternalPlugin {
    /// Metadata snapshotted at load time.
    pub info: ExternalPluginInfo,
    /// Child process handle (only present for stdio plugins).
    child: Option<Child>,
}

impl std::fmt::Debug for LoadedExternalPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedExternalPlugin")
            .field("name", &self.info.name)
            .field("transport", &self.info.transport)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl LoadedExternalPlugin {
    /// Returns `true` if the child process is still running (stdio plugins only).
    ///
    /// For HTTP plugins this always returns `true` (we cannot know from here).
    pub fn is_alive(&mut self) -> bool {
        match &mut self.child {
            None => true, // HTTP — assume alive
            Some(child) => {
                // `try_wait` returns Ok(None) when still running.
                match child.try_wait() {
                    Ok(None) => true,
                    Ok(Some(status)) => {
                        tracing::warn!(
                            name = %self.info.name,
                            status = ?status,
                            "external plugin process exited unexpectedly"
                        );
                        false
                    }
                    Err(err) => {
                        tracing::warn!(
                            name = %self.info.name,
                            error = %err,
                            "failed to query plugin process status"
                        );
                        false
                    }
                }
            }
        }
    }

    /// Kill the child process (stdio plugins only). A best-effort send of
    /// SIGKILL / TerminateProcess; errors are logged and swallowed.
    pub async fn kill(&mut self) {
        if let Some(ref mut child) = self.child {
            tracing::info!(name = %self.info.name, "killing external plugin process");
            if let Err(err) = child.kill().await {
                tracing::warn!(
                    name = %self.info.name,
                    error = %err,
                    "failed to kill plugin process"
                );
            }
        }
    }
}

impl Drop for LoadedExternalPlugin {
    fn drop(&mut self) {
        // On drop, start a kill (best-effort, non-blocking). We use start_kill
        // which doesn't require an async context.
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

// ---------------------------------------------------------------------------
// ExternalPluginLoader
// ---------------------------------------------------------------------------

/// Loads external plugins from their manifest info.
pub struct ExternalPluginLoader;

impl ExternalPluginLoader {
    /// Load a single external plugin described by `info`.
    ///
    /// - **Stdio**: spawns the child process, capturing stdin/stdout.
    /// - **HTTP**: stores the URL (no process is spawned).
    ///
    /// Returns a [`LoadedExternalPlugin`] on success.
    pub async fn load(info: &ExternalPluginInfo) -> Result<LoadedExternalPlugin, LoadError> {
        match &info.transport {
            PluginTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                if command.is_empty() {
                    return Err(LoadError::InvalidManifest(
                        "runtime.command is empty".to_string(),
                    ));
                }

                tracing::info!(
                    name = %info.name,
                    command = %command,
                    args = ?args,
                    cwd = %cwd.display(),
                    "spawning external plugin process"
                );

                let mut cmd = Command::new(command);
                cmd.args(args)
                    .current_dir(cwd)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::inherit())
                    .kill_on_drop(true);

                for (k, v) in env {
                    cmd.env(k, v);
                }

                let child = cmd.spawn().map_err(|e| {
                    LoadError::SpawnFailed(format!("failed to spawn '{}': {}", command, e))
                })?;

                tracing::info!(
                    name = %info.name,
                    pid = child.id().unwrap_or(0),
                    "external plugin process started"
                );

                Ok(LoadedExternalPlugin {
                    info: info.clone(),
                    child: Some(child),
                })
            }

            PluginTransport::Http { url } => {
                if url.is_empty() {
                    return Err(LoadError::InvalidManifest(
                        "runtime.url is empty for http transport".to_string(),
                    ));
                }

                tracing::info!(
                    name = %info.name,
                    url = %url,
                    "registered external HTTP plugin"
                );

                Ok(LoadedExternalPlugin {
                    info: info.clone(),
                    child: None,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// discover_and_load
// ---------------------------------------------------------------------------

/// On-disk manifest structure (`plugin.toml`).
#[derive(Debug, Deserialize)]
struct TomlManifest {
    #[serde(default)]
    plugin: TomlPluginSection,
    #[serde(default)]
    runtime: TomlRuntimeSection,
}

#[derive(Debug, Default, Deserialize)]
struct TomlPluginSection {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Default, Deserialize)]
struct TomlRuntimeSection {
    #[serde(default)]
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    transport: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Scan `dirs` for plugin directories, parse their `plugin.toml` manifests,
/// attempt to load each one, and register loaded plugins in `registry`.
///
/// The `event_bus` parameter is accepted for future use (e.g. emitting
/// `PluginLoaded` events); it is not yet wired to anything here.
///
/// Returns the list of successfully loaded plugins.
pub async fn discover_and_load(
    dirs: &[PathBuf],
    _event_bus: Arc<EventBus>,
    registry: Arc<RwLock<PluginRegistry>>,
) -> Vec<LoadedExternalPlugin> {
    let mut loaded = Vec::new();

    for dir in dirs {
        if !dir.exists() {
            continue;
        }

        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(
                    dir = %dir.display(),
                    error = %err,
                    "discover_and_load: failed to read plugin directory"
                );
                continue;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match parse_manifest(&manifest_path, &plugin_dir).await {
                Err(err) => {
                    tracing::warn!(
                        path = %manifest_path.display(),
                        error = %err,
                        "discover_and_load: failed to parse manifest"
                    );
                }
                Ok(info) => {
                    let name = info.name.clone();
                    match ExternalPluginLoader::load(&info).await {
                        Ok(plugin) => {
                            tracing::info!(
                                name = %name,
                                version = %info.version,
                                "external plugin loaded"
                            );

                            // Record in PluginRegistry so dashboard/API sees it.
                            {
                                let mut reg = registry.write().await;
                                reg.record_plugin(synaptic::plugin::PluginManifest {
                                    name: info.name.clone(),
                                    version: info.version.clone(),
                                    description: info.description.clone(),
                                    author: None,
                                    license: None,
                                    capabilities: vec![],
                                    slot: None,
                                });
                            }

                            loaded.push(plugin);
                        }
                        Err(err) => {
                            tracing::warn!(
                                name = %name,
                                error = %err,
                                "discover_and_load: failed to load plugin"
                            );
                        }
                    }
                }
            }
        }
    }

    loaded
}

/// Parse a `plugin.toml` file and return an [`ExternalPluginInfo`].
async fn parse_manifest(
    manifest_path: &Path,
    plugin_dir: &Path,
) -> Result<ExternalPluginInfo, LoadError> {
    let contents = tokio::fs::read_to_string(manifest_path)
        .await
        .map_err(|e| LoadError::Io(e.to_string()))?;

    let manifest: TomlManifest =
        toml::from_str(&contents).map_err(|e| LoadError::InvalidManifest(e.to_string()))?;

    let name = if manifest.plugin.name.is_empty() {
        plugin_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        manifest.plugin.name
    };

    let transport = match manifest.runtime.transport.to_lowercase().as_str() {
        "http" | "sse" => PluginTransport::Http {
            url: manifest.runtime.url,
        },
        // Default to stdio (covers "stdio", "" or any unknown value)
        _ => PluginTransport::Stdio {
            command: manifest.runtime.command,
            args: manifest.runtime.args,
            env: manifest.runtime.env,
            cwd: plugin_dir.to_path_buf(),
        },
    };

    Ok(ExternalPluginInfo {
        name,
        version: manifest.plugin.version,
        description: manifest.plugin.description,
        transport,
        plugin_dir: plugin_dir.to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// LoadError
// ---------------------------------------------------------------------------

/// Error returned by [`ExternalPluginLoader::load`].
#[derive(Debug)]
pub enum LoadError {
    /// Manifest could not be parsed or contains invalid values.
    InvalidManifest(String),
    /// Spawning the child process failed (stdio transport).
    SpawnFailed(String),
    /// An I/O error occurred while reading the manifest.
    Io(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidManifest(msg) => write!(f, "invalid plugin manifest: {}", msg),
            Self::SpawnFailed(msg) => write!(f, "plugin process spawn failed: {}", msg),
            Self::Io(msg) => write!(f, "plugin I/O error: {}", msg),
        }
    }
}

impl std::error::Error for LoadError {}
