//! CronService — periodic agent job scheduling.
//!
//! # Overview
//!
//! [`CronService`] holds an in-memory [`CronStore`] and an [`EventBus`]
//! reference.  On every [`CronService::tick`] call it:
//!
//! 1. Iterates over all enabled jobs whose `next_run` is `≤ now`.
//! 2. Emits a `CronJobFired` event via the event bus (payload contains the
//!    `job_id`, `agent_id`, `channel`, and `message`).
//! 3. Updates `last_run` and recomputes `next_run` from the cron expression.
//! 4. On failure, increments `failure_count`; disables the job when
//!    `failure_count > max_retries`.
//!
//! Callers are responsible for driving the tick loop (e.g. `tokio::time::interval`).
//!
//! # Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use synapse::cron::{CronService, CronStore, CronJob};
//! use synaptic::events::EventBus;
//!
//! # tokio_test::block_on(async {
//! let store = Arc::new(CronStore::new());
//! let bus   = Arc::new(EventBus::new());
//! let svc   = CronService::new(store.clone(), bus);
//!
//! let job = CronJob::new("job-1", "* * * * *", "default", "run diagnostics");
//! svc.add_job(job);
//!
//! // In production, drive via tokio::time::interval:
//! // loop { interval.tick().await; svc.tick().await; }
//! # });
//! ```

pub mod parser;
pub mod store;

pub use parser::CronParser;
pub use store::{CronJob, CronStore};

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use synaptic::events::{Event, EventBus, EventKind};

/// Central cron scheduling service.
///
/// Cheap to clone — all state is behind `Arc`.
#[derive(Clone)]
pub struct CronService {
    store: Arc<CronStore>,
    event_bus: Arc<EventBus>,
}

impl CronService {
    /// Create a new service backed by `store` and `event_bus`.
    pub fn new(store: Arc<CronStore>, event_bus: Arc<EventBus>) -> Self {
        Self { store, event_bus }
    }

    /// Initialize `next_run` for all jobs that do not yet have one.
    ///
    /// Should be called once after jobs are loaded / added at startup.
    pub fn initialize_next_runs(&self) {
        let now = Utc::now();
        let jobs = self.store.list();
        for job in jobs {
            if job.next_run.is_none() && job.enabled {
                let next = CronParser::next_after(&job.expression, now);
                self.store.update(&job.id, |j| j.next_run = next);
            }
        }
    }

    /// Check for due jobs and fire them.
    ///
    /// This is the main driver method; call it once per minute (or more
    /// frequently — duplicate fires within the same minute are harmless
    /// because `next_run` is advanced after each fire).
    pub async fn tick(&self) {
        let now = Utc::now();
        let jobs = self.store.enabled_jobs();

        for job in jobs {
            let due = match job.next_run {
                Some(next) => next <= now,
                // If next_run is unset fall back to expression-based check
                None => CronParser::matches(&job.expression, &now),
            };

            if !due {
                continue;
            }

            tracing::info!(
                job_id = %job.id,
                agent_id = %job.agent_id,
                expression = %job.expression,
                "cron job firing"
            );

            // Emit event — subscribers handle actual agent dispatch.
            let payload = json!({
                "type":     "cron_job_fired",
                "job_id":   job.id,
                "agent_id": job.agent_id,
                "channel":  job.channel,
                "message":  job.message,
            });
            // Use MessageReceived (Parallel/fire-and-forget) as the event kind.
            // The `type` field in the payload discriminates cron events from
            // real inbound messages.
            let mut event =
                Event::new(EventKind::MessageReceived, payload).with_source("cron_service");

            let fire_result = self.event_bus.emit(&mut event).await;

            match fire_result {
                Ok(_) => {
                    // Success — update last_run, recompute next_run, reset failure_count
                    let next = CronParser::next_after(&job.expression, now);
                    self.store.update(&job.id, |j| {
                        j.last_run = Some(now);
                        j.next_run = next;
                        j.failure_count = 0;
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        job_id = %job.id,
                        error = %e,
                        failure_count = job.failure_count + 1,
                        max_retries = job.max_retries,
                        "cron job fire failed"
                    );
                    self.store.update(&job.id, |j| {
                        j.failure_count += 1;
                        if j.failure_count > j.max_retries {
                            tracing::error!(
                                job_id = %j.id,
                                "cron job exceeded max retries — disabling"
                            );
                            j.enabled = false;
                        }
                    });
                }
            }
        }
    }

    /// Add or replace a job in the store.
    ///
    /// `next_run` is computed from the expression if not already set.
    pub fn add_job(&self, mut job: CronJob) {
        if job.next_run.is_none() && job.enabled {
            job.next_run = CronParser::next_after(&job.expression, Utc::now());
        }
        self.store.upsert(job);
    }

    /// Remove the job with the given id.  Returns `true` if a job was removed.
    pub fn remove_job(&self, id: &str) -> bool {
        self.store.remove(id)
    }

    /// Return a snapshot of all jobs (enabled and disabled).
    pub fn list_jobs(&self) -> Vec<CronJob> {
        self.store.list()
    }

    /// Return a reference to the underlying store.
    pub fn store(&self) -> &Arc<CronStore> {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use synaptic::core::SynapticError;
    use synaptic::events::{EventAction, EventFilter, EventSubscriber};

    struct EventCounter(Arc<AtomicU32>);

    #[async_trait::async_trait]
    impl EventSubscriber for EventCounter {
        fn subscriptions(&self) -> Vec<EventFilter> {
            vec![EventFilter::Exact(EventKind::MessageReceived)]
        }
        async fn handle(&self, _event: &mut Event) -> Result<EventAction, SynapticError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(EventAction::Continue)
        }
    }

    fn make_service() -> (CronService, Arc<AtomicU32>) {
        let store = Arc::new(CronStore::new());
        let bus = Arc::new(EventBus::new());
        let counter = Arc::new(AtomicU32::new(0));
        bus.subscribe(Arc::new(EventCounter(counter.clone())), 0, "test_counter");
        let svc = CronService::new(store, bus);
        (svc, counter)
    }

    #[test]
    fn add_and_list_jobs() {
        let (svc, _) = make_service();
        let job = CronJob::new("j1", "* * * * *", "agent-a", "ping");
        svc.add_job(job);
        let jobs = svc.list_jobs();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "j1");
    }

    #[test]
    fn remove_job() {
        let (svc, _) = make_service();
        svc.add_job(CronJob::new("j1", "* * * * *", "a", "m"));
        assert!(svc.remove_job("j1"));
        assert!(!svc.remove_job("j1"));
        assert!(svc.list_jobs().is_empty());
    }

    #[tokio::test]
    async fn tick_fires_due_job() {
        let (svc, counter) = make_service();

        // Force a job that's always due by setting next_run in the past.
        let mut job = CronJob::new("j1", "* * * * *", "agent-a", "hello");
        job.next_run = Some(Utc::now() - chrono::Duration::minutes(1));
        svc.store.upsert(job);

        svc.tick().await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        // next_run should have been advanced
        let jobs = svc.list_jobs();
        assert!(jobs[0].next_run.unwrap() > Utc::now());
        assert!(jobs[0].last_run.is_some());
    }

    #[tokio::test]
    async fn tick_skips_future_job() {
        let (svc, counter) = make_service();

        let mut job = CronJob::new("j1", "* * * * *", "agent-a", "hello");
        job.next_run = Some(Utc::now() + chrono::Duration::hours(1));
        svc.store.upsert(job);

        svc.tick().await;

        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tick_skips_disabled_job() {
        let (svc, counter) = make_service();

        let mut job = CronJob::new("j1", "* * * * *", "agent-a", "hello");
        job.next_run = Some(Utc::now() - chrono::Duration::minutes(1));
        job.enabled = false;
        svc.store.upsert(job);

        svc.tick().await;

        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
