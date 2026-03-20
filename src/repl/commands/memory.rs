use colored::Colorize;

use super::CommandResult;
use crate::memory::LongTermMemory;

pub async fn cmd_forget(arg: &str, ltm: &LongTermMemory) -> CommandResult {
    if arg.is_empty() {
        eprintln!("{} Usage: /forget <keyword>", "usage:".yellow().bold());
    } else {
        match ltm.forget(arg).await {
            Ok(removed) => {
                if removed > 0 {
                    eprintln!(
                        "{} Forgot {} memory(ies) matching '{}'",
                        "forget:".green().bold(),
                        removed,
                        arg
                    );
                } else {
                    eprintln!(
                        "{} No memories found matching '{}'",
                        "forget:".green().bold(),
                        arg
                    );
                }
            }
            Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
        }
    }
    CommandResult::Continue
}

pub async fn cmd_memories(arg: &str, ltm: &LongTermMemory) -> CommandResult {
    if arg == "clear" {
        match ltm.clear_all().await {
            Ok(count) => {
                eprintln!("{} Cleared {} memories", "memories:".green().bold(), count);
            }
            Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
        }
    } else {
        let memories = ltm.list().await;
        if memories.is_empty() {
            println!("{}", "No long-term memories stored.".dimmed());
        } else {
            println!(
                "{} ({} total)",
                "Long-term Memories:".bold(),
                memories.len()
            );
            for (i, (key, content)) in memories.iter().enumerate() {
                let preview = if content.len() > 100 {
                    format!("{}...", &content[..97])
                } else {
                    content.clone()
                };
                let preview = preview.replace('\n', " ");
                println!("  {}. [{}] {}", i + 1, key.dimmed(), preview);
            }
        }
    }
    CommandResult::Continue
}
