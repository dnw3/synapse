//! Builtin plugin factory registry.
//!
//! Each builtin plugin registers a factory function that creates a `Plugin`
//! instance from a JSON config value. The gateway looks up by name — no match
//! arms, no hardcoding. Adding a new plugin = one `register()` call.

use std::collections::HashMap;

use synaptic::plugin::Plugin;

/// Factory function: takes plugin config (from `[plugins.entries.xxx].config`)
/// and returns a boxed Plugin instance.
pub type PluginFactory = fn(config: serde_json::Value) -> Box<dyn Plugin>;

/// Registry of builtin plugin factories, keyed by plugin name.
pub struct BuiltinPluginRegistry {
    factories: HashMap<String, PluginFactory>,
}

impl BuiltinPluginRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a builtin plugin factory.
    pub fn register(&mut self, name: &str, factory: PluginFactory) {
        self.factories.insert(name.to_string(), factory);
    }

    /// Create a plugin instance by name + config. Returns None if not found.
    pub fn create(&self, name: &str, config: serde_json::Value) -> Option<Box<dyn Plugin>> {
        self.factories.get(name).map(|f| f(config))
    }

    /// List all registered plugin names.
    pub fn names(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
}

/// Build the default builtin plugin registry with all known plugins.
pub fn default_registry() -> BuiltinPluginRegistry {
    let mut reg = BuiltinPluginRegistry::new();

    reg.register("memory-native", |_config| {
        // TODO: pass LTM config through when LTM creation is decoupled
        Box::new(super::memory_native::NativeMemoryPlugin::new(None))
    });

    reg.register("memory-viking", |config| {
        let viking_config: crate::memory::VikingConfig =
            serde_json::from_value(config).unwrap_or_default();
        Box::new(super::memory_viking::VikingMemoryPlugin::new(viking_config))
    });

    // Future plugins just add one line here:
    // reg.register("memory-lancedb", |config| { ... });

    reg
}
