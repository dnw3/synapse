use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use synaptic::condenser::ContextWindowResolver;
use synaptic::core::SynapticError;

/// Synapse-specific context window resolver with layered resolution:
///
/// 1. User config override (`context_window` in `[memory]`)
/// 2. Dynamic discovery cache (future: populated from model APIs)
/// 3. Static mapping for known providers/models
/// 4. Conservative fallback (128k)
pub struct SynapseContextWindowResolver {
    config_override: usize,
    context_1m: bool,
    discovery_cache: RwLock<HashMap<String, usize>>,
    fallback: usize,
}

impl SynapseContextWindowResolver {
    pub fn new(config_override: usize, context_1m: bool) -> Self {
        Self {
            config_override,
            context_1m,
            discovery_cache: RwLock::new(HashMap::new()),
            fallback: 128_000,
        }
    }

    fn static_mapping(&self, model: &str, provider: &str) -> Option<usize> {
        // Anthropic 1M (requires opt-in via context_1m config flag)
        if self.context_1m && provider == "anthropic" {
            let lower = model.to_lowercase();
            if lower.contains("opus") || lower.contains("sonnet") {
                return Some(1_048_576);
            }
        }

        // Standard Anthropic
        if provider == "anthropic" {
            return Some(200_000);
        }

        // OpenAI
        if provider == "openai" {
            if model.contains("o1") || model.contains("o3") {
                return Some(200_000);
            }
            return Some(128_000);
        }

        // Ark / Doubao
        if provider == "ark" {
            return Some(128_000);
        }

        None
    }
}

#[async_trait]
impl ContextWindowResolver for SynapseContextWindowResolver {
    fn resolve(&self, model: &str, provider: &str) -> usize {
        // Layer 1: user config override
        if self.config_override > 0 {
            return self.config_override;
        }

        // Layer 2: dynamic discovery cache
        if let Some(&v) = self
            .discovery_cache
            .read()
            .expect("cache poisoned")
            .get(model)
        {
            return v;
        }

        // Layer 3: static mapping
        if let Some(v) = self.static_mapping(model, provider) {
            return v;
        }

        // Layer 4: conservative fallback
        self.fallback
    }

    async fn discover(&self) -> Result<(), SynapticError> {
        // Future: call model APIs to discover context window sizes
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_override_takes_precedence() {
        let resolver = SynapseContextWindowResolver::new(256_000, false);
        assert_eq!(
            resolver.resolve("claude-sonnet-4-20250514", "anthropic"),
            256_000
        );
    }

    #[test]
    fn anthropic_standard() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(
            resolver.resolve("claude-sonnet-4-20250514", "anthropic"),
            200_000
        );
    }

    #[test]
    fn anthropic_1m_opt_in() {
        let resolver = SynapseContextWindowResolver::new(0, true);
        assert_eq!(
            resolver.resolve("claude-sonnet-4-20250514", "anthropic"),
            1_048_576
        );
        assert_eq!(
            resolver.resolve("claude-opus-4-20250514", "anthropic"),
            1_048_576
        );
    }

    #[test]
    fn anthropic_1m_not_opted_in() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(
            resolver.resolve("claude-opus-4-20250514", "anthropic"),
            200_000
        );
    }

    #[test]
    fn openai_standard() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(resolver.resolve("gpt-4o", "openai"), 128_000);
    }

    #[test]
    fn openai_reasoning() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(resolver.resolve("o1-preview", "openai"), 200_000);
        assert_eq!(resolver.resolve("o3-mini", "openai"), 200_000);
    }

    #[test]
    fn ark_provider() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(resolver.resolve("doubao-pro", "ark"), 128_000);
    }

    #[test]
    fn unknown_provider_fallback() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        assert_eq!(resolver.resolve("some-model", "some-provider"), 128_000);
    }

    #[test]
    fn discovery_cache_used() {
        let resolver = SynapseContextWindowResolver::new(0, false);
        resolver
            .discovery_cache
            .write()
            .unwrap()
            .insert("custom-model".to_string(), 500_000);
        assert_eq!(resolver.resolve("custom-model", "custom"), 500_000);
    }
}
