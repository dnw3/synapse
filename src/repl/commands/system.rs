use std::sync::Arc;

use colored::Colorize;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{HeuristicTokenCounter, MemoryStore, Message, TokenCounter};
use synaptic::memory::ChatMessageHistory;

use super::CommandResult;
use crate::config::SynapseConfig;
use crate::gateway::usage;
use crate::memory::LongTermMemory;

pub fn cmd_help(config: &SynapseConfig) -> CommandResult {
    println!("{}", "--- Commands ---".bold());
    println!("  {} -- Exit the REPL", "/quit".cyan());
    println!(
        "  {} -- Start a new session (clear history, new ID)",
        "/new".cyan()
    );
    println!("  {} -- Show current session info", "/session".cyan());
    println!("  {} -- List all sessions", "/sessions".cyan());
    println!(
        "  {} -- Compact conversation history (keep last N turns)",
        "/compact".cyan()
    );
    println!("  {} -- Show token usage statistics", "/usage".cyan());
    println!(
        "  {} -- Show session status and model info",
        "/status".cyan()
    );
    println!(
        "  {} -- Show context budget details (tokens, message types, LTM)",
        "/context".cyan()
    );
    println!(
        "  {} -- Switch model, or: list, aliases, status",
        "/model".cyan()
    );
    println!(
        "  {} -- Set thinking level (off/low/medium/high)",
        "/think".cyan()
    );
    println!(
        "  {} -- Toggle verbose output (token counts, timing)",
        "/verbose".cyan()
    );
    println!("  {} -- Clear conversation history", "/clear".cyan());
    println!(
        "  {} -- View messages from another session",
        "/history".cyan()
    );
    println!("  {} -- Send a message to another session", "/send".cyan());
    println!("  {} -- Prune sessions older than N days", "/prune".cyan());
    println!("  {} -- Delete memories matching keyword", "/forget".cyan());
    println!(
        "  {} -- List all memories (/memories clear to wipe)",
        "/memories".cyan()
    );
    println!(
        "  {} -- List / inspect skills (/skill list, /skill info <name>)",
        "/skill".cyan()
    );
    println!("  {} -- List configured sub-agents", "/subagents".cyan());
    #[cfg(feature = "web")]
    {
        println!("  {} -- Generate device pairing QR code", "/pair".cyan());
        println!(
            "  {} -- List/approve/reject paired devices",
            "/pair list|approve|reject|remove".cyan()
        );
        println!(
            "  {} -- Manage DM pairing for bot channels",
            "/dm list|approve|allowlist|remove <channel>".cyan()
        );
    }
    #[cfg(feature = "sandbox")]
    {
        println!(
            "  {} -- Manage sandbox (list, recreate, explain, status)",
            "/sandbox".cyan()
        );
    }
    if let Some(ref commands) = config.commands {
        if !commands.is_empty() {
            println!();
            println!("{}", "--- Custom Commands ---".bold());
            for cmd in commands {
                println!(
                    "  {} -- {}",
                    format!("/{}", cmd.name).cyan(),
                    cmd.description
                );
            }
        }
    }
    CommandResult::Continue
}

pub async fn cmd_usage(tracker: &Arc<CostTrackingCallback>) -> CommandResult {
    let snapshot = tracker.snapshot().await;
    usage::display_usage(&snapshot);
    CommandResult::Continue
}

pub async fn cmd_status(
    config: &SynapseConfig,
    memory: &ChatMessageHistory,
    session_id: &str,
    messages: &[Message],
    current_model_name: &str,
) -> CommandResult {
    let count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
    println!("{}", "--- Status ---".bold());
    println!("  {} {}", "Session:".bold(), session_id.cyan());
    println!("  {} {}", "Model:".bold(), current_model_name.cyan());
    println!(
        "  {} {}",
        "Provider:".bold(),
        config.model_config().provider.dimmed()
    );
    println!("  {} {}", "Messages:".bold(), count);
    println!("  {} {} in memory", "Context:".bold(), messages.len());
    if let Some(ref fallbacks) = config.fallback_models {
        println!(
            "  {} {}",
            "Fallbacks:".bold(),
            fallbacks.join(", ").dimmed()
        );
    }
    if let Some(mcps) = config.mcp_configs() {
        if !mcps.is_empty() {
            let names: Vec<&str> = mcps.iter().map(|m| m.name.as_str()).collect();
            println!("  {} {}", "MCP servers:".bold(), names.join(", ").dimmed());
        }
    }
    CommandResult::Continue
}

pub async fn cmd_context(messages: &[Message], ltm: &LongTermMemory) -> CommandResult {
    let counter = HeuristicTokenCounter;

    let sys_tokens = messages
        .iter()
        .filter(|m| m.is_system())
        .map(|m| counter.count_text(m.content()))
        .sum::<usize>();

    let human_count = messages.iter().filter(|m| m.is_human()).count();
    let ai_count = messages.iter().filter(|m| m.is_ai()).count();
    let tool_count = messages.iter().filter(|m| m.is_tool()).count();

    let tool_chars: usize = messages
        .iter()
        .filter(|m| m.is_tool())
        .map(|m| m.content().len())
        .sum();

    let total_tokens = counter.count_messages(messages);
    let ltm_count = ltm.count().await;

    let cwd = std::env::current_dir().unwrap_or_default();
    let bootstrap_files = ["AGENTS.md", "MEMORY.md", ".synapse/context.md", "README.md"];
    let mut loaded_files = Vec::new();
    for name in &bootstrap_files {
        let path = cwd.join(name);
        if path.exists() {
            if let Ok(meta) = std::fs::metadata(&path) {
                loaded_files.push(format!(
                    "{} ({})",
                    name,
                    crate::repl::format_size(meta.len())
                ));
            }
        }
    }

    println!("{}", "--- Context Budget ---".bold());
    println!("  {} ~{}", "System prompt tokens:".bold(), sys_tokens);
    println!(
        "  {} {} human, {} assistant, {} tool",
        "Messages:".bold(),
        human_count,
        ai_count,
        tool_count
    );
    println!(
        "  {} {} chars (~{} tokens)",
        "Tool results:".bold(),
        tool_chars,
        tool_chars / 4
    );
    println!("  {} ~{}", "Total estimated tokens:".bold(), total_tokens);
    println!("  {} {}", "LTM entries:".bold(), ltm_count);
    if !loaded_files.is_empty() {
        println!(
            "  {} {}",
            "Bootstrap files:".bold(),
            loaded_files.join(", ")
        );
    }
    CommandResult::Continue
}
