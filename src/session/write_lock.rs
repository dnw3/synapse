use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Manages per-session write locks to prevent concurrent modifications.
///
/// Each session can only be written to by one holder (WebSocket connection) at a time.
/// Locks expire after a configurable timeout to handle abandoned connections.
pub struct SessionWriteLock {
    locks: Mutex<HashMap<String, LockEntry>>,
    timeout: Duration,
}

struct LockEntry {
    /// Who holds the lock (e.g. WebSocket connection id).
    holder: String,
    /// When the lock was acquired.
    acquired_at: Instant,
}

impl SessionWriteLock {
    /// Create a new `SessionWriteLock` with the given expiry timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
            timeout,
        }
    }

    /// Try to acquire a write lock for `session_id` on behalf of `holder`.
    ///
    /// Returns `Ok(())` if the lock was acquired (or re-entered by the same holder).
    /// Returns `Err` with a description if another holder already owns the lock.
    pub async fn try_acquire(&self, session_id: &str, holder: &str) -> Result<(), String> {
        let mut locks = self.locks.lock().await;

        if let Some(entry) = locks.get(session_id) {
            // Same holder re-entering is allowed.
            if entry.holder == holder {
                return Ok(());
            }
            // Check if the existing lock has expired.
            if entry.acquired_at.elapsed() >= self.timeout {
                // Expired — allow takeover.
            } else {
                return Err(format!(
                    "session '{}' is locked by '{}' (acquired {:?} ago)",
                    session_id,
                    entry.holder,
                    entry.acquired_at.elapsed(),
                ));
            }
        }

        locks.insert(
            session_id.to_string(),
            LockEntry {
                holder: holder.to_string(),
                acquired_at: Instant::now(),
            },
        );
        Ok(())
    }

    /// Release the write lock for `session_id`, but only if `holder` is the current owner.
    pub async fn release(&self, session_id: &str, holder: &str) {
        let mut locks = self.locks.lock().await;
        if let Some(entry) = locks.get(session_id) {
            if entry.holder == holder {
                locks.remove(session_id);
            }
        }
    }

    /// Remove all locks that have exceeded the configured timeout.
    ///
    /// Call this periodically (e.g. from a background task) to reclaim
    /// locks abandoned by disconnected clients.
    #[allow(dead_code)]
    pub async fn cleanup_expired(&self) {
        let mut locks = self.locks.lock().await;
        locks.retain(|_, entry| entry.acquired_at.elapsed() < self.timeout);
    }

    /// Check whether `session_id` currently has an active (non-expired) lock.
    #[allow(dead_code)]
    pub async fn is_locked(&self, session_id: &str) -> bool {
        let locks = self.locks.lock().await;
        match locks.get(session_id) {
            Some(entry) => entry.acquired_at.elapsed() < self.timeout,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquire_and_release() {
        let wl = SessionWriteLock::new(Duration::from_secs(300));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        assert!(wl.is_locked("s1").await);
        wl.release("s1", "conn-a").await;
        assert!(!wl.is_locked("s1").await);
    }

    #[tokio::test]
    async fn same_holder_reentrant() {
        let wl = SessionWriteLock::new(Duration::from_secs(300));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        // Same holder can re-acquire.
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
    }

    #[tokio::test]
    async fn different_holder_blocked() {
        let wl = SessionWriteLock::new(Duration::from_secs(300));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        assert!(wl.try_acquire("s1", "conn-b").await.is_err());
    }

    #[tokio::test]
    async fn expired_lock_allows_takeover() {
        let wl = SessionWriteLock::new(Duration::from_millis(1));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        tokio::time::sleep(Duration::from_millis(5)).await;
        // Lock has expired — another holder can take over.
        assert!(wl.try_acquire("s1", "conn-b").await.is_ok());
    }

    #[tokio::test]
    async fn cleanup_removes_expired() {
        let wl = SessionWriteLock::new(Duration::from_millis(1));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        tokio::time::sleep(Duration::from_millis(5)).await;
        wl.cleanup_expired().await;
        assert!(!wl.is_locked("s1").await);
    }

    #[tokio::test]
    async fn release_wrong_holder_noop() {
        let wl = SessionWriteLock::new(Duration::from_secs(300));
        assert!(wl.try_acquire("s1", "conn-a").await.is_ok());
        // Wrong holder cannot release.
        wl.release("s1", "conn-b").await;
        assert!(wl.is_locked("s1").await);
    }
}
