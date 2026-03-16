//! Cron job persistence — in-memory store with optional JSONL file backing.

use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single cron-scheduled agent job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// Unique job identifier (UUID v4).
    pub id: String,
    /// Standard 5-field cron expression (`minute hour dom month dow`).
    pub expression: String,
    /// Agent identifier to invoke when the job fires.
    pub agent_id: String,
    /// Optional channel to route the agent message through.
    pub channel: Option<String>,
    /// Message / prompt sent to the agent.
    pub message: String,
    /// Whether this job is active.
    pub enabled: bool,
    /// Timestamp of the last successful execution.
    pub last_run: Option<DateTime<Utc>>,
    /// Pre-computed next scheduled execution time.
    pub next_run: Option<DateTime<Utc>>,
    /// Number of consecutive failures since the last success.
    pub failure_count: u32,
    /// Maximum retries before the job is disabled.
    pub max_retries: u32,
}

impl CronJob {
    /// Create a new enabled job with sensible defaults.
    pub fn new(
        id: impl Into<String>,
        expression: impl Into<String>,
        agent_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            expression: expression.into(),
            agent_id: agent_id.into(),
            channel: None,
            message: message.into(),
            enabled: true,
            last_run: None,
            next_run: None,
            failure_count: 0,
            max_retries: 3,
        }
    }
}

/// In-memory cron job store.
///
/// All operations acquire the inner `RwLock`; the store is `Send + Sync` and
/// can be wrapped in `Arc` and shared across tasks.
pub struct CronStore {
    jobs: RwLock<Vec<CronJob>>,
}

impl Default for CronStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CronStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(Vec::new()),
        }
    }

    /// Create a store pre-populated with the given jobs.
    pub fn with_jobs(jobs: Vec<CronJob>) -> Self {
        Self {
            jobs: RwLock::new(jobs),
        }
    }

    /// Add or replace a job.  If a job with the same `id` already exists it is
    /// replaced; otherwise the job is appended.
    pub fn upsert(&self, job: CronJob) {
        let mut guard = self.jobs.write().expect("cron store write lock poisoned");
        if let Some(existing) = guard.iter_mut().find(|j| j.id == job.id) {
            *existing = job;
        } else {
            guard.push(job);
        }
    }

    /// Remove the job with the given `id`.  Returns `true` if a job was removed.
    pub fn remove(&self, id: &str) -> bool {
        let mut guard = self.jobs.write().expect("cron store write lock poisoned");
        let before = guard.len();
        guard.retain(|j| j.id != id);
        guard.len() < before
    }

    /// Return a snapshot of all jobs.
    pub fn list(&self) -> Vec<CronJob> {
        self.jobs
            .read()
            .expect("cron store read lock poisoned")
            .clone()
    }

    /// Return a snapshot of enabled jobs only.
    pub fn enabled_jobs(&self) -> Vec<CronJob> {
        self.jobs
            .read()
            .expect("cron store read lock poisoned")
            .iter()
            .filter(|j| j.enabled)
            .cloned()
            .collect()
    }

    /// Look up a single job by id.
    pub fn get(&self, id: &str) -> Option<CronJob> {
        self.jobs
            .read()
            .expect("cron store read lock poisoned")
            .iter()
            .find(|j| j.id == id)
            .cloned()
    }

    /// Apply a patch function to the job with the given `id`.
    /// Returns `true` when the job was found and patched.
    pub fn update<F>(&self, id: &str, f: F) -> bool
    where
        F: FnOnce(&mut CronJob),
    {
        let mut guard = self.jobs.write().expect("cron store write lock poisoned");
        if let Some(job) = guard.iter_mut().find(|j| j.id == id) {
            f(job);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_and_list() {
        let store = CronStore::new();
        let job = CronJob::new("job-1", "* * * * *", "agent-a", "hello");
        store.upsert(job.clone());
        let jobs = store.list();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job-1");
    }

    #[test]
    fn upsert_replaces_existing() {
        let store = CronStore::new();
        let mut job = CronJob::new("job-1", "* * * * *", "agent-a", "hello");
        store.upsert(job.clone());
        job.message = "updated".to_string();
        store.upsert(job);
        let jobs = store.list();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].message, "updated");
    }

    #[test]
    fn remove_job() {
        let store = CronStore::new();
        store.upsert(CronJob::new("job-1", "* * * * *", "agent-a", "hello"));
        assert!(store.remove("job-1"));
        assert!(!store.remove("job-1")); // already gone
        assert!(store.list().is_empty());
    }

    #[test]
    fn enabled_jobs_filter() {
        let store = CronStore::new();
        let mut job1 = CronJob::new("job-1", "* * * * *", "a", "msg");
        let mut job2 = CronJob::new("job-2", "* * * * *", "b", "msg");
        job2.enabled = false;
        store.upsert(job1.clone());
        store.upsert(job2);
        let enabled = store.enabled_jobs();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "job-1");
        let _ = job1; // suppress unused warning
    }

    #[test]
    fn update_job() {
        let store = CronStore::new();
        store.upsert(CronJob::new("job-1", "* * * * *", "a", "old"));
        let found = store.update("job-1", |j| j.message = "new".to_string());
        assert!(found);
        assert_eq!(store.get("job-1").unwrap().message, "new");
    }
}
