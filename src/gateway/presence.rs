//! Presence tracking for connected clients.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

const TTL_MS: u64 = 5 * 60 * 1000; // 5 minutes
const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub key: String,
    pub host: Option<String>,
    pub ip: Option<String>,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub device_family: Option<String>,
    pub model_identifier: Option<String>,
    pub mode: Option<String>,
    pub reason: Option<String>,
    pub device_id: Option<String>,
    pub instance_id: Option<String>,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
    pub text: String,
    pub ts: u64,
}

pub struct PresenceStore {
    entries: HashMap<String, PresenceEntry>,
    version: u64,
}

#[allow(dead_code)]
impl PresenceStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            version: 0,
        }
    }

    pub fn upsert(&mut self, entry: PresenceEntry) -> bool {
        let key = Self::generate_key(&entry);
        self.prune();
        let changed = self
            .entries
            .get(&key)
            .map(|e| e.text != entry.text || e.mode != entry.mode)
            .unwrap_or(true);
        if changed {
            self.version += 1;
        }
        self.entries.insert(key, entry);
        changed
    }

    pub fn remove(&mut self, key: &str) {
        if self.entries.remove(key).is_some() {
            self.version += 1;
        }
    }

    pub fn list(&mut self) -> Vec<PresenceEntry> {
        self.prune();
        let mut entries: Vec<_> = self.entries.values().cloned().collect();
        entries.sort_by(|a, b| b.ts.cmp(&a.ts));
        entries
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn snapshot_json(&mut self) -> serde_json::Value {
        serde_json::to_value(self.list()).unwrap_or_default()
    }

    fn generate_key(entry: &PresenceEntry) -> String {
        entry
            .device_id
            .as_deref()
            .or(entry.instance_id.as_deref())
            .or(entry.host.as_deref())
            .or(entry.ip.as_deref())
            .unwrap_or(&entry.text)
            .trim()
            .to_lowercase()
    }

    fn prune(&mut self) {
        let now = now_ms();
        self.entries
            .retain(|_, e| now.saturating_sub(e.ts) < TTL_MS);
        // Enforce MAX_ENTRIES
        while self.entries.len() > MAX_ENTRIES {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.ts)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest_key);
            } else {
                break;
            }
        }
    }
}

impl Default for PresenceStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
