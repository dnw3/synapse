use std::path::{Path, PathBuf};
use std::sync::Arc;

use colored::Colorize;
use futures::StreamExt;
use synaptic::core::{ChatModel, MemoryStore, Message};
use synaptic::graph::{MessageState, StreamMode};
use synaptic::memory::ChatMessageHistory;

use crate::agent::{self, InteractiveApprovalCallback};
use crate::config::SynapseConfig;
use crate::display;
use crate::memory::LongTermMemory;
use crate::router::AgentRouter;

/// Run a Deep Agent task with streaming output.
pub async fn run_task(
    config: &SynapseConfig,
    model: Arc<dyn ChatModel>,
    message: &str,
    session_id: Option<&str>,
    cwd: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_mgr = crate::build_session_manager(config);
    let memory = session_mgr.memory();

    // Multi-agent routing: select model/prompt based on message pattern
    let (routed_model, system_prompt_override) =
        if config.agent_routes.as_ref().is_some_and(|r| !r.is_empty()) {
            let router = AgentRouter::new(config, model.clone())?;
            let (agent_name, routed, sys_prompt) = router.route(message, None);
            tracing::info!(agent = %agent_name, "routed to agent");
            (routed.clone(), sys_prompt.map(|s| s.to_string()))
        } else {
            (model, None)
        };

    // Resolve or create session
    let sid = if let Some(id) = session_id {
        session_mgr
            .get_session(id)
            .await
            .map_err(|e| format!("failed to look up session '{}': {}", id, e))?
            .ok_or_else(|| format!("session '{}' not found", id))?;
        id.to_string()
    } else {
        session_mgr
            .create_session()
            .await
            .map_err(|e| format!("failed to create session: {}", e))?
    };

    let work_dir = cwd.unwrap_or_else(|| Path::new("."));
    let work_dir = std::fs::canonicalize(work_dir)?;

    eprintln!("{} {}", "Session:".bold(), sid.cyan());
    eprintln!("{} {}", "Task:".bold(), message);
    eprintln!(
        "{} {}",
        "Working dir:".bold(),
        work_dir.display().to_string().dimmed()
    );
    eprintln!();

    // Load MCP tools
    let mcp_tools = agent::load_mcp_tools(config).await;

    // Long-term memory (load before agent so we can share it)
    let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
    let ltm = Arc::new(LongTermMemory::new(
        sessions_dir.join("long_term_memory"),
        config.memory.clone(),
    ));
    ltm.load().await.ok();

    // Build deep agent with filesystem backend + MCP tools + interactive approval + LTM
    let checkpointer = Arc::new(session_mgr.checkpointer());
    let approval = Arc::new(InteractiveApprovalCallback::new());
    let agent = agent::build_deep_agent_with_callback(
        routed_model,
        config,
        &work_dir,
        checkpointer,
        mcp_tools,
        system_prompt_override.as_deref(),
        Some(approval),
        Some(ltm.clone()),
        None,
        None,
        None,
    )
    .await?;

    // Build initial state
    let mut initial_messages = Vec::new();
    if session_id.is_some() {
        // Resume: load existing messages from store
        if let Ok(msgs) = memory.load(&sid).await {
            initial_messages = msgs;
        }
    }

    // Recall relevant LTM context
    if !message.is_empty() && config.memory.ltm_enabled {
        let recalled = ltm.recall(message, config.memory.ltm_recall_limit).await;
        if !recalled.is_empty() {
            let context = recalled.join("\n- ");
            initial_messages.push(Message::system(format!(
                "Relevant memories from past sessions:\n- {}",
                context
            )));
        }
    }

    // Only add the new human message if not resuming (or if resuming with a new message)
    if !message.is_empty() {
        let human_msg = Message::human(message);
        memory.append(&sid, human_msg.clone()).await.ok();
        initial_messages.push(human_msg);
    }

    let initial_state = MessageState::with_messages(initial_messages);

    // Stream execution with Ctrl-C handling
    let stream = agent.stream(initial_state, StreamMode::Values);
    tokio::pin!(stream);

    let mut displayed_count = 0usize;
    let cancel = tokio::signal::ctrl_c();
    tokio::pin!(cancel);

    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(Ok(graph_event)) => {
                        displayed_count = display::render_new_messages(
                            &graph_event.state.messages,
                            displayed_count,
                        );
                        // Save new messages to store
                        save_new_messages(&memory, &sid, &graph_event.state.messages, displayed_count).await;
                    }
                    Some(Err(e)) => {
                        eprintln!("\n{} {}", "error:".red().bold(), e);
                        break;
                    }
                    None => {
                        // Stream completed
                        eprintln!();
                        eprintln!("{}", "Done.".green().bold());
                        crate::notify::send_notification(
                            "Synapse Task Complete",
                            &format!("Task finished: {}", truncate_msg(message, 60)),
                        );
                        break;
                    }
                }
            }
            _ = &mut cancel => {
                eprintln!();
                eprintln!("{}", "Interrupted.".yellow().bold());
                eprintln!(
                    "Resume with: {} task --session {}",
                    "synapse".green(),
                    sid.cyan()
                );
                return Ok(());
            }
        }
    }

    Ok(())
}

fn truncate_msg(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

/// Save messages to the store that haven't been saved yet.
async fn save_new_messages(
    memory: &ChatMessageHistory,
    session_id: &str,
    messages: &[Message],
    up_to: usize,
) {
    // We track which messages have been saved by checking the stored message count
    let saved = memory.load(session_id).await.map(|m| m.len()).unwrap_or(0);
    for msg in messages
        .iter()
        .skip(saved)
        .take(up_to.saturating_sub(saved))
    {
        memory.append(session_id, msg.clone()).await.ok();
    }
}
