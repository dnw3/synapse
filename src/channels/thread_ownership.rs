#![allow(dead_code)]
use dashmap::DashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Ownership {
    agent_id: String,
    acquired_at: Instant,
}

pub struct ThreadOwnershipTracker {
    ownership: DashMap<String, Ownership>, // thread_key → ownership
    ttl: Duration,
}

impl ThreadOwnershipTracker {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ownership: DashMap::new(),
            ttl,
        }
    }

    /// Check if a thread has an active owner. Returns the agent_id if owned.
    pub fn get_owner(&self, thread_key: &str) -> Option<String> {
        if let Some(entry) = self.ownership.get(thread_key) {
            if entry.acquired_at.elapsed() < self.ttl {
                return Some(entry.agent_id.clone());
            } else {
                drop(entry);
                self.ownership.remove(thread_key);
            }
        }
        None
    }

    /// Claim ownership of a thread for an agent.
    pub fn claim(&self, thread_key: &str, agent_id: &str) {
        self.ownership.insert(
            thread_key.to_string(),
            Ownership {
                agent_id: agent_id.to_string(),
                acquired_at: Instant::now(),
            },
        );
    }

    /// Release ownership.
    pub fn release(&self, thread_key: &str) {
        self.ownership.remove(thread_key);
    }

    /// Clean up expired entries.
    pub fn cleanup_expired(&self) {
        self.ownership
            .retain(|_, v| v.acquired_at.elapsed() < self.ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_and_get_owner() {
        let tracker = ThreadOwnershipTracker::new(Duration::from_secs(1800));
        tracker.claim("thread-1", "agent-coder");
        assert_eq!(
            tracker.get_owner("thread-1"),
            Some("agent-coder".to_string())
        );
    }

    #[test]
    fn no_owner_for_unclaimed_thread() {
        let tracker = ThreadOwnershipTracker::new(Duration::from_secs(1800));
        assert_eq!(tracker.get_owner("thread-1"), None);
    }

    #[test]
    fn expired_ownership_returns_none() {
        let tracker = ThreadOwnershipTracker::new(Duration::from_millis(1));
        tracker.claim("thread-1", "agent-a");
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(tracker.get_owner("thread-1"), None);
    }
}
