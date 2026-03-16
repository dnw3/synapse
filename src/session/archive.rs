/// Archive a session transcript by renaming the file
#[allow(dead_code)]
pub async fn archive_transcript(session_id: &str, reason: &str) {
    let sessions_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse")
        .join("sessions");

    let transcript = sessions_dir.join(format!("{}.jsonl", session_id));
    if !transcript.exists() {
        // Also check inside subdirectory
        let alt = sessions_dir.join(session_id);
        if alt.is_dir() {
            // Session stored as directory — archive the whole dir
            let archive_name = format!(
                "{}.{}.{}",
                session_id,
                reason,
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            );
            let archive_path = sessions_dir.join(archive_name);
            if let Err(e) = tokio::fs::rename(&alt, &archive_path).await {
                tracing::warn!(error = %e, session_id, "failed to archive session directory");
            }
        }
        return;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let archive_name = format!("{}.{}.{}.jsonl", session_id, reason, timestamp);
    let archive_path = sessions_dir.join(archive_name);

    if let Err(e) = tokio::fs::rename(&transcript, &archive_path).await {
        tracing::warn!(error = %e, session_id, reason, "failed to archive transcript");
    } else {
        tracing::info!(session_id, reason, archive = %archive_path.display(), "transcript archived");
    }
}

/// Cleanup old archives (older than retention_days)
#[allow(dead_code)]
pub async fn cleanup_old_archives(retention_days: u32) {
    let sessions_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse")
        .join("sessions");

    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(
            retention_days as u64 * 86400,
        ))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut dir = match tokio::fs::read_dir(&sessions_dir).await {
        Ok(d) => d,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = dir.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        // Archive files have pattern: {uuid}.{reason}.{timestamp}.jsonl
        if name.contains(".reset.")
            || name.contains(".deleted.")
            || name.contains(".idle.")
            || name.contains(".daily.")
            || name.contains(".user.")
        {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    if modified < cutoff {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                        tracing::debug!(file = %name, "cleaned up old archive");
                    }
                }
            }
        }
    }
}
