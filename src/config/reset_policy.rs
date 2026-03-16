use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetMode {
    Daily,
    Idle,
    Never,
}

impl Default for ResetMode {
    fn default() -> Self {
        Self::Daily
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPolicy {
    #[serde(default)]
    pub mode: ResetMode,
    #[serde(default = "default_at_hour")]
    pub at_hour: u8, // For daily: reset at this hour (0-23)
    #[serde(default = "default_idle_minutes")]
    pub idle_minutes: u32, // For idle: reset after N minutes
}

fn default_at_hour() -> u8 {
    4
}
fn default_idle_minutes() -> u32 {
    60
}

impl Default for ResetPolicy {
    fn default() -> Self {
        Self {
            mode: ResetMode::Daily,
            at_hour: 4,
            idle_minutes: 60,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResetConfig {
    #[serde(default)]
    pub base: ResetPolicy,
    #[serde(default)]
    pub channels: HashMap<String, ResetPolicy>, // per-channel override
    #[serde(default)]
    pub types: HashMap<String, ResetPolicy>, // per-type override (direct/group/thread)
}

impl ResetConfig {
    /// Resolve the effective policy for a given channel and chat type.
    /// Channel-specific override takes priority over type-specific override,
    /// which in turn takes priority over the base policy.
    #[allow(dead_code)]
    pub fn resolve(&self, channel: Option<&str>, chat_type: Option<&str>) -> &ResetPolicy {
        // Channel-specific override takes priority
        if let Some(ch) = channel {
            if let Some(policy) = self.channels.get(ch) {
                return policy;
            }
        }
        // Type-specific override
        if let Some(ct) = chat_type {
            if let Some(policy) = self.types.get(ct) {
                return policy;
            }
        }
        &self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ResetConfig {
        let mut channels = HashMap::new();
        channels.insert(
            "lark".to_string(),
            ResetPolicy {
                mode: ResetMode::Idle,
                at_hour: 0,
                idle_minutes: 30,
            },
        );

        let mut types = HashMap::new();
        types.insert(
            "group".to_string(),
            ResetPolicy {
                mode: ResetMode::Never,
                at_hour: 0,
                idle_minutes: 0,
            },
        );

        ResetConfig {
            base: ResetPolicy::default(),
            channels,
            types,
        }
    }

    #[test]
    fn test_resolve_base() {
        let cfg = make_config();
        let policy = cfg.resolve(None, None);
        assert!(matches!(policy.mode, ResetMode::Daily));
        assert_eq!(policy.at_hour, 4);
        assert_eq!(policy.idle_minutes, 60);
    }

    #[test]
    fn test_resolve_channel_override() {
        let cfg = make_config();
        let policy = cfg.resolve(Some("lark"), None);
        assert!(matches!(policy.mode, ResetMode::Idle));
        assert_eq!(policy.idle_minutes, 30);
    }

    #[test]
    fn test_resolve_type_override() {
        let cfg = make_config();
        // No channel override for "slack", falls through to type
        let policy = cfg.resolve(Some("slack"), Some("group"));
        assert!(matches!(policy.mode, ResetMode::Never));
    }

    #[test]
    fn test_resolve_channel_takes_priority_over_type() {
        let cfg = make_config();
        // "lark" has a channel override; even if type is "group", channel wins
        let policy = cfg.resolve(Some("lark"), Some("group"));
        assert!(matches!(policy.mode, ResetMode::Idle));
        assert_eq!(policy.idle_minutes, 30);
    }

    #[test]
    fn test_resolve_unknown_channel_and_type_falls_back_to_base() {
        let cfg = make_config();
        let policy = cfg.resolve(Some("unknown"), Some("direct"));
        assert!(matches!(policy.mode, ResetMode::Daily));
    }
}
