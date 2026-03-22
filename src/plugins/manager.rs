//! PluginManager — discovers, loads, and manages plugin lifecycle in Synapse.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use synaptic::plugin::{Plugin, PluginApi, PluginContext, PluginManifest, PluginRegistry};

use super::config::PluginsConfig;

#[allow(dead_code)]
pub struct PluginManager {
    config: PluginsConfig,
    registry: Arc<RwLock<PluginRegistry>>,
    builtins: Vec<Box<dyn Plugin>>,
    disabled: Vec<String>,
    data_root: PathBuf,
    /// Skills dirs contributed by bundle plugins.
    pub bundle_skills_dirs: Vec<PathBuf>,
    /// Agent dirs contributed by bundle plugins.
    pub bundle_agent_dirs: Vec<PathBuf>,
    /// Active external plugin bridges (kept alive for tool calls).
    bridges: Vec<Arc<super::bridge::ExternalPluginBridge>>,
}

#[allow(dead_code)]
impl PluginManager {
    pub fn new(
        config: PluginsConfig,
        registry: Arc<RwLock<PluginRegistry>>,
        data_root: PathBuf,
    ) -> Self {
        Self {
            config,
            registry,
            builtins: Vec::new(),
            disabled: Vec::new(),
            data_root,
            bundle_skills_dirs: Vec::new(),
            bundle_agent_dirs: Vec::new(),
            bridges: Vec::new(),
        }
    }

    /// Register a builtin (compiled-in) plugin.
    pub fn add_builtin(&mut self, plugin: Box<dyn Plugin>) {
        self.builtins.push(plugin);
    }

    /// Load disabled state from data_root/state.json.
    pub fn load_state(&mut self) {
        let state_path = self.data_root.join("state.json");
        if let Ok(data) = std::fs::read_to_string(&state_path) {
            if let Ok(state) = serde_json::from_str::<PluginState>(&data) {
                self.disabled = state.disabled;
            }
        }
    }

    /// Save disabled state.
    pub fn save_state(&self) -> std::io::Result<()> {
        let state_path = self.data_root.join("state.json");
        std::fs::create_dir_all(&self.data_root)?;
        let state = PluginState {
            disabled: self.disabled.clone(),
        };
        let json = serde_json::to_string_pretty(&state).map_err(std::io::Error::other)?;
        std::fs::write(&state_path, json)
    }

    /// Register all enabled builtins with the registry.
    pub async fn load_all(&mut self) -> Result<(), synaptic::core::SynapticError> {
        let builtins = std::mem::take(&mut self.builtins);

        for plugin in &builtins {
            let manifest = plugin.manifest();

            if !self.is_allowed(&manifest.name) {
                tracing::info!(plugin = %manifest.name, "plugin skipped (deny/allow list)");
                continue;
            }
            if self.disabled.contains(&manifest.name) {
                tracing::info!(plugin = %manifest.name, "plugin skipped (disabled)");
                continue;
            }
            if let Some(entry) = self.config.entries.get(&manifest.name) {
                if !entry.enabled {
                    tracing::info!(plugin = %manifest.name, "plugin skipped (config disabled)");
                    continue;
                }
            }

            tracing::info!(plugin = %manifest.name, version = %manifest.version, "loading plugin");

            let mut registry = self.registry.write().await;
            {
                let mut api = PluginApi::new(&mut registry, &manifest.name);
                plugin.register(&mut api).await?;
            }
            registry.record_plugin(manifest.clone());

            let data_dir = self.data_root.join(&manifest.name);
            std::fs::create_dir_all(&data_dir).ok();
            plugin.start(PluginContext { data_dir }).await?;
        }

        self.builtins = builtins;
        Ok(())
    }

    /// Discover and load filesystem plugins from default paths.
    pub async fn discover_and_load(&mut self) -> Result<(), synaptic::core::SynapticError> {
        let paths = super::discovery::default_plugin_paths();

        for dir in &paths {
            let discovered = super::discovery::discover_plugins(dir);
            for plugin in discovered {
                let plugin_id = plugin
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                if !self.is_allowed(&plugin_id) {
                    tracing::info!(plugin = %plugin_id, "plugin skipped (deny/allow list)");
                    continue;
                }

                match plugin.format {
                    super::discovery::PluginFormat::SynapseNative => {
                        tracing::info!(path = %plugin.path.display(), "native plugin found (compiled-in only)");
                        // Native plugins must be compiled in. Filesystem discovery is
                        // informational only until dylib support is added.
                    }
                    super::discovery::PluginFormat::OpenClaw => {
                        let config = self
                            .config
                            .entries
                            .get(&plugin_id)
                            .map(|e| e.config.clone())
                            .unwrap_or(serde_json::Value::Null);

                        match super::bridge::ExternalPluginBridge::spawn(&plugin.path, &config)
                            .await
                        {
                            Ok(bridge) => {
                                let tool_count = bridge.tool_defs.len();
                                // Register tools from bridge
                                {
                                    let mut registry = self.registry.write().await;
                                    for tool in bridge.tools() {
                                        registry.register_tool(tool);
                                    }
                                    registry.record_plugin(synaptic::plugin::PluginManifest {
                                        name: bridge.id.clone(),
                                        version: "0.0.0".into(),
                                        description: format!("OpenClaw plugin: {}", bridge.id),
                                        author: None,
                                        license: None,
                                        capabilities: vec![
                                            synaptic::plugin::PluginCapability::Tools,
                                        ],
                                        slot: None,
                                    });
                                }
                                self.bridges.push(bridge);
                                tracing::info!(
                                    plugin = %plugin_id,
                                    tools = tool_count,
                                    "OpenClaw plugin loaded"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    plugin = %plugin_id,
                                    error = %e,
                                    "failed to load OpenClaw plugin"
                                );
                            }
                        }
                    }
                    ref fmt @ (super::discovery::PluginFormat::ClaudeBundle
                    | super::discovery::PluginFormat::CodexBundle
                    | super::discovery::PluginFormat::CursorBundle) => {
                        if let Some(content) = super::bundle::load_bundle(&plugin.path, fmt) {
                            let skills = content.skills_dirs.len();
                            let agents = content.agent_dirs.len();
                            self.bundle_skills_dirs.extend(content.skills_dirs);
                            self.bundle_agent_dirs.extend(content.agent_dirs);
                            tracing::info!(
                                plugin = %content.id,
                                skills_dirs = skills,
                                agent_dirs = agents,
                                "bundle loaded"
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn is_allowed(&self, name: &str) -> bool {
        if let Some(ref deny) = self.config.deny {
            if deny.iter().any(|d| d == name) {
                return false;
            }
        }
        if let Some(ref allow) = self.config.allow {
            return allow.iter().any(|a| a == name);
        }
        true
    }

    pub fn disable(&mut self, name: &str) {
        if !self.disabled.contains(&name.to_string()) {
            self.disabled.push(name.to_string());
            self.save_state().ok();
        }
    }

    pub fn enable(&mut self, name: &str) {
        self.disabled.retain(|d| d != name);
        self.save_state().ok();
    }

    pub async fn list(&self) -> Vec<PluginManifest> {
        self.registry.read().await.plugins().to_vec()
    }

    /// Start all registered services.
    pub async fn start_services(&self) {
        let registry = self.registry.read().await;
        for service in registry.services() {
            if let Err(e) = service.start().await {
                tracing::error!(service = service.id(), error = %e, "failed to start service");
            }
        }
    }

    /// Stop all registered services (reverse order).
    pub async fn stop_services(&self) {
        let registry = self.registry.read().await;
        for service in registry.services().iter().rev() {
            tracing::info!(service = service.id(), "stopping service");
            service.stop().await;
        }
    }

    /// Stop all plugins (reverse order).
    pub async fn stop_all(&self) {
        self.stop_services().await;
        for bridge in &self.bridges {
            bridge.shutdown().await;
        }
        for plugin in self.builtins.iter().rev() {
            plugin.stop().await.ok();
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
#[allow(dead_code)]
struct PluginState {
    #[serde(default)]
    disabled: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use synaptic::events::EventBus;
    use synaptic::plugin::PluginCapability;

    struct DummyPlugin;

    #[async_trait::async_trait]
    impl Plugin for DummyPlugin {
        fn manifest(&self) -> PluginManifest {
            PluginManifest {
                name: "dummy".into(),
                version: "0.1.0".into(),
                description: "A dummy plugin for testing".into(),
                author: None,
                license: None,
                capabilities: vec![PluginCapability::Tools],
                slot: None,
            }
        }

        async fn register(
            &self,
            _api: &mut PluginApi<'_>,
        ) -> Result<(), synaptic::core::SynapticError> {
            Ok(())
        }
    }

    fn make_registry() -> Arc<RwLock<PluginRegistry>> {
        let bus = Arc::new(EventBus::new());
        Arc::new(RwLock::new(PluginRegistry::new(bus)))
    }

    #[tokio::test]
    async fn load_enabled_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let registry = make_registry();
        let config = PluginsConfig::default();

        let mut mgr = PluginManager::new(config, registry.clone(), dir.path().to_path_buf());
        mgr.add_builtin(Box::new(DummyPlugin));
        mgr.load_all().await.unwrap();

        let plugins = mgr.list().await;
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "dummy");
    }

    #[tokio::test]
    async fn deny_list_blocks_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let registry = make_registry();
        let config = PluginsConfig {
            deny: Some(vec!["dummy".into()]),
            ..Default::default()
        };

        let mut mgr = PluginManager::new(config, registry.clone(), dir.path().to_path_buf());
        mgr.add_builtin(Box::new(DummyPlugin));
        mgr.load_all().await.unwrap();

        let plugins = mgr.list().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn disable_enable_persists() {
        let dir = tempfile::tempdir().unwrap();
        let data_root = dir.path().to_path_buf();

        // Disable a plugin and verify state persists
        {
            let registry = make_registry();
            let config = PluginsConfig::default();
            let mut mgr = PluginManager::new(config, registry, data_root.clone());
            mgr.disable("dummy");
            assert!(mgr.disabled.contains(&"dummy".to_string()));
        }

        // Create new manager with same data_root, load_state, verify disabled persists
        {
            let registry = make_registry();
            let config = PluginsConfig::default();
            let mut mgr = PluginManager::new(config, registry, data_root.clone());
            mgr.load_state();
            assert!(mgr.disabled.contains(&"dummy".to_string()));

            // Enable it and verify removed
            mgr.enable("dummy");
            assert!(!mgr.disabled.contains(&"dummy".to_string()));
        }

        // Verify enable persisted
        {
            let registry = make_registry();
            let config = PluginsConfig::default();
            let mut mgr = PluginManager::new(config, registry, data_root.clone());
            mgr.load_state();
            assert!(!mgr.disabled.contains(&"dummy".to_string()));
        }
    }
}
