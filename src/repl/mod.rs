pub mod commands;
pub mod run;
pub mod session;
pub mod skills;
pub mod state;

// Re-exports for backward compatibility
pub use self::run::repl;
pub use self::session::{list_sessions, prune_sessions, save_session_title};

use std::sync::Arc;

use colored::Colorize;
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, MemoryStore, Message};
use synaptic::memory::ChatMessageHistory;

use crate::memory::LongTermMemory;

/// Run single-shot chat: send one message, print response, exit.
pub async fn single_shot(
    model: Arc<dyn ChatModel>,
    memory: &ChatMessageHistory,
    session_id: &str,
    messages: &mut Vec<Message>,
    user_message: &str,
    ltm: &LongTermMemory,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("{} {}", "Session:".bold(), session_id.cyan());

    // Recall relevant long-term memories
    let recalled = ltm.recall(user_message, 3).await;
    if !recalled.is_empty() {
        let context = recalled.join("\n- ");
        let mem_msg = Message::system(format!(
            "Relevant memories from past conversations:\n- {}",
            context
        ));
        messages.push(mem_msg);
    }

    let human_msg = Message::human(user_message);
    memory.append(session_id, human_msg.clone()).await.ok();
    messages.push(human_msg);

    let request = ChatRequest::new(messages.clone());
    let mut stream = model.stream_chat(request);

    let mut full_response = String::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(c) => {
                print!("{}", c.content);
                full_response.push_str(&c.content);
            }
            Err(e) => {
                eprintln!("\n{} {}", "error:".red().bold(), e);
                return Err(e.into());
            }
        }
    }
    println!();

    let ai_msg = Message::ai(&full_response);
    memory.append(session_id, ai_msg).await.ok();

    // Auto-save important messages
    if LongTermMemory::is_important(&full_response) {
        ltm.remember(&full_response).await.ok();
    }

    Ok(())
}

/// Get today's date as "YYYY-MM-DD" string (for daily reset comparison).
fn chrono_today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let remaining_days = days - years * 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
}

/// Format a byte size into a human-readable string.
pub(crate) fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
