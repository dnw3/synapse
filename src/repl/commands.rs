use std::sync::Arc;

use colored::Colorize;
use futures::StreamExt;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{
    ChatModel, ChatRequest, HeuristicTokenCounter, MemoryStore, Message, ThinkingConfig,
    TokenCounter,
};
use synaptic::memory::ChatMessageHistory;
use synaptic::session::SessionManager;

use crate::agent;
use crate::config::SynapseConfig;
use crate::memory::LongTermMemory;
use crate::usage;

use super::session::{list_sessions, prune_sessions, send_to_session, view_session_history};
use super::skills::{resolve_skill_slash_command, SkillSlashResult};

pub enum CommandResult {
    Continue,
    Quit,
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_command(
    input: &str,
    config: &SynapseConfig,
    session_mgr: &SessionManager,
    memory: &ChatMessageHistory,
    session_id: &mut String,
    messages: &mut Vec<Message>,
    model: &mut Arc<dyn ChatModel>,
    tracker: &Arc<CostTrackingCallback>,
    current_model_name: &mut String,
    verbose: &mut bool,
    thinking: &mut Option<ThinkingConfig>,
    ltm: &LongTermMemory,
) -> CommandResult {
    let (cmd, arg) = match input.split_once(' ') {
        Some((c, a)) => (c, a.trim()),
        None => (input, ""),
    };

    match cmd {
        "/quit" | "/exit" => {
            eprintln!("{}", "Goodbye.".dimmed());
            CommandResult::Quit
        }

        "/help" => {
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

        "/new" => {
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

        "/session" => {
            let count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
            println!("{} {}", "Session ID:".bold(), session_id.cyan());
            if let Ok(Some(info)) = session_mgr.get_session(session_id).await {
                println!("{} {}", "Created:".bold(), info.created_at.dimmed());
            }
            println!("{} {}", "Messages:".bold(), count);
            CommandResult::Continue
        }

        "/sessions" => {
            if let Err(e) = list_sessions(session_mgr).await {
                eprintln!("{} {}", "error:".red().bold(), e);
            }
            CommandResult::Continue
        }

        "/compact" => {
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

        "/usage" => {
            let snapshot = tracker.snapshot().await;
            usage::display_usage(&snapshot);
            CommandResult::Continue
        }

        "/status" => {
            let count = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
            println!("{}", "--- Status ---".bold());
            println!("  {} {}", "Session:".bold(), session_id.cyan());
            println!("  {} {}", "Model:".bold(), current_model_name.cyan());
            println!(
                "  {} {}",
                "Provider:".bold(),
                config.base.model.provider.dimmed()
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
            if let Some(ref mcps) = config.base.mcp {
                if !mcps.is_empty() {
                    let names: Vec<&str> = mcps.iter().map(|m| m.name.as_str()).collect();
                    println!("  {} {}", "MCP servers:".bold(), names.join(", ").dimmed());
                }
            }
            CommandResult::Continue
        }

        "/context" => {
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
                        loaded_files.push(format!("{} ({})", name, super::format_size(meta.len())));
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

        "/model" => {
            let sub = arg.split_whitespace().next().unwrap_or("");
            match sub {
                "" => {
                    println!("{} {}", "Current model:".bold(), current_model_name.cyan());
                }
                "list" | "ls" => {
                    let registry = agent::registry::ModelRegistry::from_config(config);
                    let entries = registry.list();
                    if entries.is_empty() {
                        println!(
                            "{} No models in catalog. Add [[models]] to config.",
                            "model:".dimmed()
                        );
                    } else {
                        println!("{}", "--- Model Catalog ---".bold());
                        for entry in &entries {
                            let is_current = entry.name == *current_model_name
                                || entry
                                    .aliases
                                    .iter()
                                    .any(|a| a == current_model_name.as_str());
                            let marker = if is_current { " *" } else { "" };
                            let aliases = if entry.aliases.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", entry.aliases.join(", "))
                            };
                            let provider = entry.provider.as_deref().unwrap_or("-");
                            println!(
                                "  {} [{}]{}{}",
                                entry.name.cyan(),
                                provider.dimmed(),
                                aliases.dimmed(),
                                marker.green()
                            );
                        }
                    }
                }
                "aliases" => {
                    let registry = agent::registry::ModelRegistry::from_config(config);
                    let aliases = registry.aliases();
                    if aliases.is_empty() {
                        println!("{}", "No aliases defined.".dimmed());
                    } else {
                        println!("{}", "--- Model Aliases ---".bold());
                        for (alias, canonical) in &aliases {
                            println!("  {} -> {}", alias.cyan(), canonical);
                        }
                    }
                }
                "status" => {
                    let registry = agent::registry::ModelRegistry::from_config(config);
                    println!("{}", "--- Model Status ---".bold());
                    println!("  {} {}", "Current:".bold(), current_model_name.cyan());
                    println!(
                        "  {} {}",
                        "Provider:".bold(),
                        config.base.model.provider.dimmed()
                    );
                    if let Some(temp) = config.base.model.temperature {
                        println!("  {} {}", "Temperature:".bold(), temp);
                    }
                    if let Some(max) = config.base.model.max_tokens {
                        println!("  {} {}", "Max tokens:".bold(), max);
                    }
                    if let Some(ref fallbacks) = config.fallback_models {
                        println!(
                            "  {} {}",
                            "Fallbacks:".bold(),
                            fallbacks.join(", ").dimmed()
                        );
                    }
                    // Show registry provider info if model is from catalog
                    if let Some(prov) = registry.provider_for(current_model_name) {
                        println!("  {} {}", "Base URL:".bold(), prov.base_url.dimmed());
                        let key_status = if prov.api_keys_env.is_some() {
                            "multi-key rotation"
                        } else if prov.api_key_env.is_some() {
                            "single key"
                        } else {
                            "default"
                        };
                        println!("  {} {}", "Key mode:".bold(), key_status);
                    }
                    println!("  {} {}", "Catalog size:".bold(), registry.list().len());
                }
                _ => {
                    // Switch model (name or alias)
                    match agent::build_model_by_name(config, arg) {
                        Ok(new_model) => {
                            // Resolve to canonical name if it's an alias
                            let registry = agent::registry::ModelRegistry::from_config(config);
                            let display_name = registry.canonical_name(arg).unwrap_or(arg);
                            *model = new_model;
                            *current_model_name = display_name.to_string();
                            tracker.set_model(display_name).await;
                            if display_name != arg {
                                eprintln!(
                                    "{} Switched to {} (alias: {})",
                                    "model:".green().bold(),
                                    display_name.cyan(),
                                    arg.dimmed()
                                );
                            } else {
                                eprintln!("{} Switched to {}", "model:".green().bold(), arg.cyan());
                            }
                        }
                        Err(e) => {
                            eprintln!("{} Failed to switch model: {}", "error:".red().bold(), e);
                        }
                    }
                }
            }
            CommandResult::Continue
        }

        "/verbose" => {
            *verbose = !*verbose;
            eprintln!(
                "{} Verbose mode {}",
                "verbose:".green().bold(),
                if *verbose { "enabled" } else { "disabled" }
            );
            CommandResult::Continue
        }

        "/think" => {
            let level = if arg.is_empty() { "medium" } else { arg };
            match level {
                "off" | "none" => {
                    *thinking = None;
                    messages
                        .retain(|m| !(m.is_system() && m.content().starts_with("[Thinking mode:")));
                    eprintln!("{} Thinking mode disabled", "think:".green().bold());
                }
                "low" | "minimal" => {
                    *thinking = Some(ThinkingConfig {
                        enabled: true,
                        budget_tokens: Some(2000),
                    });
                    eprintln!(
                        "{} Thinking level set to '{}' (budget: 2000 tokens)",
                        "think:".green().bold(),
                        level
                    );
                }
                "medium" => {
                    *thinking = Some(ThinkingConfig {
                        enabled: true,
                        budget_tokens: Some(10000),
                    });
                    eprintln!(
                        "{} Thinking level set to '{}' (budget: 10000 tokens)",
                        "think:".green().bold(),
                        level
                    );
                }
                "high" => {
                    *thinking = Some(ThinkingConfig {
                        enabled: true,
                        budget_tokens: Some(50000),
                    });
                    eprintln!(
                        "{} Thinking level set to '{}' (budget: 50000 tokens)",
                        "think:".green().bold(),
                        level
                    );
                }
                _ => {
                    if let Ok(budget) = level.parse::<u32>() {
                        *thinking = Some(ThinkingConfig {
                            enabled: true,
                            budget_tokens: Some(budget),
                        });
                        eprintln!(
                            "{} Thinking enabled with custom budget: {} tokens",
                            "think:".green().bold(),
                            budget
                        );
                    } else {
                        eprintln!(
                            "{} Unknown level '{}'. Use: off, low, medium, high, or a number (token budget)",
                            "warning:".yellow().bold(),
                            level
                        );
                    }
                }
            }
            CommandResult::Continue
        }

        "/prune" => {
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

        "/clear" => {
            let system_msg = messages.iter().find(|m| m.is_system()).cloned();
            messages.clear();
            if let Some(sys) = system_msg {
                messages.push(sys);
            }
            eprintln!("{} Conversation history cleared", "clear:".green().bold());
            CommandResult::Continue
        }

        "/history" => {
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

        "/forget" => {
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

        "/memories" => {
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

        "/send" => {
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

        "/skill" => {
            let parts: Vec<&str> = arg.splitn(2, ' ').collect();
            let action = parts.first().copied().unwrap_or("list");
            let skill_arg = parts.get(1).copied();

            match action {
                "list" | "ls" | "" => {
                    if let Err(e) = crate::commands::run_skill_command("list", None) {
                        eprintln!("{} {}", "error:".red().bold(), e);
                    }
                }
                "info" => {
                    if let Some(name) = skill_arg {
                        if let Err(e) = crate::commands::run_skill_command("info", Some(name)) {
                            eprintln!("{} {}", "error:".red().bold(), e);
                        }
                    } else {
                        eprintln!("{} Usage: /skill info <name>", "usage:".yellow().bold());
                    }
                }
                _ => {
                    eprintln!(
                        "{} Unknown skill action '{}'. Use: list, info <name>",
                        "warning:".yellow().bold(),
                        action
                    );
                }
            }
            CommandResult::Continue
        }

        "/subagents" => {
            let parts: Vec<&str> = arg.splitn(2, ' ').collect();
            let action = parts.first().copied().unwrap_or("");

            match action {
                "" | "info" => {
                    println!("{}", "--- Sub-Agent Configuration ---".bold());
                    println!("  Enabled:        {}", config.subagent.enabled);
                    println!("  Max depth:      {}", config.subagent.max_depth);
                    println!("  Max concurrent: {}", config.subagent.max_concurrent);
                    println!("  Timeout:        {}s", config.subagent.timeout_secs);
                    println!();

                    println!("{}", "--- Built-in Agents ---".bold());
                    for name in &["Explore", "Plan", "Bash"] {
                        if let Some(def) = synaptic_deep::builtin_agent_def(name) {
                            println!("  {} — {}", name.cyan(), def.description);
                        }
                    }
                    println!();

                    if !config.subagent.agents.is_empty() {
                        println!("{}", "--- Config Agents ---".bold());
                        for def in &config.subagent.agents {
                            println!("  {} — {}", def.name.cyan(), def.description);
                            if let Some(ref tp) = def.tool_profile {
                                println!("    profile: {}", tp);
                            }
                        }
                        println!();
                    }

                    let cwd = std::env::current_dir().unwrap_or_default();
                    let discovered = crate::agent::discovery::discover_agents(&cwd);
                    if !discovered.is_empty() {
                        println!("{}", "--- Discovered Agents (.claude/agents/) ---".bold());
                        for def in &discovered {
                            println!("  {} — {}", def.name.cyan(), def.description);
                        }
                        println!();
                    }

                    if !config.subagent.tool_profiles.is_empty() {
                        println!("{}", "--- Tool Profiles ---".bold());
                        for (name, tools) in &config.subagent.tool_profiles {
                            println!("  {}: {}", name.cyan(), tools.join(", "));
                        }
                    }
                }
                "help" => {
                    println!("{}", "--- /subagents Commands ---".bold());
                    println!("  {} — Show config + defined agents", "/subagents".cyan());
                    println!("  {} — This help", "/subagents help".cyan());
                }
                _ => {
                    eprintln!(
                        "{} Unknown subagents action '{}'. Use: /subagents help",
                        "warning:".yellow().bold(),
                        action
                    );
                }
            }
            CommandResult::Continue
        }

        #[cfg(feature = "web")]
        "/pair" | "/devices" => {
            let sub = arg.split_whitespace().next().unwrap_or("");
            let rest: Vec<&str> = arg.split_whitespace().skip(1).collect();

            match sub {
                "" => {
                    // Generate QR + setup code
                    let mut bootstrap = crate::gateway::nodes::BootstrapStore::new();
                    let token = bootstrap.issue();
                    let port = config.serve.as_ref().and_then(|s| s.port).unwrap_or(3000);
                    let url = format!("ws://localhost:{}/ws", port);
                    let code = crate::gateway::nodes::bootstrap::encode_setup_code(&url, &token);
                    if let Some(qr) = crate::gateway::nodes::bootstrap::generate_qr_text(&code) {
                        eprintln!("{qr}");
                    }
                    eprintln!("Setup code: {code}");
                    eprintln!("Gateway:    {url}");
                    eprintln!("Expires in 10 minutes.");
                }
                "list" => {
                    let mut store = crate::gateway::nodes::PairingStore::new();
                    let pending = store.list_pending();
                    let paired = store.list_paired();
                    if pending.is_empty() && paired.is_empty() {
                        eprintln!("No devices.");
                    } else {
                        if !pending.is_empty() {
                            eprintln!("Pending:");
                            for r in &pending {
                                eprintln!(
                                    "  {} - {} ({})",
                                    r.request_id,
                                    &r.node_name,
                                    r.platform.as_deref().unwrap_or("-")
                                );
                            }
                        }
                        if !paired.is_empty() {
                            eprintln!("Paired:");
                            for n in &paired {
                                eprintln!(
                                    "  {} - {} ({})",
                                    n.node_id,
                                    &n.name,
                                    n.platform.as_deref().unwrap_or("-")
                                );
                            }
                        }
                    }
                }
                "approve" => {
                    let mut store = crate::gateway::nodes::PairingStore::new();
                    if let Some(id) = rest.first().copied() {
                        match store.approve(id) {
                            Some(n) => eprintln!("Approved: {}", n.node_id),
                            None => eprintln!("Not found: {id}"),
                        }
                    } else {
                        let pending = store.list_pending();
                        if pending.is_empty() {
                            eprintln!("No pending requests.");
                        } else {
                            let last_id = pending.last().unwrap().request_id.clone();
                            match store.approve(&last_id) {
                                Some(n) => eprintln!("Approved: {}", n.node_id),
                                None => eprintln!("Not found: {last_id}"),
                            }
                        }
                    }
                }
                "reject" => {
                    if let Some(id) = rest.first() {
                        let mut store = crate::gateway::nodes::PairingStore::new();
                        if store.reject(id) {
                            eprintln!("Rejected: {id}");
                        } else {
                            eprintln!("Not found: {id}");
                        }
                    } else {
                        eprintln!("Usage: /pair reject <request_id>");
                    }
                }
                "remove" => {
                    if let Some(id) = rest.first() {
                        let mut store = crate::gateway::nodes::PairingStore::new();
                        if store.remove_paired(id) {
                            eprintln!("Removed: {id}");
                        } else {
                            eprintln!("Not found: {id}");
                        }
                    } else {
                        eprintln!("Usage: /pair remove <device_id>");
                    }
                }
                _ => eprintln!("Usage: /pair [list|approve|reject|remove]"),
            }
            CommandResult::Continue
        }

        #[cfg(feature = "web")]
        "/dm" => {
            use synaptic::DmPolicyEnforcer;

            let parts: Vec<&str> = arg.split_whitespace().collect();
            let sub = parts.first().copied().unwrap_or("");
            let channel = parts.get(1).copied().unwrap_or("");

            if sub.is_empty() {
                eprintln!("Usage: /dm <action> <channel> [value]");
                return CommandResult::Continue;
            }

            if channel.is_empty() {
                eprintln!("Usage: /dm <action> <channel> [value]");
                return CommandResult::Continue;
            }

            let pairing_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".synapse")
                .join("pairing");
            let enforcer = crate::channels::dm::FileDmPolicyEnforcer::new(
                pairing_dir,
                synaptic::DmPolicy::Pairing,
                None,
            );

            match sub {
                "list" => {
                    let pending = enforcer.list_pending(channel).await;
                    if pending.is_empty() {
                        eprintln!("No pending DM requests for {channel}.");
                    } else {
                        for p in &pending {
                            eprintln!("  {} - {} ({})", p.code, p.sender_id, p.channel);
                        }
                    }
                }
                "approve" => {
                    let code = parts.get(2).copied().unwrap_or("");
                    if code.is_empty() {
                        eprintln!("Usage: /dm approve <channel> <code>");
                    } else {
                        match enforcer.approve_code(channel, code).await {
                            Ok(sender) => eprintln!("Approved: {sender}"),
                            Err(e) => eprintln!("Failed: {e}"),
                        }
                    }
                }
                "allowlist" => {
                    let list = enforcer.get_allowlist(channel);
                    if list.is_empty() {
                        eprintln!("Allowlist for {channel} is empty.");
                    } else {
                        for id in &list {
                            eprintln!("  {id}");
                        }
                    }
                }
                "remove" => {
                    let sender = parts.get(2).copied().unwrap_or("");
                    if sender.is_empty() {
                        eprintln!("Usage: /dm remove <channel> <sender_id>");
                    } else if enforcer.remove_from_allowlist(channel, sender) {
                        eprintln!("Removed: {sender}");
                    } else {
                        eprintln!("Not found: {sender}");
                    }
                }
                _ => eprintln!("Usage: /dm [list|approve|allowlist|remove] <channel>"),
            }
            CommandResult::Continue
        }

        _ => {
            let cmd_name = &cmd[1..]; // strip leading '/'

            // Check skill slash commands first
            let cwd = std::env::current_dir().unwrap_or_default();
            if let Some(skill_result) = resolve_skill_slash_command(cmd_name, arg, &cwd).await {
                eprintln!("{} Using skill /{}", "skill:".magenta().bold(), cmd_name);
                match skill_result {
                    SkillSlashResult::ToolDispatch {
                        tool_name,
                        arguments,
                        arg_mode,
                    } => {
                        eprintln!(
                            "  {} tool={} args={} mode={}",
                            "dispatch:".yellow().bold(),
                            tool_name,
                            arguments,
                            arg_mode
                        );
                        let dispatch_msg =
                            format!("Execute tool `{}` with arguments: {}", tool_name, arguments);
                        messages.push(Message::human(&dispatch_msg));
                    }
                    SkillSlashResult::Body(skill_body) => {
                        messages.push(Message::human(&skill_body));
                    }
                }

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
                            break;
                        }
                    }
                }
                println!();

                messages.push(Message::ai(&full_response));
                return CommandResult::Continue;
            }

            // Check custom commands from config
            if let Some(ref commands) = config.commands {
                if let Some(custom) = commands.iter().find(|c| c.name == cmd_name) {
                    let prompt = custom.prompt.replace("{{input}}", arg);
                    eprintln!(
                        "{} Running custom command /{}",
                        "command:".cyan().bold(),
                        cmd_name
                    );
                    messages.push(Message::human(&prompt));

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
                                break;
                            }
                        }
                    }
                    println!();

                    messages.push(Message::ai(&full_response));
                    return CommandResult::Continue;
                }
            }

            eprintln!(
                "{} unknown command '{}'. Type /help for available commands.",
                "warning:".yellow().bold(),
                cmd
            );
            CommandResult::Continue
        }
    }
}
