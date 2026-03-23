use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use rand::Rng;
use serde::{Deserialize, Serialize};
// DM policy types — defined locally because the framework only has stubs.

/// DM access policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum DmPolicy {
    /// Allow all DMs.
    #[default]
    Open,
    /// Deny all DMs.
    Disabled,
    /// Require a pre-configured allowlist.
    Allowlist,
    /// Require pairing challenge before accepting DMs.
    Pairing,
}

/// DM access denied reason.
#[derive(Debug, Clone)]
pub enum DmAccessDenied {
    /// DMs are disabled.
    DmDisabled,
    /// Sender is not on the allowlist.
    NotAllowed,
    /// Sender needs to complete the pairing challenge.
    NeedsPairing(PairingChallenge),
}

/// A pending pairing challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingChallenge {
    pub code: String,
    pub sender_id: String,
    pub channel: String,
    pub created_at: u64,
    pub ttl_ms: u64,
}

impl PairingChallenge {
    pub fn is_expired(&self) -> bool {
        now_ms().saturating_sub(self.created_at) > self.ttl_ms
    }
}

/// Error during pairing operations.
#[derive(Debug, Clone)]
pub enum PairingError {
    CodeNotFound,
    CodeExpired,
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairingError::CodeNotFound => write!(f, "pairing code not found"),
            PairingError::CodeExpired => write!(f, "pairing code expired"),
        }
    }
}

impl std::error::Error for PairingError {}

/// Trait for DM access policy enforcement.
#[async_trait]
pub trait DmPolicyEnforcer: Send + Sync {
    async fn check_access(&self, sender_id: &str, channel: &str) -> Result<(), DmAccessDenied>;
    async fn approve_code(&self, channel: &str, code: &str) -> Result<String, PairingError>;
    async fn list_pending(&self, channel: &str) -> Vec<PairingChallenge>;
}

const MAX_PENDING_PER_CHANNEL: usize = 3;
const DEFAULT_PAIRING_TTL_MS: u64 = 3_600_000; // 1 hour

/// Callback to notify a user on a specific channel after pairing approval.
#[async_trait]
pub trait ApproveNotifier: Send + Sync {
    /// Send a message to the given sender_id (e.g. open_id for Lark).
    async fn notify_approved(&self, sender_id: &str) -> crate::error::Result<()>;
}

/// Registry of per-channel approve notifiers.
#[derive(Default)]
pub struct ApproveNotifierRegistry {
    notifiers: Mutex<HashMap<String, std::sync::Arc<dyn ApproveNotifier>>>,
}

impl ApproveNotifierRegistry {
    pub fn register(&self, channel: &str, notifier: std::sync::Arc<dyn ApproveNotifier>) {
        self.notifiers
            .lock()
            .unwrap()
            .insert(channel.to_string(), notifier);
    }

    pub async fn notify(&self, channel: &str, sender_id: &str) {
        let notifier = { self.notifiers.lock().unwrap().get(channel).cloned() };
        if let Some(n) = notifier {
            if let Err(e) = n.notify_approved(sender_id).await {
                tracing::warn!(channel, sender_id, error = %e, "failed to send pairing approval notification");
            }
        }
    }
}

/// Charset for pairing codes: uppercase + digits, excluding 0O1I.
const PAIRING_CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn generate_pairing_code() -> String {
    let mut rng = rand::rng();
    (0..8)
        .map(|_| PAIRING_CHARSET[rng.random_range(0..PAIRING_CHARSET.len())] as char)
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PendingStore {
    entries: Vec<PairingChallenge>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AllowlistStore {
    sender_ids: Vec<String>,
}

/// File-backed DM policy enforcer.
pub struct FileDmPolicyEnforcer {
    dir: PathBuf,
    policy: DmPolicy,
    config_allowlist: Option<Vec<String>>,
    pairing_ttl_ms: u64,
    state: Mutex<()>, // serialize file operations
}

impl FileDmPolicyEnforcer {
    pub fn new(dir: PathBuf, policy: DmPolicy, config_allowlist: Option<Vec<String>>) -> Self {
        Self::with_ttl(dir, policy, config_allowlist, DEFAULT_PAIRING_TTL_MS)
    }

    pub fn with_ttl(
        dir: PathBuf,
        policy: DmPolicy,
        config_allowlist: Option<Vec<String>>,
        pairing_ttl_ms: u64,
    ) -> Self {
        std::fs::create_dir_all(&dir).ok();
        Self {
            dir,
            policy,
            config_allowlist,
            pairing_ttl_ms,
            state: Mutex::new(()),
        }
    }

    fn pending_path(&self, channel: &str) -> PathBuf {
        self.dir.join(format!("{channel}-dm-pending.json"))
    }

    fn allowlist_path(&self, channel: &str) -> PathBuf {
        self.dir.join(format!("{channel}-dm-allowlist.json"))
    }

    /// Scan pairing dir for all channels that have pending or allowlist files.
    pub fn list_channels(&self) -> Vec<String> {
        let mut channels = std::collections::BTreeSet::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(ch) = name.strip_suffix("-dm-pending.json") {
                    channels.insert(ch.to_string());
                } else if let Some(ch) = name.strip_suffix("-dm-allowlist.json") {
                    channels.insert(ch.to_string());
                }
            }
        }
        channels.into_iter().collect()
    }

    fn load_pending(&self, channel: &str) -> PendingStore {
        let path = self.pending_path(channel);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_pending(&self, channel: &str, store: &PendingStore) {
        let path = self.pending_path(channel);
        if let Ok(json) = serde_json::to_string_pretty(store) {
            std::fs::write(&path, json).ok();
        }
    }

    fn load_allowlist(&self, channel: &str) -> AllowlistStore {
        let path = self.allowlist_path(channel);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_allowlist(&self, channel: &str, store: &AllowlistStore) {
        let path = self.allowlist_path(channel);
        if let Ok(json) = serde_json::to_string_pretty(store) {
            std::fs::write(&path, json).ok();
        }
    }

    fn is_in_file_allowlist(&self, sender_id: &str, channel: &str) -> bool {
        self.load_allowlist(channel)
            .sender_ids
            .iter()
            .any(|id| id == sender_id)
    }

    /// Remove an approved sender from the file-based allowlist.
    pub fn remove_from_allowlist(&self, channel: &str, sender_id: &str) -> bool {
        let _lock = self.state.lock().unwrap();
        let mut store = self.load_allowlist(channel);
        let before = store.sender_ids.len();
        store.sender_ids.retain(|id| id != sender_id);
        if store.sender_ids.len() < before {
            self.save_allowlist(channel, &store);
            true
        } else {
            false
        }
    }

    /// Get the file-based allowlist for a channel.
    pub fn get_allowlist(&self, channel: &str) -> Vec<String> {
        self.load_allowlist(channel).sender_ids
    }
}

#[async_trait]
impl DmPolicyEnforcer for FileDmPolicyEnforcer {
    async fn check_access(&self, sender_id: &str, channel: &str) -> Result<(), DmAccessDenied> {
        match &self.policy {
            DmPolicy::Open => Ok(()),
            DmPolicy::Disabled => Err(DmAccessDenied::DmDisabled),
            DmPolicy::Allowlist => {
                if let Some(ref list) = self.config_allowlist {
                    if list.iter().any(|id| id == sender_id) {
                        return Ok(());
                    }
                }
                Err(DmAccessDenied::NotAllowed)
            }
            DmPolicy::Pairing => {
                // Check file-based allowlist first
                if self.is_in_file_allowlist(sender_id, channel) {
                    return Ok(());
                }

                let _lock = self.state.lock().unwrap();
                let mut store = self.load_pending(channel);

                // Prune expired
                store.entries.retain(|e| !e.is_expired());

                // Check if sender already has a pending challenge
                if let Some(existing) = store.entries.iter().find(|e| e.sender_id == sender_id) {
                    let challenge = existing.clone();
                    self.save_pending(channel, &store);
                    return Err(DmAccessDenied::NeedsPairing(challenge));
                }

                // Evict oldest if at max
                while store.entries.len() >= MAX_PENDING_PER_CHANNEL {
                    store.entries.remove(0);
                }

                // Generate new challenge
                let challenge = PairingChallenge {
                    code: generate_pairing_code(),
                    sender_id: sender_id.to_string(),
                    channel: channel.to_string(),
                    created_at: now_ms(),
                    ttl_ms: self.pairing_ttl_ms,
                };
                store.entries.push(challenge.clone());
                self.save_pending(channel, &store);
                Err(DmAccessDenied::NeedsPairing(challenge))
            }
        }
    }

    async fn approve_code(&self, channel: &str, code: &str) -> Result<String, PairingError> {
        let _lock = self.state.lock().unwrap();
        let mut store = self.load_pending(channel);

        // Prune expired
        store.entries.retain(|e| !e.is_expired());

        let idx = store
            .entries
            .iter()
            .position(|e| e.code == code)
            .ok_or(PairingError::CodeNotFound)?;

        let challenge = store.entries.remove(idx);
        if challenge.is_expired() {
            self.save_pending(channel, &store);
            return Err(PairingError::CodeExpired);
        }

        let sender_id = challenge.sender_id.clone();
        self.save_pending(channel, &store);

        // Add to allowlist
        let mut allowlist = self.load_allowlist(channel);
        if !allowlist.sender_ids.contains(&sender_id) {
            allowlist.sender_ids.push(sender_id.clone());
        }
        self.save_allowlist(channel, &allowlist);

        Ok(sender_id)
    }

    async fn list_pending(&self, channel: &str) -> Vec<PairingChallenge> {
        let _lock = self.state.lock().unwrap();
        let mut store = self.load_pending(channel);
        store.entries.retain(|e| !e.is_expired());
        self.save_pending(channel, &store);
        store.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_enforcer(policy: DmPolicy) -> (TempDir, FileDmPolicyEnforcer) {
        let dir = TempDir::new().unwrap();
        let enforcer = FileDmPolicyEnforcer::new(dir.path().to_path_buf(), policy, None);
        (dir, enforcer)
    }

    #[tokio::test]
    async fn open_policy_allows_all() {
        let (_dir, enforcer) = make_enforcer(DmPolicy::Open);
        assert!(enforcer.check_access("user1", "lark").await.is_ok());
    }

    #[tokio::test]
    async fn disabled_policy_denies_all() {
        let (_dir, enforcer) = make_enforcer(DmPolicy::Disabled);
        let result = enforcer.check_access("user1", "lark").await;
        assert!(matches!(result, Err(DmAccessDenied::DmDisabled)));
    }

    #[tokio::test]
    async fn allowlist_checks_config_list() {
        let dir = TempDir::new().unwrap();
        let enforcer = FileDmPolicyEnforcer::new(
            dir.path().to_path_buf(),
            DmPolicy::Allowlist,
            Some(vec!["user1".to_string()]),
        );
        assert!(enforcer.check_access("user1", "lark").await.is_ok());
        assert!(matches!(
            enforcer.check_access("user2", "lark").await,
            Err(DmAccessDenied::NotAllowed)
        ));
    }

    #[tokio::test]
    async fn pairing_issues_challenge_and_approves() {
        let (_dir, enforcer) = make_enforcer(DmPolicy::Pairing);

        // First access: challenge issued
        let result = enforcer.check_access("user1", "lark").await;
        let code = match result {
            Err(DmAccessDenied::NeedsPairing(c)) => {
                assert_eq!(c.code.len(), 8);
                assert_eq!(c.sender_id, "user1");
                c.code
            }
            _ => panic!("expected NeedsPairing"),
        };

        // Same sender: returns same code
        let result2 = enforcer.check_access("user1", "lark").await;
        match result2 {
            Err(DmAccessDenied::NeedsPairing(c)) => assert_eq!(c.code, code),
            _ => panic!("expected same code"),
        }

        // Approve
        let sender = enforcer.approve_code("lark", &code).await.unwrap();
        assert_eq!(sender, "user1");

        // Now allowed
        assert!(enforcer.check_access("user1", "lark").await.is_ok());
    }

    #[test]
    fn code_charset_is_valid() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 8);
        for c in code.chars() {
            assert!(PAIRING_CHARSET.contains(&(c as u8)));
        }
    }

    #[tokio::test]
    async fn max_pending_evicts_oldest() {
        let (_dir, enforcer) = make_enforcer(DmPolicy::Pairing);
        // Generate 4 pending (max 3)
        for i in 0..4 {
            let _ = enforcer.check_access(&format!("user{i}"), "lark").await;
        }
        let pending = enforcer.list_pending("lark").await;
        assert_eq!(pending.len(), 3);
        // user0 should have been evicted
        assert!(!pending.iter().any(|p| p.sender_id == "user0"));
    }

    #[tokio::test]
    async fn approve_nonexistent_code_fails() {
        let (_dir, enforcer) = make_enforcer(DmPolicy::Pairing);
        let result = enforcer.approve_code("lark", "ZZZZZZZZ").await;
        assert!(matches!(result, Err(PairingError::CodeNotFound)));
    }
}
