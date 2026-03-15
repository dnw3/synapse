use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use synaptic::{
    ChannelAccountSnapshot, ChannelProbe, ChannelState, ChannelStatusHandle, ChannelStatusPatch,
};
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// ChannelStatusHandleImpl — implements ChannelStatusHandle with interior mutability
// ---------------------------------------------------------------------------

pub struct ChannelStatusHandleImpl {
    inner: std::sync::RwLock<ChannelAccountSnapshot>,
}

impl ChannelStatusHandleImpl {
    pub fn new(channel: &str, account_id: &str) -> Self {
        Self {
            inner: std::sync::RwLock::new(ChannelAccountSnapshot::new(channel, account_id)),
        }
    }
}

impl ChannelStatusHandle for ChannelStatusHandleImpl {
    fn get(&self) -> ChannelAccountSnapshot {
        self.inner.read().unwrap().clone()
    }

    fn set(&self, patch: ChannelStatusPatch) {
        let mut snap = self.inner.write().unwrap();
        if let Some(state) = patch.state {
            // When transitioning to Connected, set connected_at
            if state == ChannelState::Connected && snap.state != ChannelState::Connected {
                snap.connected_at = Some(SystemTime::now());
            }
            snap.state = state;
        }
        if let Some(running) = patch.running {
            snap.running = running;
        }
        if let Some(busy) = patch.busy {
            snap.busy = busy;
        }
        if let Some(active_runs) = patch.active_runs {
            snap.active_runs = active_runs;
        }
        if let Some(last_event_at) = patch.last_event_at {
            snap.last_event_at = Some(last_event_at);
        }
        if let Some(ref last_error) = patch.last_error {
            snap.last_error = last_error.clone();
        }
        if let Some(ref last_disconnect) = patch.last_disconnect {
            snap.last_disconnect = last_disconnect.clone();
        }
        if let Some(ref mode) = patch.mode {
            snap.mode = Some(mode.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// ActivityRecord — tracks inbound/outbound timestamps per account
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct ActivityRecord {
    last_inbound_at: Option<SystemTime>,
    last_outbound_at: Option<SystemTime>,
}

// ---------------------------------------------------------------------------
// ChannelAdapterManager — lifecycle management for all channel adapters
// ---------------------------------------------------------------------------

type AccountKey = (String, String);

#[allow(dead_code)]
pub struct ChannelAdapterManager {
    statuses: tokio::sync::RwLock<HashMap<AccountKey, Arc<ChannelStatusHandleImpl>>>,
    tasks: tokio::sync::RwLock<HashMap<AccountKey, JoinHandle<()>>>,
    probes: tokio::sync::RwLock<HashMap<AccountKey, Arc<dyn ChannelProbe>>>,
    activities: tokio::sync::RwLock<HashMap<AccountKey, ActivityRecord>>,
}

#[allow(dead_code)]
impl ChannelAdapterManager {
    pub fn new() -> Self {
        Self {
            statuses: tokio::sync::RwLock::new(HashMap::new()),
            tasks: tokio::sync::RwLock::new(HashMap::new()),
            probes: tokio::sync::RwLock::new(HashMap::new()),
            activities: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Register a new adapter account with an existing status handle,
    /// stores the task handle and optional probe.
    pub async fn register(
        &self,
        channel: &str,
        account_id: &str,
        task: JoinHandle<()>,
        probe: Option<Arc<dyn ChannelProbe>>,
        handle: Arc<ChannelStatusHandleImpl>,
    ) {
        let key = (channel.to_string(), account_id.to_string());

        self.statuses.write().await.insert(key.clone(), handle);
        self.tasks.write().await.insert(key.clone(), task);
        if let Some(p) = probe {
            self.probes.write().await.insert(key.clone(), p);
        }
        self.activities.write().await.insert(
            key,
            ActivityRecord {
                last_inbound_at: None,
                last_outbound_at: None,
            },
        );
    }

    /// Collect snapshots for all registered accounts, merging activity data.
    pub async fn snapshot_all(&self) -> Vec<ChannelAccountSnapshot> {
        let statuses = self.statuses.read().await;
        let activities = self.activities.read().await;
        let mut out = Vec::with_capacity(statuses.len());
        for (key, handle) in statuses.iter() {
            let mut snap = handle.get();
            if let Some(activity) = activities.get(key) {
                if activity.last_inbound_at.is_some() {
                    snap.last_inbound_at = activity.last_inbound_at;
                }
                if activity.last_outbound_at.is_some() {
                    snap.last_outbound_at = activity.last_outbound_at;
                }
            }
            out.push(snap);
        }
        out
    }

    /// Record an inbound message timestamp for an account.
    pub async fn record_inbound(&self, channel: &str, account_id: &str) {
        let key = (channel.to_string(), account_id.to_string());
        let mut activities = self.activities.write().await;
        if let Some(record) = activities.get_mut(&key) {
            record.last_inbound_at = Some(SystemTime::now());
        }
    }

    /// Record an outbound message timestamp for an account.
    pub async fn record_outbound(&self, channel: &str, account_id: &str) {
        let key = (channel.to_string(), account_id.to_string());
        let mut activities = self.activities.write().await;
        if let Some(record) = activities.get_mut(&key) {
            record.last_outbound_at = Some(SystemTime::now());
        }
    }

    /// Run all registered probes and return results.
    pub async fn run_probes(&self) -> Vec<(String, String, Result<(), String>)> {
        let probes = self.probes.read().await;
        let mut results = Vec::with_capacity(probes.len());
        for ((channel, account_id), probe) in probes.iter() {
            let result = probe.probe().await;
            results.push((channel.clone(), account_id.clone(), result));
        }
        results
    }
}
