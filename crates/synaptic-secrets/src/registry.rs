use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use synaptic_core::SynapticError;

struct SecretEntry {
    value: String,
    mask: String,
}

/// Registry for managing secrets that should be masked in AI outputs.
///
/// Secrets are registered with a name and value, and optionally a custom mask.
/// The registry can mask occurrences of secret values in text, and inject
/// secret values into templates using `{{secret:name}}` syntax.
pub struct SecretRegistry {
    secrets: Arc<RwLock<HashMap<String, SecretEntry>>>,
}

impl Default for SecretRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretRegistry {
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a secret with default mask `[REDACTED:name]`.
    pub fn register(&self, name: &str, value: &str) {
        let mask = format!("[REDACTED:{}]", name);
        self.register_with_mask(name, value, &mask);
    }

    /// Register a secret with a custom mask string.
    pub fn register_with_mask(&self, name: &str, value: &str, mask: &str) {
        let mut secrets = self.secrets.write().unwrap();
        secrets.insert(
            name.to_string(),
            SecretEntry {
                value: value.to_string(),
                mask: mask.to_string(),
            },
        );
    }

    /// Replace all secret values in the text with their masks.
    pub fn mask_output(&self, text: &str) -> String {
        let secrets = self.secrets.read().unwrap();
        let mut result = text.to_string();
        // Sort by value length descending to handle overlapping secrets
        let mut entries: Vec<_> = secrets.values().collect();
        entries.sort_by(|a, b| b.value.len().cmp(&a.value.len()));
        for entry in entries {
            if !entry.value.is_empty() {
                result = result.replace(&entry.value, &entry.mask);
            }
        }
        result
    }

    /// Inject secret values into a template string.
    ///
    /// Replaces `{{secret:name}}` patterns with the actual secret value.
    pub fn inject(&self, template: &str) -> Result<String, SynapticError> {
        let secrets = self.secrets.read().unwrap();
        let re = regex::Regex::new(r"\{\{secret:(\w+)\}\}")
            .map_err(|e| SynapticError::Config(format!("invalid regex: {}", e)))?;

        let mut result = template.to_string();
        for cap in re.captures_iter(template) {
            let full_match = &cap[0];
            let name = &cap[1];
            match secrets.get(name) {
                Some(entry) => {
                    result = result.replace(full_match, &entry.value);
                }
                None => {
                    return Err(SynapticError::Config(format!(
                        "secret '{}' not found in registry",
                        name
                    )));
                }
            }
        }
        Ok(result)
    }

    /// Remove a secret from the registry.
    pub fn remove(&self, name: &str) {
        let mut secrets = self.secrets.write().unwrap();
        secrets.remove(name);
    }
}
