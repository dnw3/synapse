use std::sync::Arc;

use synaptic::session::{SessionInfo, SessionManager};

#[allow(dead_code)]
pub struct ResetService;

#[allow(dead_code)]
impl ResetService {
    /// Reset a session: archive transcript, generate new session_id, preserve session_key + metadata.
    ///
    /// `reason` should be one of `"daily"`, `"idle"`, or `"user"`.
    ///
    /// If `memory_provider` is given, memories are committed before the transcript is archived
    /// so that important context is preserved in long-term storage.
    pub async fn reset(
        session_mgr: &SessionManager,
        session_key: &str,
        old_info: &SessionInfo,
        reason: &str,
        memory_provider: Option<&Arc<dyn synaptic::memory::MemoryProvider>>,
    ) -> crate::error::Result<SessionInfo> {
        let old_session_id = &old_info.session_id;

        // 0. Before archiving, commit memories so important context is persisted
        if let Some(memory) = memory_provider {
            match memory.commit(session_key).await {
                Ok(result) => {
                    tracing::info!(
                        session_key,
                        extracted = result.memories_extracted,
                        merged = result.memories_merged,
                        "memory commit on session reset"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, session_key, "memory commit failed on reset");
                }
            }
        }

        // 1. Archive old transcript
        crate::session::archive::archive_transcript(old_session_id, reason).await;

        // 2. Create new session_id
        let new_session_id = session_mgr.create_session().await?;

        // 3. Copy metadata from old session to new, reset runtime state
        let mut new_info = session_mgr
            .get_session(&new_session_id)
            .await?
            .unwrap_or_default();

        new_info.session_key = old_info.session_key.clone();
        new_info.channel = old_info.channel.clone();
        new_info.chat_type = old_info.chat_type.clone();
        new_info.display_name = old_info.display_name.clone();
        new_info.label = old_info.label.clone();
        new_info.last_channel = old_info.last_channel.clone();
        new_info.last_to = old_info.last_to.clone();
        new_info.last_account_id = old_info.last_account_id.clone();

        // Reset runtime state
        new_info.system_sent = false;
        new_info.aborted_last_run = false;
        new_info.model = None;
        new_info.model_provider = None;
        new_info.total_tokens = 0;
        new_info.input_tokens = 0;
        new_info.output_tokens = 0;

        session_mgr.update_session(&new_info).await?;

        // 4. Delete old session entry (messages + checkpoints cleaned up by delete_session)
        session_mgr.delete_session(old_session_id).await?;

        tracing::info!(
            session_key = %session_key,
            old_session_id = %old_session_id,
            new_session_id = %new_info.session_id,
            reason = %reason,
            "session reset"
        );

        Ok(new_info)
    }
}
