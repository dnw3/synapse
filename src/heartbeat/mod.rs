//! Heartbeat system — periodic proactive agent runs.
//!
//! When enabled, the heartbeat runner periodically loads a prompt from
//! `HEARTBEAT.md` (or a configured file) and runs the agent. Results can
//! optionally be delivered to a target channel.

use std::time::Duration;

use serde::Deserialize;
use tokio::sync::watch;

/// Configuration for heartbeat-driven agent runs.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct HeartbeatConfig {
    /// Whether heartbeat is enabled (default: false).
    #[serde(default)]
    pub enabled: bool,

    /// Interval between heartbeat runs (e.g., "5m", "1h", "30s").
    #[serde(default = "default_interval")]
    pub interval: String,

    /// Path to HEARTBEAT.md prompt file (relative to workspace).
    #[serde(default = "default_heartbeat_file")]
    pub prompt_file: String,

    /// Active hours window (e.g., "09:00-18:00"). Empty = always active.
    #[serde(default)]
    pub active_hours: Option<String>,

    /// Maximum response length for heartbeat runs.
    #[serde(default = "default_ack_max_chars")]
    pub ack_max_chars: usize,

    /// Channel to deliver heartbeat results to (e.g., "slack", "telegram").
    #[serde(default)]
    pub target_channel: Option<String>,
}

fn default_interval() -> String {
    "5m".to_string()
}

fn default_heartbeat_file() -> String {
    "HEARTBEAT.md".to_string()
}

fn default_ack_max_chars() -> usize {
    2000
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: default_interval(),
            prompt_file: default_heartbeat_file(),
            active_hours: None,
            ack_max_chars: default_ack_max_chars(),
            target_channel: None,
        }
    }
}

/// Parse duration strings like "5m", "1h", "30s", "500ms" into [`Duration`].
///
/// Supported suffixes: `ms` (milliseconds), `s` (seconds), `m` (minutes), `h` (hours).
/// A bare number without suffix is treated as seconds.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(val) = s.strip_suffix("ms") {
        val.trim().parse::<u64>().ok().map(Duration::from_millis)
    } else if let Some(val) = s.strip_suffix('h') {
        val.trim()
            .parse::<u64>()
            .ok()
            .map(|v| Duration::from_secs(v * 3600))
    } else if let Some(val) = s.strip_suffix('m') {
        val.trim()
            .parse::<u64>()
            .ok()
            .map(|v| Duration::from_secs(v * 60))
    } else if let Some(val) = s.strip_suffix('s') {
        val.trim().parse::<u64>().ok().map(Duration::from_secs)
    } else {
        // Bare number → seconds
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

/// Check if the current local time is within the given active-hours window.
///
/// Expected format: `"HH:MM-HH:MM"` (24-hour clock, local time).
/// Returns `true` if the current time falls within the window, or if the
/// window string cannot be parsed (fail-open).
pub fn is_within_active_hours(window: &str) -> bool {
    let parts: Vec<&str> = window.split('-').collect();
    if parts.len() != 2 {
        tracing::warn!(
            "heartbeat: invalid active_hours format '{}', expected 'HH:MM-HH:MM'; defaulting to active",
            window
        );
        return true;
    }

    let now = chrono::Local::now().time();

    let parse_hm = |s: &str| -> Option<chrono::NaiveTime> {
        let hm: Vec<&str> = s.trim().split(':').collect();
        if hm.len() != 2 {
            return None;
        }
        let h: u32 = hm[0].parse().ok()?;
        let m: u32 = hm[1].parse().ok()?;
        chrono::NaiveTime::from_hms_opt(h, m, 0)
    };

    let (start, end) = match (parse_hm(parts[0]), parse_hm(parts[1])) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            tracing::warn!(
                "heartbeat: could not parse active_hours '{}'; defaulting to active",
                window
            );
            return true;
        }
    };

    if start <= end {
        // Normal window, e.g. 09:00-18:00
        now >= start && now < end
    } else {
        // Overnight window, e.g. 22:00-06:00
        now >= start || now < end
    }
}

/// The heartbeat runner — periodically invokes the agent with a prompt file.
pub struct HeartbeatRunner {
    config: HeartbeatConfig,
    shutdown: watch::Receiver<bool>,
}

impl HeartbeatRunner {
    pub fn new(config: HeartbeatConfig, shutdown: watch::Receiver<bool>) -> Self {
        Self { config, shutdown }
    }

    /// Start the heartbeat loop. Returns a [`tokio::task::JoinHandle`].
    ///
    /// The loop reads the prompt file on each tick, checks active hours,
    /// and invokes the agent. The response is truncated to `ack_max_chars`
    /// and optionally delivered to the configured target channel.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let interval_dur =
                parse_duration(&self.config.interval).unwrap_or(Duration::from_secs(300));
            let mut ticker = tokio::time::interval(interval_dur);
            let mut shutdown = self.shutdown;

            tracing::info!(
                "heartbeat: started (interval={}, prompt_file={}, active_hours={:?})",
                self.config.interval,
                self.config.prompt_file,
                self.config.active_hours
            );
            eprintln!(
                "[heartbeat] Running every {} (prompt: {})",
                self.config.interval, self.config.prompt_file
            );

            // Skip the first immediate tick
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        // Check active hours gate
                        if let Some(ref window) = self.config.active_hours {
                            if !is_within_active_hours(window) {
                                tracing::debug!("heartbeat: outside active hours ({}), skipping", window);
                                continue;
                            }
                        }

                        // Load the prompt file
                        let prompt = match tokio::fs::read_to_string(&self.config.prompt_file).await {
                            Ok(content) if !content.trim().is_empty() => content,
                            Ok(_) => {
                                tracing::debug!("heartbeat: prompt file is empty, skipping");
                                continue;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "heartbeat: could not read '{}': {}",
                                    self.config.prompt_file, e
                                );
                                continue;
                            }
                        };

                        tracing::info!("heartbeat: tick — running agent with prompt ({} chars)", prompt.len());

                        // TODO: Wire in actual agent invocation once dependencies are available.
                        // For now, log the heartbeat event.
                        let response = format!(
                            "[heartbeat] Would run agent with prompt ({} chars). Target channel: {:?}",
                            prompt.len(),
                            self.config.target_channel
                        );

                        // Truncate response to ack_max_chars
                        let truncated = if response.len() > self.config.ack_max_chars {
                            format!("{}…", &response[..self.config.ack_max_chars.saturating_sub(1)])
                        } else {
                            response
                        };

                        eprintln!("{}", truncated);

                        if let Some(ref channel) = self.config.target_channel {
                            tracing::info!(
                                "heartbeat: would deliver to channel '{}' ({} chars)",
                                channel,
                                truncated.len()
                            );
                        }
                    }
                    _ = shutdown.changed() => {
                        tracing::info!("heartbeat: shutdown signal received, stopping");
                        eprintln!("[heartbeat] Shutting down");
                        break;
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(parse_duration("60"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc"), None);
    }

    #[test]
    fn test_active_hours_bad_format() {
        // Malformed input should fail-open (return true)
        assert!(is_within_active_hours("garbage"));
        assert!(is_within_active_hours("25:00-18:00"));
    }

    #[test]
    fn test_default_config() {
        let cfg = HeartbeatConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.interval, "5m");
        assert_eq!(cfg.prompt_file, "HEARTBEAT.md");
        assert_eq!(cfg.ack_max_chars, 2000);
        assert!(cfg.active_hours.is_none());
        assert!(cfg.target_channel.is_none());
    }

    #[test]
    fn test_deserialize_config() {
        let toml_str = r#"
enabled = true
interval = "10m"
prompt_file = "my-heartbeat.md"
active_hours = "09:00-18:00"
ack_max_chars = 500
target_channel = "slack"
"#;
        let cfg: HeartbeatConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.interval, "10m");
        assert_eq!(cfg.prompt_file, "my-heartbeat.md");
        assert_eq!(cfg.active_hours.as_deref(), Some("09:00-18:00"));
        assert_eq!(cfg.ack_max_chars, 500);
        assert_eq!(cfg.target_channel.as_deref(), Some("slack"));
    }
}
