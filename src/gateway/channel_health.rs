use std::sync::Arc;
use std::time::{Duration, SystemTime};

use synaptic::{ChannelAccountSnapshot, ChannelState};

use super::channel_manager::ChannelAdapterManager;

pub struct HealthMonitorConfig {
    pub check_interval: Duration,
    pub stale_threshold: Duration,
    pub stuck_threshold: Duration,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(300),
            stale_threshold: Duration::from_secs(1800),
            stuck_threshold: Duration::from_secs(1500),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthVerdict {
    Healthy,
    Disconnected,
    StaleSocket,
    Stuck,
    NotRunning,
}

impl std::fmt::Display for HealthVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::StaleSocket => write!(f, "stale_socket"),
            Self::Stuck => write!(f, "stuck"),
            Self::NotRunning => write!(f, "not_running"),
        }
    }
}

pub struct ChannelHealthMonitor {
    manager: Arc<ChannelAdapterManager>,
    config: HealthMonitorConfig,
}

impl ChannelHealthMonitor {
    pub fn new(manager: Arc<ChannelAdapterManager>, config: HealthMonitorConfig) -> Self {
        Self { manager, config }
    }

    pub fn evaluate(&self, snap: &ChannelAccountSnapshot) -> HealthVerdict {
        if !snap.running {
            return HealthVerdict::NotRunning;
        }

        if snap.state == ChannelState::Disconnected {
            return HealthVerdict::Disconnected;
        }

        let now = SystemTime::now();

        // Connected but no events for too long → stale
        if snap.state == ChannelState::Connected {
            if let Some(last_event) = snap.last_event_at {
                if let Ok(elapsed) = now.duration_since(last_event) {
                    if elapsed > self.config.stale_threshold {
                        return HealthVerdict::StaleSocket;
                    }
                }
            }
        }

        // Busy but no events for too long → stuck
        if snap.busy {
            if let Some(last_event) = snap.last_event_at {
                if let Ok(elapsed) = now.duration_since(last_event) {
                    if elapsed > self.config.stuck_threshold {
                        return HealthVerdict::Stuck;
                    }
                }
            }
        }

        HealthVerdict::Healthy
    }

    pub async fn run(self) {
        loop {
            tokio::time::sleep(self.config.check_interval).await;

            let snapshots = self.manager.snapshot_all().await;
            for snap in &snapshots {
                let verdict = self.evaluate(snap);
                if verdict != HealthVerdict::Healthy {
                    tracing::warn!(
                        channel = %snap.channel,
                        account_id = %snap.account_id,
                        verdict = %verdict,
                        state = %snap.state,
                        running = snap.running,
                        busy = snap.busy,
                        "channel health check failed"
                    );
                }
            }
        }
    }
}
