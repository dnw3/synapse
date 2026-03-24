use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use async_trait::async_trait;
use synaptic::condenser::ContextWindowResolver;
use synaptic::core::SynapticError;

// ---------------------------------------------------------------------------
// Built-in models catalog — known context windows for major providers
// ---------------------------------------------------------------------------

/// (provider, model_prefix, context_window)
const BUILTIN_CATALOG: &[(&str, &str, usize)] = &[
    // Anthropic
    ("anthropic", "claude-opus-4", 200_000),
    ("anthropic", "claude-sonnet-4", 200_000),
    ("anthropic", "claude-haiku-4", 200_000),
    ("anthropic", "claude-3.5-sonnet", 200_000),
    ("anthropic", "claude-3.5-haiku", 200_000),
    ("anthropic", "claude-3-opus", 200_000),
    ("anthropic", "claude-3-sonnet", 200_000),
    ("anthropic", "claude-3-haiku", 200_000),
    // OpenAI
    ("openai", "gpt-4o", 128_000),
    ("openai", "gpt-4o-mini", 128_000),
    ("openai", "gpt-4-turbo", 128_000),
    ("openai", "gpt-4", 8_192),
    ("openai", "gpt-3.5-turbo", 16_385),
    ("openai", "o1", 200_000),
    ("openai", "o1-mini", 128_000),
    ("openai", "o1-preview", 128_000),
    ("openai", "o3", 200_000),
    ("openai", "o3-mini", 200_000),
    ("openai", "o4-mini", 200_000),
    // Google Gemini
    ("gemini", "gemini-2.5-pro", 1_048_576),
    ("gemini", "gemini-2.5-flash", 1_048_576),
    ("gemini", "gemini-2.0-flash", 1_048_576),
    ("gemini", "gemini-1.5-pro", 2_097_152),
    ("gemini", "gemini-1.5-flash", 1_048_576),
    ("google", "gemini-2.5-pro", 1_048_576),
    ("google", "gemini-2.5-flash", 1_048_576),
    ("google", "gemini-2.0-flash", 1_048_576),
    // DeepSeek
    ("deepseek", "deepseek-chat", 64_000),
    ("deepseek", "deepseek-reasoner", 64_000),
    ("deepseek", "deepseek-v3", 64_000),
    // Mistral
    ("mistral", "mistral-large", 128_000),
    ("mistral", "mistral-medium", 32_000),
    ("mistral", "mistral-small", 32_000),
    ("mistral", "codestral", 256_000),
    // xAI
    ("xai", "grok-3", 131_072),
    ("xai", "grok-2", 131_072),
    // Together
    ("together", "meta-llama/Llama-3.3-70B", 128_000),
    ("together", "meta-llama/Llama-4-Scout", 512_000),
    ("together", "meta-llama/Llama-4-Maverick", 1_048_576),
    // Ark / Doubao (generic fallback; name-based parsing is preferred)
    ("ark", "doubao-1.5-lite", 32_000),
    ("ark", "doubao-1.5-plus", 32_000),
    ("ark", "doubao-1.5-max", 256_000),
    ("ark", "doubao-seed", 128_000),
    ("ark", "doubao-pro", 128_000),
];

/// Lookup from built-in catalog using prefix matching.
fn catalog_lookup(provider: &str, model: &str) -> Option<usize> {
    let model_lower = model.to_lowercase();
    // Prefer longest prefix match
    let mut best: Option<usize> = None;
    let mut best_len = 0;
    for &(p, prefix, window) in BUILTIN_CATALOG {
        if p == provider && model_lower.starts_with(prefix) && prefix.len() > best_len {
            best = Some(window);
            best_len = prefix.len();
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Ark model name parsing — extract context window from name like "doubao-1.5-max-256k"
// ---------------------------------------------------------------------------

/// Parse context window size from Ark/Doubao model name suffix.
/// e.g. "doubao-1.5-max-256k" → 256_000, "ep-20260125170616-c6dnz" → None
fn parse_ark_context_window(model: &str) -> Option<usize> {
    let lower = model.to_lowercase();
    // Look for pattern like "-32k", "-128k", "-256k", "-1m" at end or before version suffix
    for part in lower.split('-').rev() {
        if let Some(k) = part.strip_suffix('k') {
            if let Ok(n) = k.parse::<usize>() {
                return Some(n * 1_000);
            }
        }
        if let Some(m) = part.strip_suffix('m') {
            if let Ok(n) = m.parse::<usize>() {
                return Some(n * 1_000_000);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// User models.json — ~/.synapse/models.json
// ---------------------------------------------------------------------------

/// Load user-defined context windows from ~/.synapse/models.json.
/// Format: { "provider::model_prefix": context_window, ... }
/// e.g. { "ark::my-custom-endpoint": 256000, "anthropic::claude-opus-4": 1048576 }
fn load_user_models_json() -> HashMap<String, usize> {
    let path = user_models_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    match serde_json::from_str::<HashMap<String, serde_json::Value>>(&content) {
        Ok(map) => {
            let mut result = HashMap::new();
            for (key, val) in map {
                if let Some(n) = val.as_u64() {
                    result.insert(key, n as usize);
                }
            }
            if !result.is_empty() {
                tracing::info!(count = result.len(), path = %path.display(), "loaded user models.json overrides");
            }
            result
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), %e, "failed to parse models.json");
            HashMap::new()
        }
    }
}

fn user_models_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse")
        .join("models.json")
}

// ---------------------------------------------------------------------------
// SynapseContextWindowResolver
// ---------------------------------------------------------------------------

/// Synapse-specific context window resolver with 5-layer resolution:
///
/// 1. User config override (`context_window` in `[memory]`)
/// 2. User models.json (`~/.synapse/models.json`)
/// 3. Dynamic discovery cache (populated from model APIs)
/// 4. Built-in catalog + Ark model name parsing
/// 5. Conservative fallback (128k)
pub struct SynapseContextWindowResolver {
    config_override: usize,
    context_1m: bool,
    user_overrides: HashMap<String, usize>,
    discovery_cache: RwLock<HashMap<String, usize>>,
    fallback: usize,
}

impl SynapseContextWindowResolver {
    pub fn new(config_override: usize, context_1m: bool) -> Self {
        let user_overrides = load_user_models_json();
        Self {
            config_override,
            context_1m,
            user_overrides,
            discovery_cache: RwLock::new(HashMap::new()),
            fallback: 128_000,
        }
    }

    /// Resolve from built-in catalog + Ark name parsing + Anthropic 1M opt-in.
    fn builtin_resolve(&self, model: &str, provider: &str) -> Option<usize> {
        // Anthropic 1M opt-in (overrides standard 200K from catalog)
        if self.context_1m && provider == "anthropic" {
            let lower = model.to_lowercase();
            if lower.contains("opus") || lower.contains("sonnet") {
                return Some(1_048_576);
            }
        }

        // Ark: parse from model name first (e.g. "doubao-1.5-max-256k" → 256K)
        // Name parsing is more specific than catalog prefix match
        if provider == "ark" || provider == "doubao" {
            if let Some(v) = parse_ark_context_window(model) {
                return Some(v);
            }
        }

        // Built-in catalog (prefix match)
        if let Some(v) = catalog_lookup(provider, model) {
            return Some(v);
        }

        // Ark endpoint IDs (ep-xxxx) → conservative default
        if provider == "ark" || provider == "doubao" {
            return Some(128_000);
        }

        None
    }
}

#[async_trait]
impl ContextWindowResolver for SynapseContextWindowResolver {
    fn resolve(&self, model: &str, provider: &str) -> usize {
        // Layer 1: synapse.toml config override
        if self.config_override > 0 {
            return self.config_override;
        }

        // Layer 2: user models.json (provider::model prefix match)
        let key = format!("{provider}::{model}");
        if let Some(&v) = self.user_overrides.get(&key) {
            return v;
        }
        // Also try prefix match on user overrides
        for (k, &v) in &self.user_overrides {
            if key.starts_with(k) {
                return v;
            }
        }

        // Layer 3: dynamic discovery cache
        if let Some(&v) = self
            .discovery_cache
            .read()
            .expect("cache poisoned")
            .get(model)
        {
            return v;
        }

        // Layer 4: built-in catalog + Ark name parsing
        if let Some(v) = self.builtin_resolve(model, provider) {
            return v;
        }

        // Layer 5: conservative fallback
        self.fallback
    }

    async fn discover(&self) -> Result<(), SynapticError> {
        // Future: call provider APIs to discover context window sizes
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver(context_1m: bool) -> SynapseContextWindowResolver {
        // Bypass models.json loading in tests
        SynapseContextWindowResolver {
            config_override: 0,
            context_1m,
            user_overrides: HashMap::new(),
            discovery_cache: RwLock::new(HashMap::new()),
            fallback: 128_000,
        }
    }

    // -- Layer 1: config override --

    #[test]
    fn config_override_takes_precedence() {
        let r = SynapseContextWindowResolver {
            config_override: 256_000,
            ..resolver(false)
        };
        assert_eq!(r.resolve("claude-sonnet-4-20250514", "anthropic"), 256_000);
    }

    // -- Layer 2: user models.json --

    #[test]
    fn user_override_exact_match() {
        let mut r = resolver(false);
        r.user_overrides.insert("ark::my-endpoint".into(), 512_000);
        assert_eq!(r.resolve("my-endpoint", "ark"), 512_000);
    }

    #[test]
    fn user_override_prefix_match() {
        let mut r = resolver(false);
        r.user_overrides
            .insert("anthropic::claude-opus".into(), 999_000);
        assert_eq!(r.resolve("claude-opus-4-20250514", "anthropic"), 999_000);
    }

    // -- Layer 3: discovery cache --

    #[test]
    fn discovery_cache_used() {
        let r = resolver(false);
        r.discovery_cache
            .write()
            .unwrap()
            .insert("custom-model".into(), 500_000);
        assert_eq!(r.resolve("custom-model", "custom"), 500_000);
    }

    // -- Layer 4: built-in catalog --

    #[test]
    fn anthropic_standard() {
        let r = resolver(false);
        assert_eq!(r.resolve("claude-sonnet-4-20250514", "anthropic"), 200_000);
        assert_eq!(r.resolve("claude-opus-4-20250514", "anthropic"), 200_000);
        assert_eq!(
            r.resolve("claude-3.5-sonnet-20241022", "anthropic"),
            200_000
        );
    }

    #[test]
    fn anthropic_1m_opt_in() {
        let r = resolver(true);
        assert_eq!(
            r.resolve("claude-sonnet-4-20250514", "anthropic"),
            1_048_576
        );
        assert_eq!(r.resolve("claude-opus-4-20250514", "anthropic"), 1_048_576);
    }

    #[test]
    fn anthropic_1m_not_opted_in() {
        let r = resolver(false);
        assert_eq!(r.resolve("claude-opus-4-20250514", "anthropic"), 200_000);
    }

    #[test]
    fn openai_models() {
        let r = resolver(false);
        assert_eq!(r.resolve("gpt-4o", "openai"), 128_000);
        assert_eq!(r.resolve("gpt-4o-mini-2024", "openai"), 128_000);
        assert_eq!(r.resolve("gpt-4", "openai"), 8_192);
        assert_eq!(r.resolve("o1-preview", "openai"), 128_000);
        assert_eq!(r.resolve("o3-mini", "openai"), 200_000);
    }

    #[test]
    fn gemini_models() {
        let r = resolver(false);
        assert_eq!(r.resolve("gemini-2.5-pro-latest", "gemini"), 1_048_576);
        assert_eq!(r.resolve("gemini-2.5-flash", "google"), 1_048_576);
        assert_eq!(r.resolve("gemini-1.5-pro-latest", "gemini"), 2_097_152);
    }

    #[test]
    fn deepseek_models() {
        let r = resolver(false);
        assert_eq!(r.resolve("deepseek-chat", "deepseek"), 64_000);
        assert_eq!(r.resolve("deepseek-reasoner", "deepseek"), 64_000);
    }

    // -- Layer 4: Ark name parsing --

    #[test]
    fn ark_name_parsing() {
        let r = resolver(false);
        assert_eq!(r.resolve("doubao-1.5-max-256k", "ark"), 256_000);
        assert_eq!(r.resolve("doubao-1.5-lite-32k", "ark"), 32_000);
        assert_eq!(r.resolve("doubao-1.5-plus-32k", "ark"), 32_000);
    }

    #[test]
    fn ark_catalog_fallback() {
        let r = resolver(false);
        assert_eq!(r.resolve("doubao-seed-2-0-pro", "ark"), 128_000);
        // Name parsing wins over catalog: "-32k" suffix → 32K
        assert_eq!(r.resolve("doubao-pro-32k", "ark"), 32_000);
    }

    #[test]
    fn ark_endpoint_id_fallback() {
        let r = resolver(false);
        // Endpoint IDs like "ep-20260125170616-c6dnz" don't have window in name
        assert_eq!(r.resolve("ep-20260125170616-c6dnz", "ark"), 128_000);
    }

    // -- Layer 5: fallback --

    #[test]
    fn unknown_provider_fallback() {
        let r = resolver(false);
        assert_eq!(r.resolve("some-model", "some-provider"), 128_000);
    }

    // -- parse_ark_context_window unit tests --

    #[test]
    fn parse_ark_window_from_name() {
        assert_eq!(
            parse_ark_context_window("doubao-1.5-max-256k"),
            Some(256_000)
        );
        assert_eq!(
            parse_ark_context_window("doubao-1.5-lite-32k"),
            Some(32_000)
        );
        assert_eq!(parse_ark_context_window("some-model-1m"), Some(1_000_000));
        assert_eq!(parse_ark_context_window("ep-20260125170616-c6dnz"), None);
        assert_eq!(parse_ark_context_window("doubao-seed-2-0-pro"), None);
    }
}
