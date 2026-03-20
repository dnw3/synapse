use colored::Colorize;
use synaptic::core::{MemoryStore, Message};
use synaptic::session::SessionManager;

/// List all sessions.
pub async fn list_sessions(mgr: &SessionManager) -> Result<(), Box<dyn std::error::Error>> {
    let sessions = mgr.list_sessions().await.map_err(|e| format!("{}", e))?;
    if sessions.is_empty() {
        println!("{}", "No sessions found.".dimmed());
        return Ok(());
    }

    let memory = mgr.memory();
    let store = mgr.store();

    println!("{}", "Sessions:".bold());
    for s in &sessions {
        let count = memory
            .load(&s.session_id)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let title = load_session_title(store, &s.session_id).await;
        let title_display = title.as_deref().unwrap_or("(untitled)");
        let token_info = if s.total_tokens > 0 {
            format!(", ~{}tok", s.total_tokens)
        } else {
            String::new()
        };
        let compact_info = if s.compaction_count > 0 {
            format!(", {}x compacted", s.compaction_count)
        } else {
            String::new()
        };
        println!(
            "  {} {} {} ({} messages{}{})",
            s.session_id.cyan(),
            title_display.bold(),
            s.created_at.dimmed(),
            count,
            token_info,
            compact_info,
        );
    }
    Ok(())
}

/// Prune sessions older than the given number of days.
pub async fn prune_sessions(
    mgr: &SessionManager,
    max_age_days: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    let sessions = mgr.list_sessions().await.map_err(|e| format!("{}", e))?;

    let cutoff = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now - (max_age_days * 86400)
    };

    let mut removed = 0;
    for s in &sessions {
        if let Some(timestamp) = parse_iso_timestamp(&s.created_at) {
            if timestamp < cutoff && mgr.delete_session(&s.session_id).await.is_ok() {
                removed += 1;
            }
        }
    }

    Ok(removed)
}

/// Simple ISO 8601 timestamp parser — returns Unix seconds.
fn parse_iso_timestamp(s: &str) -> Option<u64> {
    let s = s.replace('T', " ").replace('Z', "");
    let parts: Vec<&str> = s.split(' ').collect();
    let date_parts: Vec<&str> = parts.first()?.split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    let year: u64 = date_parts[0].parse().ok()?;
    let month: u64 = date_parts[1].parse().ok()?;
    let day: u64 = date_parts[2].parse().ok()?;

    // Rough calculation (not leap-year-accurate, but good enough for pruning)
    let days_since_epoch = (year - 1970) * 365 + (month - 1) * 30 + day;
    Some(days_since_epoch * 86400)
}

/// Save a session title derived from the first user message.
pub async fn save_session_title(
    store: &std::sync::Arc<dyn synaptic::core::Store>,
    session_id: &str,
    first_message: &str,
) {
    let title = if first_message.len() > 60 {
        format!("{}...", &first_message[..57])
    } else {
        first_message.to_string()
    };
    let title = title.replace('\n', " ");
    let ns = &["session_titles"];
    let _ = store
        .put(ns, session_id, serde_json::Value::String(title))
        .await;
}

/// View message history from another session.
pub async fn view_session_history(
    mgr: &SessionManager,
    target_session_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    mgr.get_session(target_session_id)
        .await
        .map_err(|e| format!("{}", e))?
        .ok_or_else(|| format!("session '{}' not found", target_session_id))?;

    let memory = mgr.memory();
    let messages = memory.load(target_session_id).await.unwrap_or_default();

    if messages.is_empty() {
        println!(
            "{} Session {} has no messages",
            "history:".dimmed(),
            target_session_id.cyan()
        );
        return Ok(());
    }

    let store = mgr.store();
    let title = load_session_title(store, target_session_id).await;
    println!(
        "{} Session {} {}",
        "history:".bold(),
        target_session_id.cyan(),
        title.as_deref().unwrap_or("(untitled)").dimmed()
    );
    println!("{}", "-".repeat(60));

    for (i, msg) in messages.iter().enumerate() {
        let role = if msg.is_system() {
            "system".blue()
        } else if msg.is_human() {
            "human".green()
        } else if msg.is_ai() {
            "assistant".cyan()
        } else if msg.is_tool() {
            "tool".magenta()
        } else {
            "unknown".dimmed()
        };

        let content = msg.content();
        let preview = if content.len() > 120 {
            format!("{}...", &content[..117])
        } else {
            content.to_string()
        };
        let preview = preview.replace('\n', " ");
        println!("  {}. [{}] {}", i + 1, role, preview);
    }
    println!("{} message(s)", messages.len());
    Ok(())
}

/// Send a message to another session (inter-session collaboration).
pub async fn send_to_session(
    mgr: &SessionManager,
    target_session_id: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    mgr.get_session(target_session_id)
        .await
        .map_err(|e| format!("{}", e))?
        .ok_or_else(|| format!("session '{}' not found", target_session_id))?;

    let memory = mgr.memory();
    let msg = Message::human(format!("[from another session] {}", message));
    memory
        .append(target_session_id, msg)
        .await
        .map_err(|e| format!("failed to send: {}", e))?;
    Ok(())
}

/// Load a session title.
pub async fn load_session_title(
    store: &std::sync::Arc<dyn synaptic::core::Store>,
    session_id: &str,
) -> Option<String> {
    let ns = &["session_titles"];
    store
        .get(ns, session_id)
        .await
        .ok()
        .flatten()
        .and_then(|item| item.value.as_str().map(|s| s.to_string()))
}
