use std::path::PathBuf;
use std::sync::Arc;

use colored::Colorize;
use futures::StreamExt;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{
    ChatModel, ChatRequest, HeuristicTokenCounter, MemoryStore, Message, TokenCounter, TokenUsage,
};
use synaptic::memory::ChatMessageHistory;
use synaptic::session::SessionManager;

use crate::config::SynapseConfig;
use crate::memory::LongTermMemory;
#[cfg(feature = "sandbox")]
use crate::sandbox::orchestrator::SandboxOrchestrator;
use crate::tools::pruning as tool_pruning;

use super::commands::{handle_command, CommandResult};
use super::session::{prune_sessions, save_session_title};

/// Interactive REPL mode.
#[allow(clippy::too_many_arguments)]
pub async fn repl(
    model: Arc<dyn ChatModel>,
    config: &SynapseConfig,
    session_mgr: &SessionManager,
    memory: &ChatMessageHistory,
    session_id: &str,
    messages: &mut Vec<Message>,
    tracker: Arc<CostTrackingCallback>,
    ltm: Arc<LongTermMemory>,
    #[cfg(feature = "sandbox")] sandbox_orchestrator: Option<Arc<SandboxOrchestrator>>,
) -> crate::error::Result<()> {
    let mut model = model;
    let mut current_model_name = config.model_config().model.clone();
    tracker.set_model(&current_model_name).await;

    let mut current_session_id = session_id.to_string();
    let mut verbose = false;
    let mut thinking = None;

    let ltm_count = ltm.count().await;
    if ltm_count > 0 {
        eprintln!(
            "{} {} long-term memories loaded",
            "memory:".blue().bold(),
            ltm_count
        );
    }

    // Auto-prune old sessions at startup
    if config.memory.session_prune_days > 0 {
        match prune_sessions(session_mgr, config.memory.session_prune_days).await {
            Ok(n) if n > 0 => {
                eprintln!(
                    "{} Pruned {} old session(s) (>{} days)",
                    "prune:".blue().bold(),
                    n,
                    config.memory.session_prune_days
                );
            }
            _ => {}
        }
    }

    // Session auto-reset: check if session should be reset (daily or idle timeout)
    let mut last_input_time = std::time::Instant::now();
    if config.session.daily_reset {
        if let Ok(Some(info)) = session_mgr.get_session(&current_session_id).await {
            let today = super::chrono_today();
            if !info.created_at.starts_with(&today) {
                if let Ok(new_sid) = session_mgr.create_session().await {
                    let system_msg = messages.iter().find(|m| m.is_system()).cloned();
                    messages.clear();
                    if let Some(sys) = system_msg {
                        messages.push(sys);
                    }
                    current_session_id = new_sid;
                    eprintln!(
                        "{} Daily session reset (new day)",
                        "session:".green().bold(),
                    );
                }
            }
        }
    }

    eprintln!("{} {}", "Session:".bold(), current_session_id.cyan());
    eprintln!("{} {}", "Model:".bold(), current_model_name.dimmed());
    let mut cmd_line = String::from(
        "Commands: /quit /new /session /sessions /compact /usage /status /context /model /think /clear /history /send /forget /memories /prune /help",
    );
    if let Some(ref commands) = config.commands {
        for custom in commands {
            cmd_line.push_str(&format!(" /{}", custom.name));
        }
    }
    eprintln!("{}", cmd_line.dimmed());
    eprintln!();

    // Spawn LTM file watcher for live reloading
    let _watcher_handle = LongTermMemory::watch(ltm.clone());

    let history_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse_history");

    let mut rl = rustyline::DefaultEditor::new().map_err(crate::error::SynapseError::internal)?;
    let _ = rl.load_history(&history_path);

    loop {
        let readline = rl.readline(&format!("{} ", "synapse>".green().bold()));
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(input);

                // Session idle reset check
                if config.session.idle_reset_minutes > 0 {
                    let elapsed = last_input_time.elapsed().as_secs() / 60;
                    if elapsed >= config.session.idle_reset_minutes {
                        if let Ok(new_sid) = session_mgr.create_session().await {
                            let system_msg = messages.iter().find(|m| m.is_system()).cloned();
                            messages.clear();
                            if let Some(sys) = system_msg {
                                messages.push(sys);
                            }
                            current_session_id = new_sid;
                            eprintln!(
                                "{} Session auto-reset (idle {}+ minutes)",
                                "session:".green().bold(),
                                config.session.idle_reset_minutes,
                            );
                        }
                    }
                }
                last_input_time = std::time::Instant::now();

                // Handle commands
                if input.starts_with('/') {
                    match handle_command(
                        input,
                        config,
                        session_mgr,
                        memory,
                        &mut current_session_id,
                        messages,
                        &mut model,
                        &tracker,
                        &mut current_model_name,
                        &mut verbose,
                        &mut thinking,
                        &ltm,
                        #[cfg(feature = "sandbox")]
                        &sandbox_orchestrator,
                    )
                    .await
                    {
                        CommandResult::Continue => continue,
                        CommandResult::Quit => break,
                    }
                }

                // Recall relevant long-term memories
                let recalled = ltm.recall(input, 3).await;
                if !recalled.is_empty() {
                    let context = recalled.join("\n- ");
                    messages.push(Message::system(format!(
                        "Relevant memories from past conversations:\n- {}",
                        context
                    )));
                }

                // Regular chat
                let human_msg = Message::human(input);
                memory
                    .append(&current_session_id, human_msg.clone())
                    .await
                    .ok();

                // Auto-title: save first user message as session title
                let human_count = messages.iter().filter(|m| m.is_human()).count();
                if human_count == 0 {
                    save_session_title(session_mgr.store(), &current_session_id, input).await;
                }

                messages.push(human_msg);

                // Tool result pruning
                if config.memory.max_tool_result_chars > 0 || config.memory.hard_clear_chars > 0 {
                    let opts = tool_pruning::PruningOptions::from_config(&config.memory);
                    tool_pruning::prune_tool_results_with_options(messages, &opts);
                }

                // Auto-compaction
                if config.memory.auto_compact {
                    let counter = HeuristicTokenCounter;
                    let estimated = counter.count_messages(messages);
                    if estimated > config.memory.effective_compact_threshold() {
                        let keep = config.memory.keep_recent;
                        let (system_msg, rest) = if !messages.is_empty() && messages[0].is_system()
                        {
                            (Some(messages[0].clone()), &messages[1..])
                        } else {
                            (None, messages.as_slice())
                        };

                        if rest.len() > keep {
                            let discard_count = rest.len() - keep;
                            let to_discard = &rest[..discard_count];

                            if config.memory.pre_compact_flush {
                                ltm.flush_before_compact(to_discard, model.as_ref()).await;
                            }

                            let to_keep = rest[discard_count..].to_vec();
                            messages.clear();
                            if let Some(sys) = system_msg {
                                messages.push(sys);
                            }
                            messages.extend(to_keep);
                        }

                        eprintln!(
                            "{} auto-compacted to {} messages",
                            "compact:".green().bold(),
                            messages.len()
                        );

                        if let Ok(Some(mut info)) =
                            session_mgr.get_session(&current_session_id).await
                        {
                            info.compaction_count += 1;
                            session_mgr.update_session(&info).await.ok();
                        }
                    }
                }

                let prompt_chars: usize = messages.iter().map(|m| m.content().len()).sum();
                let mut request = ChatRequest::new(messages.clone());
                if let Some(ref tc) = thinking {
                    request = request.with_thinking(tc.clone());
                }
                let mut stream = model.stream_chat(request);

                let mut full_response = String::new();
                let mut is_no_reply = false;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(c) => {
                            full_response.push_str(&c.content);
                            if full_response.len() <= 10 && full_response.starts_with("NO_REPLY") {
                                is_no_reply = true;
                            }
                            if !is_no_reply {
                                print!("{}", c.content);
                            }
                        }
                        Err(e) => {
                            eprintln!("\n{} {}", "error:".red().bold(), e);
                            break;
                        }
                    }
                }
                if !is_no_reply {
                    println!();
                }

                let input_est = (prompt_chars / 4) as u32;
                let output_est = (full_response.len() / 4) as u32;
                let est_usage = TokenUsage {
                    input_tokens: input_est,
                    output_tokens: output_est,
                    total_tokens: input_est + output_est,
                    input_details: None,
                    output_details: None,
                };
                tracker.record_usage(&est_usage).await;

                if verbose {
                    eprintln!(
                        "{} ~{} input + ~{} output tokens, {} messages in context",
                        "verbose:".dimmed(),
                        input_est,
                        output_est,
                        messages.len() + 1,
                    );
                }

                let ai_msg = Message::ai(&full_response);
                memory
                    .append(&current_session_id, ai_msg.clone())
                    .await
                    .ok();
                messages.push(ai_msg);

                if LongTermMemory::is_important(&full_response) {
                    ltm.remember(&full_response).await.ok();
                }

                let turn_tokens = (prompt_chars + full_response.len()) / 4;
                if turn_tokens > 0 {
                    if let Ok(Some(mut info)) = session_mgr.get_session(&current_session_id).await {
                        info.total_tokens += turn_tokens as u64;
                        session_mgr.update_session(&info).await.ok();
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("{}", "Interrupted (Ctrl-C). Type /quit to exit.".dimmed());
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                eprintln!("{}", "Goodbye.".dimmed());
                break;
            }
            Err(e) => {
                eprintln!("{} readline: {}", "error:".red().bold(), e);
                break;
            }
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}
