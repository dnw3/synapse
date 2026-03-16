use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct AuthProfile {
    pub name: String,
    pub provider: String,
    pub api_key: String,
    pub priority: i32,
}

#[derive(Debug)]
enum ProfileStatus {
    Active,
    RateLimited { until: Instant },
    Failed,
    Disabled,
}

struct ProfileEntry {
    profile: AuthProfile,
    status: RwLock<ProfileStatus>,
}

pub struct AuthProfileManager {
    profiles: Vec<ProfileEntry>,
}

impl AuthProfileManager {
    pub fn new(profiles: Vec<AuthProfile>) -> Self {
        let mut entries: Vec<_> = profiles
            .into_iter()
            .map(|p| ProfileEntry {
                profile: p,
                status: RwLock::new(ProfileStatus::Active),
            })
            .collect();
        entries.sort_by_key(|e| e.profile.priority);
        Self { profiles: entries }
    }

    /// Get the highest-priority active API key for a provider.
    pub fn resolve(&self, provider: &str) -> Option<&str> {
        for entry in &self.profiles {
            if entry.profile.provider != provider {
                continue;
            }
            let status = entry.status.read().unwrap();
            match &*status {
                ProfileStatus::Active => return Some(&entry.profile.api_key),
                ProfileStatus::RateLimited { until } if Instant::now() > *until => {
                    drop(status);
                    *entry.status.write().unwrap() = ProfileStatus::Active;
                    return Some(&entry.profile.api_key);
                }
                _ => continue,
            }
        }
        None
    }

    /// Mark an API key as rate-limited for the given cooldown duration.
    pub fn mark_rate_limited(&self, provider: &str, api_key: &str, cooldown: Duration) {
        for entry in &self.profiles {
            if entry.profile.provider == provider && entry.profile.api_key == api_key {
                *entry.status.write().unwrap() = ProfileStatus::RateLimited {
                    until: Instant::now() + cooldown,
                };
                tracing::warn!(
                    name = %entry.profile.name,
                    provider,
                    "API key rate limited, degrading"
                );
                break;
            }
        }
    }

    /// Mark an API key as permanently failed, removing it from rotation.
    pub fn mark_failed(&self, provider: &str, api_key: &str) {
        for entry in &self.profiles {
            if entry.profile.provider == provider && entry.profile.api_key == api_key {
                *entry.status.write().unwrap() = ProfileStatus::Failed;
                tracing::error!(
                    name = %entry.profile.name,
                    provider,
                    "API key failed, removing from rotation"
                );
                break;
            }
        }
    }

    /// Return the count of currently active profiles for a provider.
    pub fn active_count(&self, provider: &str) -> usize {
        self.profiles
            .iter()
            .filter(|e| e.profile.provider == provider)
            .filter(|e| matches!(&*e.status.read().unwrap(), ProfileStatus::Active))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn make_profile(name: &str, provider: &str, api_key: &str, priority: i32) -> AuthProfile {
        AuthProfile {
            name: name.to_string(),
            provider: provider.to_string(),
            api_key: api_key.to_string(),
            priority,
        }
    }

    #[test]
    fn resolve_returns_highest_priority() {
        let profiles = vec![
            make_profile("low", "openai", "key-low", 10),
            make_profile("high", "openai", "key-high", 1),
            make_profile("mid", "openai", "key-mid", 5),
        ];
        let manager = AuthProfileManager::new(profiles);

        // Should return the highest-priority (lowest priority number) key.
        assert_eq!(manager.resolve("openai"), Some("key-high"));
    }

    #[test]
    fn resolve_returns_none_for_unknown_provider() {
        let profiles = vec![make_profile("p1", "openai", "key-1", 1)];
        let manager = AuthProfileManager::new(profiles);

        assert_eq!(manager.resolve("anthropic"), None);
    }

    #[test]
    fn rate_limited_key_is_skipped() {
        let profiles = vec![
            make_profile("primary", "openai", "key-primary", 1),
            make_profile("fallback", "openai", "key-fallback", 2),
        ];
        let manager = AuthProfileManager::new(profiles);

        // Mark primary as rate-limited for a long cooldown.
        manager.mark_rate_limited("openai", "key-primary", Duration::from_secs(3600));

        // Should skip rate-limited primary and return fallback.
        assert_eq!(manager.resolve("openai"), Some("key-fallback"));
    }

    #[test]
    fn failed_key_is_permanently_skipped() {
        let profiles = vec![
            make_profile("primary", "openai", "key-primary", 1),
            make_profile("fallback", "openai", "key-fallback", 2),
        ];
        let manager = AuthProfileManager::new(profiles);

        manager.mark_failed("openai", "key-primary");

        assert_eq!(manager.resolve("openai"), Some("key-fallback"));
    }

    #[test]
    fn all_keys_exhausted_returns_none() {
        let profiles = vec![
            make_profile("p1", "openai", "key-1", 1),
            make_profile("p2", "openai", "key-2", 2),
        ];
        let manager = AuthProfileManager::new(profiles);

        manager.mark_failed("openai", "key-1");
        manager.mark_failed("openai", "key-2");

        assert_eq!(manager.resolve("openai"), None);
    }

    #[test]
    fn cooldown_expires_and_key_becomes_active() {
        let profiles = vec![
            make_profile("primary", "openai", "key-primary", 1),
            make_profile("fallback", "openai", "key-fallback", 2),
        ];
        let manager = AuthProfileManager::new(profiles);

        // Mark primary rate-limited with a very short cooldown.
        manager.mark_rate_limited("openai", "key-primary", Duration::from_millis(50));

        // Immediately, the fallback key should be returned.
        assert_eq!(manager.resolve("openai"), Some("key-fallback"));

        // Wait for cooldown to expire.
        thread::sleep(Duration::from_millis(100));

        // After cooldown, primary should be active again.
        assert_eq!(manager.resolve("openai"), Some("key-primary"));
    }

    #[test]
    fn active_count_reflects_current_state() {
        let profiles = vec![
            make_profile("p1", "openai", "key-1", 1),
            make_profile("p2", "openai", "key-2", 2),
            make_profile("p3", "openai", "key-3", 3),
        ];
        let manager = AuthProfileManager::new(profiles);

        assert_eq!(manager.active_count("openai"), 3);

        manager.mark_failed("openai", "key-1");
        assert_eq!(manager.active_count("openai"), 2);

        manager.mark_rate_limited("openai", "key-2", Duration::from_secs(3600));
        // Rate-limited keys are not counted as active.
        assert_eq!(manager.active_count("openai"), 1);
    }

    #[test]
    fn profiles_from_different_providers_are_isolated() {
        let profiles = vec![
            make_profile("openai-key", "openai", "oai-key", 1),
            make_profile("anthropic-key", "anthropic", "ant-key", 1),
        ];
        let manager = AuthProfileManager::new(profiles);

        manager.mark_failed("openai", "oai-key");

        // OpenAI should have no active keys.
        assert_eq!(manager.resolve("openai"), None);
        // Anthropic should be unaffected.
        assert_eq!(manager.resolve("anthropic"), Some("ant-key"));
    }
}
