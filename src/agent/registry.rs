use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use synaptic::core::ChatModel;
use synaptic::models::HttpBackend;
use synaptic::openai::{OpenAiChatModel, OpenAiConfig};

use crate::config::{ChannelModelBinding, ModelEntry, ProviderEntry, SynapseConfig};

/// Round-robin key rotator for providers with multiple API keys.
struct KeyRotator {
    keys: Vec<String>,
    index: AtomicUsize,
}

impl KeyRotator {
    fn new(keys: Vec<String>) -> Self {
        Self {
            keys,
            index: AtomicUsize::new(0),
        }
    }

    /// Get the next API key in round-robin order.
    fn next_key(&self) -> &str {
        let idx = self.index.fetch_add(1, Ordering::Relaxed) % self.keys.len();
        &self.keys[idx]
    }

    /// Get all keys (for building fallback instances).
    fn all_keys(&self) -> &[String] {
        &self.keys
    }
}

/// Central model registry — resolves model names/aliases to ChatModel instances.
///
/// Built from `[[models]]`, `[[providers]]`, and `[[channel_models]]` config sections.
/// Falls through to the hardcoded provider detection when a model isn't in the catalog.
#[allow(dead_code)]
pub struct ModelRegistry {
    /// name → ModelEntry (canonical names only)
    catalog: HashMap<String, ModelEntry>,
    /// alias → canonical name
    alias_map: HashMap<String, String>,
    /// provider name → ProviderEntry
    providers: HashMap<String, ProviderEntry>,
    /// provider name → KeyRotator (only for providers with api_keys_env)
    key_rotators: HashMap<String, KeyRotator>,
    /// Channel-level model bindings
    channel_bindings: Vec<ChannelModelBinding>,
    /// Reference to the full config (for fallback api_key resolution)
    config: SynapseConfig,
}

impl ModelRegistry {
    /// Build a registry from the config. Cheap (microsecond-level HashMap construction).
    pub fn from_config(config: &SynapseConfig) -> Self {
        let mut catalog = HashMap::new();
        let mut alias_map = HashMap::new();
        let mut providers = HashMap::new();
        let mut key_rotators = HashMap::new();

        // Index providers
        if let Some(ref entries) = config.provider_catalog {
            for p in entries {
                // Try to resolve multi-key rotation
                if let Some(ref env_name) = p.api_keys_env {
                    if let Ok(val) = std::env::var(env_name) {
                        let keys: Vec<String> = val
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if !keys.is_empty() {
                            key_rotators.insert(p.name.clone(), KeyRotator::new(keys));
                        }
                    }
                }
                providers.insert(p.name.clone(), p.clone());
            }
        }

        // Index models and aliases
        if let Some(ref entries) = config.model_catalog {
            for m in entries {
                catalog.insert(m.name.clone(), m.clone());
                for alias in &m.aliases {
                    alias_map.insert(alias.clone(), m.name.clone());
                }
            }
        }

        let channel_bindings = config.channel_model_bindings.clone().unwrap_or_default();

        Self {
            catalog,
            alias_map,
            providers,
            key_rotators,
            channel_bindings,
            config: config.clone(),
        }
    }

    /// Check if a name or alias exists in the catalog.
    pub fn contains(&self, name_or_alias: &str) -> bool {
        self.catalog.contains_key(name_or_alias) || self.alias_map.contains_key(name_or_alias)
    }

    /// Get the canonical model name for a name or alias.
    pub fn canonical_name<'a>(&'a self, name_or_alias: &'a str) -> Option<&'a str> {
        if self.catalog.contains_key(name_or_alias) {
            Some(name_or_alias)
        } else {
            self.alias_map.get(name_or_alias).map(|s| s.as_str())
        }
    }

    /// Resolve a model name or alias to a ChatModel instance.
    pub fn resolve(&self, name_or_alias: &str) -> crate::error::Result<Arc<dyn ChatModel>> {
        let canonical = self
            .canonical_name(name_or_alias)
            .ok_or_else(|| format!("model '{}' not found in catalog", name_or_alias))?;
        let entry = &self.catalog[canonical];
        self.build_from_entry(entry, None)
    }

    /// Resolve the model for a given channel identifier (e.g. "telegram:12345").
    ///
    /// Matching priority: exact "platform:channel_id" → platform wildcard "platform:*" → None.
    #[allow(dead_code)]
    pub fn resolve_for_channel(
        &self,
        channel_id: &str,
    ) -> Option<crate::error::Result<Arc<dyn ChatModel>>> {
        // Exact match first
        let binding = self
            .channel_bindings
            .iter()
            .find(|b| b.channel == channel_id);

        // Platform wildcard fallback
        let binding = binding.or_else(|| {
            let platform = channel_id.split(':').next()?;
            let wildcard = format!("{}:*", platform);
            self.channel_bindings.iter().find(|b| b.channel == wildcard)
        });

        binding.map(|b| {
            if self.contains(&b.model) {
                self.resolve(&b.model)
            } else {
                // Fallback: try building via hardcoded logic
                super::model::build_model_by_name(&self.config, &b.model)
            }
        })
    }

    /// List all model entries in the catalog.
    pub fn list(&self) -> Vec<&ModelEntry> {
        self.catalog.values().collect()
    }

    /// List all alias → canonical name mappings.
    pub fn aliases(&self) -> Vec<(&str, &str)> {
        self.alias_map
            .iter()
            .map(|(alias, canonical)| (alias.as_str(), canonical.as_str()))
            .collect()
    }

    /// Get the provider entry for a model, if any.
    pub fn provider_for(&self, model_name: &str) -> Option<&ProviderEntry> {
        let canonical = self.canonical_name(model_name)?;
        let entry = self.catalog.get(canonical)?;
        let prov_name = entry.provider.as_ref()?;
        self.providers.get(prov_name)
    }

    /// Build additional fallback instances using different API keys from the same provider.
    /// Returns None if the provider only has a single key.
    pub fn rotation_fallbacks(&self, model_name: &str) -> Option<Vec<Arc<dyn ChatModel>>> {
        let canonical = self.canonical_name(model_name)?;
        let entry = self.catalog.get(canonical)?;
        let prov_name = entry.provider.as_ref()?;
        let rotator = self.key_rotators.get(prov_name)?;

        let keys = rotator.all_keys();
        if keys.len() <= 1 {
            return None;
        }

        let mut fallbacks = Vec::new();
        // Skip index 0 (that's the primary), build fallbacks from remaining keys
        for key in keys.iter().skip(1) {
            if let Ok(model) = self.build_from_entry(entry, Some(key)) {
                fallbacks.push(model);
            }
        }

        if fallbacks.is_empty() {
            None
        } else {
            Some(fallbacks)
        }
    }

    /// Build a ChatModel from a catalog entry, optionally overriding the API key.
    fn build_from_entry(
        &self,
        entry: &ModelEntry,
        key_override: Option<&str>,
    ) -> crate::error::Result<Arc<dyn ChatModel>> {
        let http = Arc::new(HttpBackend::new());

        // Resolve API key
        let api_key = if let Some(key) = key_override {
            key.to_string()
        } else if let Some(ref prov_name) = entry.provider {
            // Try key rotation first
            if let Some(rotator) = self.key_rotators.get(prov_name) {
                rotator.next_key().to_string()
            } else if let Some(prov) = self.providers.get(prov_name) {
                // Single key from provider config
                if let Some(ref env_name) = prov.api_key_env {
                    std::env::var(env_name).unwrap_or_default()
                } else {
                    self.config.resolve_api_key().unwrap_or_default()
                }
            } else {
                self.config.resolve_api_key().unwrap_or_default()
            }
        } else {
            self.config.resolve_api_key().unwrap_or_default()
        };

        let mut oai_config = OpenAiConfig::new(&api_key, &entry.name);

        // Set base_url from provider
        if let Some(ref prov_name) = entry.provider {
            if let Some(prov) = self.providers.get(prov_name) {
                oai_config = oai_config.with_base_url(&prov.base_url);
            }
        }

        // Apply per-model parameter overrides
        if let Some(temp) = entry.temperature {
            oai_config = oai_config.with_temperature(temp);
        }
        if let Some(max) = entry.max_tokens {
            oai_config = oai_config.with_max_tokens(max);
        }

        Ok(Arc::new(OpenAiChatModel::new(oai_config, http)))
    }
}
