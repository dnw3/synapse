use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::outbound::OutboundPayload;

/// Mirror delivery to additional targets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryMirror {
    pub channel: String,
    pub to: String,
    pub account_id: Option<String>,
}

/// A persistent delivery task with retry semantics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedDelivery {
    pub id: String,
    pub channel: String,
    pub to: String,
    pub account_id: Option<String>,
    pub thread_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub payloads: Vec<OutboundPayload>,
    pub best_effort: bool,
    pub silent: bool,
    pub gif_playback: bool,
    pub force_document: bool,
    pub mirror: Option<DeliveryMirror>,
    pub enqueued_at: u64,
    pub retry_count: u32,
    pub last_attempt_at: Option<u64>,
    pub last_error: Option<String>,
}

/// Persistent delivery queue with disk-backed storage and retry support.
pub struct DeliveryQueue {
    queue_dir: PathBuf,
    pending: Vec<QueuedDelivery>,
}

impl DeliveryQueue {
    pub fn new(queue_dir: PathBuf) -> Self {
        Self {
            queue_dir,
            pending: Vec::new(),
        }
    }

    /// Enqueue a delivery — persists to disk before returning.
    pub fn enqueue(&mut self, delivery: QueuedDelivery) -> Result<(), std::io::Error> {
        let path = self.queue_dir.join(format!("{}.json", delivery.id));
        std::fs::create_dir_all(&self.queue_dir)?;
        let json = serde_json::to_string_pretty(&delivery)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, json)?;
        self.pending.push(delivery);
        Ok(())
    }

    /// Mark a delivery as complete — removes from disk and pending list.
    pub fn mark_complete(&mut self, id: &str) {
        let path = self.queue_dir.join(format!("{}.json", id));
        let _ = std::fs::remove_file(path);
        self.pending.retain(|d| d.id != id);
    }

    /// Mark a delivery as failed — updates retry info, persists.
    pub fn mark_failed(&mut self, id: &str, error: &str) {
        if let Some(d) = self.pending.iter_mut().find(|d| d.id == id) {
            d.retry_count += 1;
            d.last_error = Some(error.to_string());
            d.last_attempt_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
            // Re-persist
            let path = self.queue_dir.join(format!("{}.json", d.id));
            if let Ok(json) = serde_json::to_string_pretty(d) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Load pending deliveries from disk (call on startup).
    pub fn load_pending(&mut self) -> Result<(), std::io::Error> {
        if !self.queue_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.queue_dir)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .map(|e| e == "json")
                .unwrap_or(false)
            {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(delivery) = serde_json::from_str::<QueuedDelivery>(&content) {
                        self.pending.push(delivery);
                    }
                }
            }
        }
        Ok(())
    }

    /// Get all pending deliveries.
    pub fn pending_deliveries(&self) -> &[QueuedDelivery] {
        &self.pending
    }
}
