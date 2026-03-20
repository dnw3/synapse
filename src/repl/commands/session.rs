use colored::Colorize;
use synaptic::core::{MemoryStore, Message};
use synaptic::memory::ChatMessageHistory;
use synaptic::session::SessionManager;

use super::CommandResult;
use crate::repl::session::{list_sessions, prune_sessions, send_to_session, view_session_history};

pub async fn cmd_new(
    session_mgr: &SessionManager,
    messages: &mut Vec<Message>,
    session_id: &mut String,
) -> CommandResult {
    match session_mgr.create_session().await {
        Ok(new_sid) => {
            let system_msg = messages.iter().find(|m| m.is_system()).cloned();
            messages.clear();
            if let Some(sys) = system_msg {
                messages.push(sys);
            }

            *session_id = new_sid;
            eprintln!(
                "{} New session created: {}",
                "new:".green().bold(),
                session_id.cyan()
            );
        }
        Err(e) => {
            eprintln!(
                "{} Failed to create new session: {}",
                "error:".red().bold(),
                e
            );
        }
    }
    CommandResult::Continue
}

pub async fn cmd_session(
    memory: &ChatMessageHistory,
    session_mgr: &SessionManager,
    session_id: &str,
) -> CommandResult {
    let count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
    println!("{} {}", "Session ID:".bold(), session_id.cyan());
    if let Ok(Some(info)) = session_mgr.get_session(session_id).await {
        println!("{} {}", "Created:".bold(), info.created_at.dimmed());
    }
    println!("{} {}", "Messages:".bold(), count);
    CommandResult::Continue
}

pub async fn cmd_sessions(session_mgr: &SessionManager) -> CommandResult {
    if let Err(e) = list_sessions(session_mgr).await {
        eprintln!("{} {}", "error:".red().bold(), e);
    }
    CommandResult::Continue
}

pub fn cmd_compact(arg: &str, messages: &mut Vec<Message>) -> CommandResult {
    let keep = if arg.is_empty() {
        20
    } else {
        arg.parse::<usize>().unwrap_or(20)
    };

    let system_msg = messages.iter().find(|m| m.is_system()).cloned();
    let non_system: Vec<Message> = messages
        .iter()
        .filter(|m| !m.is_system())
        .cloned()
        .collect();
    let total_non_system = non_system.len();
    let kept: Vec<Message> = non_system
        .into_iter()
        .skip(total_non_system.saturating_sub(keep))
        .collect();

    messages.clear();
    if let Some(sys) = system_msg {
        messages.push(sys);
    }
    messages.extend(kept);

    eprintln!(
        "{} Compacted to {} messages (kept last {})",
        "compact:".green().bold(),
        messages.len(),
        keep
    );
    CommandResult::Continue
}

pub fn cmd_clear(messages: &mut Vec<Message>) -> CommandResult {
    let system_msg = messages.iter().find(|m| m.is_system()).cloned();
    messages.clear();
    if let Some(sys) = system_msg {
        messages.push(sys);
    }
    eprintln!("{} Conversation history cleared", "clear:".green().bold());
    CommandResult::Continue
}

pub async fn cmd_history(arg: &str, session_mgr: &SessionManager) -> CommandResult {
    if arg.is_empty() {
        eprintln!("{} Usage: /history <session_id>", "usage:".yellow().bold());
    } else {
        let target_sid = arg.split_whitespace().next().unwrap_or(arg);
        match view_session_history(session_mgr, target_sid).await {
            Ok(()) => {}
            Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
        }
    }
    CommandResult::Continue
}

pub async fn cmd_send(arg: &str, session_mgr: &SessionManager) -> CommandResult {
    let parts: Vec<&str> = arg.splitn(2, ' ').collect();
    if parts.len() < 2 || parts[1].trim().is_empty() {
        eprintln!(
            "{} Usage: /send <session_id> <message>",
            "usage:".yellow().bold()
        );
    } else {
        let target_sid = parts[0];
        let msg_text = parts[1].trim();
        match send_to_session(session_mgr, target_sid, msg_text).await {
            Ok(()) => {
                eprintln!(
                    "{} Message sent to session {}",
                    "send:".green().bold(),
                    target_sid.cyan()
                );
            }
            Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
        }
    }
    CommandResult::Continue
}

pub async fn cmd_prune(arg: &str, session_mgr: &SessionManager) -> CommandResult {
    let days = if arg.is_empty() {
        30
    } else {
        arg.parse::<u64>().unwrap_or(30)
    };
    match prune_sessions(session_mgr, days).await {
        Ok(removed) => {
            if removed > 0 {
                eprintln!(
                    "{} Pruned {} session(s) older than {} days",
                    "prune:".green().bold(),
                    removed,
                    days
                );
            } else {
                eprintln!(
                    "{} No sessions older than {} days",
                    "prune:".green().bold(),
                    days
                );
            }
        }
        Err(e) => {
            eprintln!("{} {}", "error:".red().bold(), e);
        }
    }
    CommandResult::Continue
}
