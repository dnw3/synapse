use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct RunGuard {
    _permit: OwnedSemaphorePermit,
}

pub struct AgentRunQueue {
    locks: DashMap<String, Arc<Semaphore>>,
}

impl AgentRunQueue {
    pub fn new() -> Self {
        Self {
            locks: DashMap::new(),
        }
    }

    /// Acquire execution slot. Same session_key blocks until previous execution finishes.
    pub async fn acquire(&self, session_key: &str) -> RunGuard {
        let sem = self
            .locks
            .entry(session_key.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone();
        let permit = sem.acquire_owned().await.expect("semaphore closed");
        RunGuard { _permit: permit }
    }
}

impl Default for AgentRunQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn same_session_serialized() {
        let queue = AgentRunQueue::new();
        let guard1 = queue.acquire("session-1").await;
        let acquired = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            queue.acquire("session-1"),
        )
        .await;
        assert!(acquired.is_err()); // timeout = still blocked
        drop(guard1);
        let _guard2 = queue.acquire("session-1").await;
    }

    #[tokio::test]
    async fn different_sessions_parallel() {
        let queue = AgentRunQueue::new();
        let _guard1 = queue.acquire("session-1").await;
        let _guard2 = queue.acquire("session-2").await; // should not block
    }
}
