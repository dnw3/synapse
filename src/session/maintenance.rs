use synaptic::session::SessionManager;

/// Session store maintenance: prune stale entries and cap total count.
#[allow(dead_code)]
pub struct SessionMaintenance {
    pub prune_after_days: u32, // Remove sessions older than N days (default 30)
    pub max_entries: usize,    // Keep max N sessions (default 500)
}

impl Default for SessionMaintenance {
    fn default() -> Self {
        Self {
            prune_after_days: 30,
            max_entries: 500,
        }
    }
}

#[allow(dead_code)]
impl SessionMaintenance {
    /// Run maintenance: prune stale + cap entries
    pub async fn run(&self, session_mgr: &SessionManager) -> MaintenanceResult {
        let mut result = MaintenanceResult::default();
        let sessions = match session_mgr.list_sessions().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to list sessions for maintenance");
                return result;
            }
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let cutoff_ms = now_ms.saturating_sub(self.prune_after_days as u64 * 86400 * 1000);

        // Prune stale entries
        for s in &sessions {
            if s.updated_at > 0 && s.updated_at < cutoff_ms {
                if let Err(e) = session_mgr.delete_session(&s.session_id).await {
                    tracing::warn!(session_id = %s.session_id, error = %e, "failed to prune stale session");
                } else {
                    result.pruned += 1;
                }
            }
        }

        // Cap entries (keep most recent by updated_at)
        let remaining = match session_mgr.list_sessions().await {
            Ok(s) => s,
            Err(_) => return result,
        };

        if remaining.len() > self.max_entries {
            let mut sorted = remaining;
            sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            for s in sorted.iter().skip(self.max_entries) {
                if let Err(e) = session_mgr.delete_session(&s.session_id).await {
                    tracing::warn!(session_id = %s.session_id, error = %e, "failed to cap session");
                } else {
                    result.capped += 1;
                }
            }
        }

        if result.pruned > 0 || result.capped > 0 {
            tracing::info!(
                pruned = result.pruned,
                capped = result.capped,
                "session maintenance completed"
            );
        }

        result
    }
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct MaintenanceResult {
    pub pruned: usize,
    pub capped: usize,
}
