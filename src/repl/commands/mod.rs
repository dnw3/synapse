mod agents;
#[cfg(feature = "web")]
mod devices;
mod fallback;
mod memory;
mod model;
mod session;
mod system;

use std::sync::Arc;

use colored::Colorize;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{ChatModel, Message, ThinkingConfig};
use synaptic::memory::ChatMessageHistory;
use synaptic::session::SessionManager;

use crate::config::SynapseConfig;
use crate::memory::LongTermMemory;

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

        "/help" => system::cmd_help(config),
        "/new" => session::cmd_new(session_mgr, messages, session_id).await,
        "/session" => session::cmd_session(memory, session_mgr, session_id).await,
        "/sessions" => session::cmd_sessions(session_mgr).await,
        "/compact" => session::cmd_compact(arg, messages),
        "/clear" => session::cmd_clear(messages),
        "/history" => session::cmd_history(arg, session_mgr).await,
        "/send" => session::cmd_send(arg, session_mgr).await,
        "/prune" => session::cmd_prune(arg, session_mgr).await,

        "/usage" => system::cmd_usage(tracker).await,
        "/status" => {
            system::cmd_status(config, memory, session_id, messages, current_model_name).await
        }
        "/context" => system::cmd_context(messages, ltm).await,

        "/model" => model::cmd_model(arg, config, model, tracker, current_model_name).await,
        "/verbose" => model::cmd_verbose(verbose),
        "/think" => model::cmd_think(arg, thinking, messages),

        "/forget" => memory::cmd_forget(arg, ltm).await,
        "/memories" => memory::cmd_memories(arg, ltm).await,

        "/skill" => agents::cmd_skill(arg),
        "/subagents" => agents::cmd_subagents(arg, config),

        #[cfg(feature = "web")]
        "/pair" | "/devices" => devices::cmd_pair(arg, config),
        #[cfg(feature = "web")]
        "/dm" => devices::cmd_dm(arg).await,

        _ => fallback::handle_fallback(cmd, arg, config, messages, model).await,
    }
}
