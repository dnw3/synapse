//! Plugin SDK scaffold for Synapse.
//!
//! Defines the [`Plugin`] trait, [`PluginCommand`], [`PluginManager`], and [`PluginInfo`]
//! types. Plugins can be loaded as built-in Rust structs or discovered from
//! `.synapse/plugins/` via TOML manifests (`plugin.toml`).
//!
//! # Future work
//! - Dynamic loading via `libloading` (shared objects / `.dll` / `.dylib`).
//! - A remote plugin registry for distribution and versioning.

pub mod loader;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use synaptic::core::ToolDefinition;
use synaptic::plugin::{PluginCapability, PluginManifest, PluginRegistry};

// ---------------------------------------------------------------------------
// PluginCommand
// ---------------------------------------------------------------------------

/// A command contributed by a plugin, callable from the Synapse CLI.
pub struct PluginCommand {
    /// Short command name (e.g. `"greet"`).
    pub name: String,
    /// Human-readable description shown in help output.
    pub description: String,
    /// Handler invoked with the raw argument string; returns a response string.
    pub handler: Box<dyn Fn(&str) -> String + Send + Sync>,
}

impl std::fmt::Debug for PluginCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// The core interface every Synapse plugin must implement.
///
/// Plugins are registered with a [`PluginManager`] either as built-in Rust
/// types (via [`PluginManager::load_builtin`]) or discovered from disk
/// manifests (via [`PluginManager::scan_directory`]).
pub trait Plugin: Send + Sync {
    /// Unique identifier for the plugin (e.g. `"my-plugin"`).
    fn name(&self) -> &str;

    /// SemVer version string (e.g. `"1.0.0"`).
    fn version(&self) -> &str;

    /// Short human-readable description.
    fn description(&self) -> &str;

    /// Called once when the plugin is loaded. Return an error to abort loading.
    fn on_load(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Called when the plugin is unloaded or the manager is dropped.
    fn on_unload(&self) {}

    /// Tool definitions this plugin contributes to the agent's tool registry.
    fn tools(&self) -> Vec<ToolDefinition> {
        vec![]
    }

    /// CLI commands this plugin contributes.
    fn commands(&self) -> Vec<PluginCommand> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// PluginInfo
// ---------------------------------------------------------------------------

/// Lightweight, cloneable snapshot of a plugin's identity and state.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin description.
    pub description: String,
    /// Whether the plugin is currently enabled.
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// On-disk manifest (plugin.toml)
// ---------------------------------------------------------------------------

/// Minimal TOML manifest describing a file-based plugin.
///
/// Stored as `.synapse/plugins/<plugin-name>/plugin.toml`.
#[derive(Debug, Deserialize)]
struct FilePluginManifest {
    name: String,
    version: String,
    description: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// PluginManager
// ---------------------------------------------------------------------------

/// Manages the lifecycle of all loaded plugins.
///
/// Plugins may be:
/// - **Built-in**: compiled Rust types registered via [`load_builtin`](Self::load_builtin).
/// - **File-based**: discovered from `.synapse/plugins/` via [`scan_directory`](Self::scan_directory).
///
/// The manager tracks enabled/disabled state in `.synapse/plugins/state.json`.
pub struct PluginManager {
    /// All loaded plugin instances.
    plugins: Vec<Box<dyn Plugin>>,
    /// Names of explicitly disabled plugins.
    disabled: HashSet<String>,
}

impl PluginManager {
    /// Create an empty plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            disabled: HashSet::new(),
        }
    }

    // ------------------------------------------------------------------
    // Loading
    // ------------------------------------------------------------------

    /// Register a built-in plugin, calling its [`Plugin::on_load`] hook.
    ///
    /// If `on_load` returns an error the plugin is **not** added.
    pub fn load_builtin(&mut self, plugin: Box<dyn Plugin>) {
        match plugin.on_load() {
            Ok(()) => {
                tracing::info!(name = %plugin.name(), version = %plugin.version(), "plugin loaded");
                self.plugins.push(plugin);
            }
            Err(e) => {
                tracing::warn!(name = %plugin.name(), error = %e, "plugin on_load failed, skipping");
            }
        }
    }

    /// Scan a directory for file-based plugin manifests (`plugin.toml`).
    ///
    /// Each immediate subdirectory of `dir` is checked for a `plugin.toml`.
    /// If a shared library (`.so`/`.dylib`/`.dll`) is found alongside the manifest,
    /// it is loaded dynamically via `libloading` (requires the `plugins` feature).
    pub fn scan_directory(&mut self, dir: &Path) {
        if !dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(dir = %dir.display(), error = %err, "failed to read plugin directory");
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match std::fs::read_to_string(&manifest_path) {
                Err(err) => {
                    tracing::warn!(path = %manifest_path.display(), error = %err, "failed to read plugin manifest");
                }
                Ok(contents) => match toml::from_str::<FilePluginManifest>(&contents) {
                    Err(err) => {
                        tracing::warn!(path = %manifest_path.display(), error = %err, "invalid plugin manifest");
                    }
                    Ok(manifest) => {
                        // Try dynamic loading if a shared library is present
                        #[cfg(feature = "plugins")]
                        {
                            if let Some(lib_path) = find_shared_library(&path) {
                                match load_dynamic_plugin(&lib_path) {
                                    Ok(plugin) => {
                                        tracing::info!(
                                            name = %plugin.name(),
                                            version = %plugin.version(),
                                            path = %lib_path.display(),
                                            "plugin loaded from shared library"
                                        );
                                        self.plugins.push(plugin);
                                        continue;
                                    }
                                    Err(err) => {
                                        tracing::warn!(
                                            name = %manifest.name,
                                            path = %lib_path.display(),
                                            error = %err,
                                            "failed to load plugin from shared library"
                                        );
                                    }
                                }
                            }
                        }

                        tracing::info!(
                            name = %manifest.name,
                            version = %manifest.version,
                            path = %path.display(),
                            "plugin discovered{}",
                            if cfg!(feature = "plugins") {
                                " (no shared library found)"
                            } else {
                                " (enable 'plugins' feature for dynamic loading)"
                            }
                        );
                    }
                },
            }
        }
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Return a snapshot of all loaded plugins.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|p| PluginInfo {
                name: p.name().to_string(),
                version: p.version().to_string(),
                description: p.description().to_string(),
                enabled: !self.disabled.contains(p.name()),
            })
            .collect()
    }

    /// Collect [`ToolDefinition`]s from all **enabled** plugins.
    pub fn get_tools(&self) -> Vec<ToolDefinition> {
        self.plugins
            .iter()
            .filter(|p| !self.disabled.contains(p.name()))
            .flat_map(|p| p.tools())
            .collect()
    }

    /// Collect [`PluginCommand`]s from all **enabled** plugins.
    pub fn get_commands(&self) -> Vec<PluginCommand> {
        self.plugins
            .iter()
            .filter(|p| !self.disabled.contains(p.name()))
            .flat_map(|p| p.commands())
            .collect()
    }

    // ------------------------------------------------------------------
    // Enable / disable
    // ------------------------------------------------------------------

    /// Enable a previously disabled plugin.
    ///
    /// Returns `true` if the plugin was found, `false` otherwise.
    pub fn enable(&mut self, name: &str) -> bool {
        let found = self.plugins.iter().any(|p| p.name() == name);
        if found {
            self.disabled.remove(name);
            persist_state(self);
        }
        found
    }

    /// Disable a plugin by name.
    ///
    /// Returns `true` if the plugin was found, `false` otherwise.
    pub fn disable(&mut self, name: &str) -> bool {
        let found = self.plugins.iter().any(|p| p.name() == name);
        if found {
            self.disabled.insert(name.to_string());
            persist_state(self);
        }
        found
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        for plugin in &self.plugins {
            plugin.on_unload();
        }
    }
}

// ---------------------------------------------------------------------------
// State persistence helpers
// ---------------------------------------------------------------------------

/// Path to the plugin state file: `.synapse/plugins/state.json` (relative to CWD).
fn state_file_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".synapse")
        .join("plugins")
        .join("state.json")
}

/// Serialise the current disabled-set to `.synapse/plugins/state.json`.
fn persist_state(mgr: &PluginManager) {
    let path = state_file_path();

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %err, "failed to create plugin state directory");
            return;
        }
    }

    // Build a sorted list for deterministic output.
    let mut disabled: Vec<&str> = mgr.disabled.iter().map(String::as_str).collect();
    disabled.sort_unstable();

    let json = serde_json::json!({ "disabled": disabled });
    if let Ok(contents) = serde_json::to_string_pretty(&json) {
        if let Err(err) = std::fs::write(&path, contents) {
            tracing::warn!(path = %path.display(), error = %err, "failed to write plugin state");
        }
    }
}

// ---------------------------------------------------------------------------
// Dynamic plugin loading (requires `plugins` feature + libloading)
// ---------------------------------------------------------------------------

/// Find a shared library file in a plugin directory.
#[cfg(feature = "plugins")]
fn find_shared_library(dir: &Path) -> Option<std::path::PathBuf> {
    let extensions: &[&str] = if cfg!(target_os = "macos") {
        &["dylib", "so"]
    } else if cfg!(target_os = "windows") {
        &["dll"]
    } else {
        &["so"]
    };

    for ext in extensions {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some(ext) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Load a plugin from a shared library.
///
/// The library must export a `synapse_plugin_create` symbol that returns
/// `*mut dyn Plugin`. The function signature must be:
///
/// ```c
/// extern "C" fn synapse_plugin_create() -> *mut dyn Plugin;
/// ```
#[cfg(feature = "plugins")]
fn load_dynamic_plugin(lib_path: &Path) -> Result<Box<dyn Plugin>, Box<dyn std::error::Error>> {
    unsafe {
        let lib = libloading::Library::new(lib_path)?;
        let create_fn: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> =
            lib.get(b"synapse_plugin_create")?;
        let plugin = Box::from_raw(create_fn());

        // Call on_load
        plugin.on_load()?;

        // Leak the library handle so it stays loaded for the lifetime of the process
        std::mem::forget(lib);

        Ok(plugin)
    }
}

/// Load the disabled-set from `.synapse/plugins/state.json` if it exists.
#[allow(dead_code)]
pub fn load_state() -> HashSet<String> {
    let path = state_file_path();
    if !path.exists() {
        return HashSet::new();
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return HashSet::new(),
    };

    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        disabled: Vec<String>,
    }

    match serde_json::from_str::<State>(&contents) {
        Ok(state) => state.disabled.into_iter().collect(),
        Err(_) => HashSet::new(),
    }
}

// ---------------------------------------------------------------------------
// Builtin plugin registration
// ---------------------------------------------------------------------------

/// Register all builtin plugins into a [`PluginRegistry`].
///
/// This wires the core event subscribers — tracing, thinking, loop detection,
/// and cost tracking — into the framework's event bus and records their
/// manifests so the dashboard and `/api/plugins` endpoints can surface them.
pub fn register_builtin_plugins(
    registry: &mut PluginRegistry,
    cost_tracker: Arc<synaptic::callbacks::CostTrackingCallback>,
    usage_tracker: Arc<crate::gateway::usage::UsageTracker>,
) -> Result<(), Box<dyn std::error::Error>> {
    // --- 1. Tracing subscriber ---
    registry.register_event_subscriber(
        Arc::new(crate::agent::subscribers::TracingSubscriber::new()),
        -80,
        "builtin:tracing",
    );
    registry.record_plugin(PluginManifest {
        name: "builtin-tracing".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Agent tracing and latency measurement".into(),
        author: Some("synapse".into()),
        license: None,
        capabilities: vec![PluginCapability::Hooks],
    });

    // --- 2. Thinking subscriber (no fixed config — adaptive is set separately
    //        via ThinkingSubscriber::adaptive(); use None for default pass-through) ---
    registry.register_event_subscriber(
        Arc::new(crate::agent::subscribers::ThinkingSubscriber::new(None)),
        -70,
        "builtin:thinking",
    );
    registry.record_plugin(PluginManifest {
        name: "builtin-thinking".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Extended thinking configuration".into(),
        author: Some("synapse".into()),
        license: None,
        capabilities: vec![PluginCapability::Hooks],
    });

    // --- 3. Loop detection subscriber (max 3 consecutive identical tool-call hashes) ---
    registry.register_event_subscriber(
        Arc::new(crate::agent::subscribers::LoopDetectionSubscriber::new(3)),
        -85,
        "builtin:loop-detection",
    );
    registry.record_plugin(PluginManifest {
        name: "builtin-loop-detection".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Detect and break agent execution loops".into(),
        author: Some("synapse".into()),
        license: None,
        capabilities: vec![PluginCapability::Hooks],
    });

    // --- 4. Cost tracking subscriber (records usage to UsageTracker via EventBus) ---
    registry.register_event_subscriber(
        Arc::new(crate::agent::subscribers::CostTrackingSubscriber::new(
            cost_tracker,
            usage_tracker,
        )),
        -60,
        "builtin:cost-tracking",
    );
    registry.record_plugin(PluginManifest {
        name: "builtin-cost-tracking".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Token usage and cost tracking via EventBus".into(),
        author: Some("synapse".into()),
        license: None,
        capabilities: vec![PluginCapability::Hooks],
    });

    tracing::info!(
        count = registry.plugins().len(),
        "registered builtin plugins"
    );

    Ok(())
}
