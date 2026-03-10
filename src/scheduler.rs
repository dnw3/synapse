//! Cron/interval-based job scheduling.
//!
//! Reads `[[schedule]]` entries from config and registers them
//! with the Synaptic scheduler.
//!
//! When `gateway.leader_election` is enabled, uses a simple file-lock based
//! leader election so that only one instance runs scheduled jobs.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use synaptic::core::ChatModel;
use synaptic::scheduler::{Scheduler, SchedulerTask, TokioScheduler};

use crate::config::SynapseConfig;

/// A scheduled job that runs an agent with a predefined prompt.
struct AgentTask {
    model: Arc<dyn ChatModel>,
    prompt: String,
}

#[async_trait]
impl SchedulerTask for AgentTask {
    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let messages = vec![
            synaptic::core::Message::system("You are Synapse, executing a scheduled task."),
            synaptic::core::Message::human(&self.prompt),
        ];
        let request = synaptic::core::ChatRequest::new(messages);
        match self.model.chat(request).await {
            Ok(response) => {
                tracing::info!(result = %response.message.content(), "scheduler task completed");
            }
            Err(e) => {
                tracing::error!(error = %e, "scheduler task failed");
            }
        }
        Ok(())
    }
}

/// Path for the leader lock file.
const LEADER_LOCK_FILE: &str = ".synapse_leader";

/// Attempt to acquire leader status via a simple file lock.
///
/// Writes `instance_id` into `.synapse_leader`. If the file already exists
/// and contains a different instance_id, this instance is not the leader.
///
/// Returns `true` if this instance is (or became) the leader.
fn try_acquire_leader(instance_id: &str) -> bool {
    let lock_path = PathBuf::from(LEADER_LOCK_FILE);

    // Check if lock file exists and belongs to another instance
    if lock_path.exists() {
        if let Ok(existing) = fs::read_to_string(&lock_path) {
            let existing = existing.trim();
            if existing == instance_id {
                return true; // We already hold the lock
            }
            // Another instance holds the lock — not leader
            tracing::info!(
                leader = %existing,
                instance = %instance_id,
                "leader lock held by another instance, skipping scheduling"
            );
            return false;
        }
    }

    // Try to create/overwrite the lock file
    match fs::File::create(&lock_path).and_then(|mut f| f.write_all(instance_id.as_bytes())) {
        Ok(()) => {
            tracing::info!(instance = %instance_id, "acquired leader lock");
            true
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to acquire leader lock");
            false
        }
    }
}

/// Start the scheduler with jobs from config.
///
/// If `gateway.leader_election` is enabled, only starts the scheduler if this
/// instance acquires the leader lock (file-based).
pub async fn start_scheduler(
    config: &SynapseConfig,
    model: Arc<dyn ChatModel>,
) -> Result<Arc<TokioScheduler>, Box<dyn std::error::Error>> {
    // Check leader election gate
    if let Some(ref gw) = config.gateway {
        if gw.leader_election.unwrap_or(false) {
            let instance_id = gw
                .instance_id
                .as_deref()
                .unwrap_or("default");

            if !try_acquire_leader(instance_id) {
                tracing::info!(
                    instance = %instance_id,
                    "skipping scheduler start, not the leader"
                );
                // Return an empty scheduler (no jobs registered)
                return Ok(Arc::new(TokioScheduler::new()));
            }
        }
    }

    let scheduler = Arc::new(TokioScheduler::new());

    if let Some(schedules) = &config.schedules {
        for entry in schedules {
            let task = Box::new(AgentTask {
                model: model.clone(),
                prompt: entry.prompt.clone(),
            });

            let job_id = if let Some(ref cron) = entry.cron {
                scheduler.schedule_cron(cron, &entry.name, task).await?
            } else if let Some(secs) = entry.interval_secs {
                scheduler
                    .schedule_interval(Duration::from_secs(secs), &entry.name, task)
                    .await?
            } else {
                tracing::warn!(name = %entry.name, "skipping schedule entry: no cron or interval_secs");
                continue;
            };

            tracing::info!(name = %entry.name, job_id = %job_id, "registered scheduled job");
        }
    }

    Ok(scheduler)
}
