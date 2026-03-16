//! Device pairing service using short-lived 6-digit challenge codes.
//!
//! Workflow:
//! 1. A device calls `create_challenge(device_name)` → gets a 6-digit code.
//! 2. The user sees the code and approves (or rejects) via the dashboard.
//! 3. The device polls `verify(code)` to learn whether it was approved/rejected/expired.
//! 4. `cleanup_expired()` can be called periodically to prune stale entries.

use dashmap::DashMap;
use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

#[derive(Debug, Clone)]
pub struct PairingChallenge {
    pub code: String,
    pub device_name: String,
    pub status: PairingStatus,
    pub created_at: Instant,
}

// ---------------------------------------------------------------------------
// PairingService
// ---------------------------------------------------------------------------

/// In-memory device pairing service backed by a [`DashMap`].
///
/// All methods are `&self` (no `&mut self`) so the service can be held behind
/// an [`Arc`] and shared across async tasks / Axum handlers without a `Mutex`.
#[derive(Clone)]
pub struct PairingService {
    challenges: Arc<DashMap<String, PairingChallenge>>,
    ttl: Duration,
}

impl PairingService {
    /// Create a new service with the given TTL for pending challenges.
    pub fn new(ttl: Duration) -> Self {
        Self {
            challenges: Arc::new(DashMap::new()),
            ttl,
        }
    }

    /// Create a new service with the default TTL of 5 minutes.
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(5 * 60))
    }

    /// Generate a new 6-digit challenge code and store it with `Pending` status.
    ///
    /// Returns the 6-digit code string (zero-padded, e.g. `"042731"`).
    pub fn create_challenge(&self, device_name: &str) -> String {
        let code: String = {
            let mut rng = rand::rng();
            (0..6)
                .map(|_| rng.random_range(0..10u8).to_string())
                .collect()
        };
        self.challenges.insert(
            code.clone(),
            PairingChallenge {
                code: code.clone(),
                device_name: device_name.to_string(),
                status: PairingStatus::Pending,
                created_at: Instant::now(),
            },
        );
        tracing::info!(
            code = %code,
            device_name = %device_name,
            "pairing challenge created"
        );
        code
    }

    /// Approve a pending challenge.  Returns `true` if the code existed and
    /// was in `Pending` state; `false` otherwise (already resolved or unknown).
    pub fn approve(&self, code: &str) -> bool {
        if let Some(mut entry) = self.challenges.get_mut(code) {
            if entry.created_at.elapsed() >= self.ttl {
                entry.status = PairingStatus::Expired;
                tracing::warn!(code = %code, "approve: challenge already expired");
                return false;
            }
            if entry.status == PairingStatus::Pending {
                entry.status = PairingStatus::Approved;
                tracing::info!(code = %code, device_name = %entry.device_name, "pairing approved");
                return true;
            }
        }
        false
    }

    /// Reject a pending challenge.  Returns `true` if the code existed and
    /// was in `Pending` state; `false` otherwise.
    pub fn reject(&self, code: &str) -> bool {
        if let Some(mut entry) = self.challenges.get_mut(code) {
            if entry.created_at.elapsed() >= self.ttl {
                entry.status = PairingStatus::Expired;
                tracing::warn!(code = %code, "reject: challenge already expired");
                return false;
            }
            if entry.status == PairingStatus::Pending {
                entry.status = PairingStatus::Rejected;
                tracing::info!(code = %code, device_name = %entry.device_name, "pairing rejected");
                return true;
            }
        }
        false
    }

    /// Check the current status of a challenge code.
    ///
    /// Returns `None` if the code is unknown.  If the entry has exceeded the
    /// TTL and is still `Pending` it is transitioned to `Expired` in-place.
    pub fn verify(&self, code: &str) -> Option<PairingStatus> {
        let mut entry = self.challenges.get_mut(code)?;
        if entry.status == PairingStatus::Pending && entry.created_at.elapsed() >= self.ttl {
            entry.status = PairingStatus::Expired;
        }
        Some(entry.status.clone())
    }

    /// Remove all entries that have exceeded the TTL and are still `Pending`.
    /// Entries that have already been `Approved` / `Rejected` are left in place
    /// so callers can still read the final status once.
    pub fn cleanup_expired(&self) {
        let ttl = self.ttl;
        self.challenges.retain(|_, challenge| {
            if challenge.status == PairingStatus::Pending && challenge.created_at.elapsed() >= ttl {
                tracing::debug!(code = %challenge.code, "pairing challenge expired, removing");
                false
            } else {
                true
            }
        });
    }

    /// Return all challenges currently in `Pending` state (and not yet expired).
    pub fn list_pending(&self) -> Vec<PairingChallenge> {
        let ttl = self.ttl;
        self.challenges
            .iter()
            .filter(|entry| {
                entry.status == PairingStatus::Pending && entry.created_at.elapsed() < ttl
            })
            .map(|entry| entry.value().clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn short_ttl() -> PairingService {
        PairingService::new(Duration::from_millis(100))
    }

    // -- create --

    #[test]
    fn create_returns_six_digit_code() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("my-laptop");
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn created_challenge_is_pending() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("tablet");
        assert_eq!(svc.verify(&code), Some(PairingStatus::Pending));
    }

    #[test]
    fn create_challenge_appears_in_list_pending() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("phone");
        let pending = svc.list_pending();
        assert!(pending.iter().any(|c| c.code == code));
    }

    // -- approve --

    #[test]
    fn approve_changes_status_to_approved() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("watch");
        assert!(svc.approve(&code));
        assert_eq!(svc.verify(&code), Some(PairingStatus::Approved));
    }

    #[test]
    fn approve_removes_from_list_pending() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("tv");
        svc.approve(&code);
        let pending = svc.list_pending();
        assert!(!pending.iter().any(|c| c.code == code));
    }

    #[test]
    fn approve_unknown_code_returns_false() {
        let svc = PairingService::with_default_ttl();
        assert!(!svc.approve("999999"));
    }

    #[test]
    fn double_approve_returns_false_second_time() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("speaker");
        assert!(svc.approve(&code));
        assert!(!svc.approve(&code));
    }

    // -- reject --

    #[test]
    fn reject_changes_status_to_rejected() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("unknown-device");
        assert!(svc.reject(&code));
        assert_eq!(svc.verify(&code), Some(PairingStatus::Rejected));
    }

    #[test]
    fn reject_unknown_code_returns_false() {
        let svc = PairingService::with_default_ttl();
        assert!(!svc.reject("000000"));
    }

    #[test]
    fn reject_after_approve_returns_false() {
        let svc = PairingService::with_default_ttl();
        let code = svc.create_challenge("dev");
        svc.approve(&code);
        assert!(!svc.reject(&code));
    }

    // -- expiry --

    #[test]
    fn verify_returns_expired_after_ttl() {
        let svc = short_ttl();
        let code = svc.create_challenge("old-device");
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(svc.verify(&code), Some(PairingStatus::Expired));
    }

    #[test]
    fn approve_after_ttl_returns_false() {
        let svc = short_ttl();
        let code = svc.create_challenge("stale");
        std::thread::sleep(Duration::from_millis(150));
        assert!(!svc.approve(&code));
    }

    #[test]
    fn cleanup_removes_expired_pending() {
        let svc = short_ttl();
        let code = svc.create_challenge("ghost");
        std::thread::sleep(Duration::from_millis(150));
        svc.cleanup_expired();
        assert_eq!(svc.verify(&code), None);
    }

    #[test]
    fn cleanup_keeps_approved_entries() {
        let svc = short_ttl();
        let code = svc.create_challenge("keeper");
        svc.approve(&code);
        std::thread::sleep(Duration::from_millis(150));
        svc.cleanup_expired(); // only removes Pending+expired
        assert_eq!(svc.verify(&code), Some(PairingStatus::Approved));
    }

    #[test]
    fn expired_not_in_list_pending() {
        let svc = short_ttl();
        let code = svc.create_challenge("slow-device");
        std::thread::sleep(Duration::from_millis(150));
        let pending = svc.list_pending();
        assert!(!pending.iter().any(|c| c.code == code));
    }

    // -- arc sharing --

    #[test]
    fn service_is_clone_shareable() {
        let svc = PairingService::with_default_ttl();
        let svc2 = svc.clone();
        let code = svc.create_challenge("cloned");
        assert_eq!(svc2.verify(&code), Some(PairingStatus::Pending));
    }
}
