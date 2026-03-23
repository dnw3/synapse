//! Stdio transport for ACP — reads JSON-RPC requests from stdin, writes responses to stdout.
//!
//! Each message is framed with `Content-Length: <n>\r\n\r\n` header (LSP-style).

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use colored::Colorize;
use synaptic::core::{ChatModel, MemoryStore, Message};
use synaptic::deep::acp::handler::AcpHandler;
use synaptic::deep::acp::types::*;
use synaptic::graph::MessageState;
use synaptic::session::SessionManager;

use crate::agent;
use crate::config::SynapseConfig;

/// Run the ACP stdio transport (blocking — reads from stdin).
pub async fn run_stdio(
    config: &SynapseConfig,
    model: Arc<dyn ChatModel>,
) -> crate::error::Result<()> {
    let handler = AcpHandler::new("synapse", env!("CARGO_PKG_VERSION"));
    let session_mgr = crate::build_session_manager(config);

    eprintln!("{} ACP stdio transport ready", "acp:".cyan().bold());

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();

    loop {
        // Read Content-Length header
        let mut header_line = String::new();
        if reader.read_line(&mut header_line)? == 0 {
            break; // EOF
        }
        let header_line = header_line.trim();
        if header_line.is_empty() {
            continue;
        }

        let content_length: usize = if let Some(val) = header_line.strip_prefix("Content-Length:") {
            val.trim().parse().unwrap_or(0)
        } else {
            // Try reading as raw JSON (lenient mode)
            let resp =
                handle_raw_request(header_line, &handler, config, &model, &session_mgr).await;
            write_response(&stdout, &resp)?;
            continue;
        };

        if content_length == 0 {
            continue;
        }

        // Read blank line separator
        let mut blank = String::new();
        reader.read_line(&mut blank)?;

        // Read content body
        let mut body = vec![0u8; content_length];
        io::Read::read_exact(&mut reader, &mut body)?;
        let body = String::from_utf8_lossy(&body);

        let resp = handle_raw_request(&body, &handler, config, &model, &session_mgr).await;
        write_response(&stdout, &resp)?;
    }

    Ok(())
}

async fn handle_raw_request(
    raw: &str,
    handler: &AcpHandler,
    config: &SynapseConfig,
    model: &Arc<dyn ChatModel>,
    session_mgr: &SessionManager,
) -> JsonRpcResponse {
    let req = match AcpHandler::parse_request(raw) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Try framework-level routing first (capabilities, unknown methods)
    if let Some(resp) = handler.route(&req) {
        return resp;
    }

    // Handle agent methods
    match req.method.as_str() {
        "agent/run" => {
            handle_agent_run(
                req.id.clone(),
                req.params.clone(),
                config,
                model,
                session_mgr,
            )
            .await
        }
        "agent/status" => JsonRpcResponse::success(req.id, serde_json::json!({"status": "idle"})),
        "agent/cancel" => JsonRpcResponse::success(req.id, serde_json::json!({"cancelled": true})),
        _ => JsonRpcResponse::error(req.id, METHOD_NOT_FOUND, "method not found"),
    }
}

async fn handle_agent_run(
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
    config: &SynapseConfig,
    model: &Arc<dyn ChatModel>,
    session_mgr: &SessionManager,
) -> JsonRpcResponse {
    let run_params: AgentRunParams = match params {
        Some(p) => match serde_json::from_value(p) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(id, INVALID_PARAMS, format!("invalid params: {}", e))
            }
        },
        None => return JsonRpcResponse::error(id, INVALID_PARAMS, "missing params"),
    };

    // Create or reuse session
    let sid = match &run_params.session_id {
        Some(s) => s.clone(),
        None => match session_mgr.create_session().await {
            Ok(s) => s,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    INTERNAL_ERROR,
                    format!("failed to create session: {}", e),
                )
            }
        },
    };

    // Build and run agent
    let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let checkpointer = Arc::new(session_mgr.checkpointer());

    let agent = match agent::build_deep_agent(
        model.clone(),
        config,
        &cwd,
        checkpointer,
        vec![],
        None,
        agent::SessionKind::Full,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            return JsonRpcResponse::error(
                id,
                INTERNAL_ERROR,
                format!("failed to build agent: {}", e),
            )
        }
    };

    let memory = session_mgr.memory();
    let mut messages = memory.load(&sid).await.unwrap_or_default();
    if messages.is_empty() {
        if let Some(ref prompt) = config.agent_config().system_prompt {
            messages.push(Message::system(prompt));
        }
    }
    messages.push(Message::human(&run_params.task));

    let initial_state = MessageState::with_messages(messages);
    match agent.invoke(initial_state).await {
        Ok(result) => {
            let final_state = result.into_state();
            let response_text = final_state
                .messages
                .iter()
                .rev()
                .find(|m| m.is_ai() && !m.content().is_empty())
                .map(|m| m.content().to_string())
                .unwrap_or_default();

            // Save messages
            for msg in &final_state.messages {
                memory.append(&sid, msg.clone()).await.ok();
            }

            JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "session_id": sid,
                    "content": response_text,
                }),
            )
        }
        Err(e) => JsonRpcResponse::error(id, INTERNAL_ERROR, format!("agent error: {}", e)),
    }
}

fn write_response(stdout: &io::Stdout, resp: &JsonRpcResponse) -> io::Result<()> {
    let body = serde_json::to_string(resp).unwrap_or_default();
    let mut out = stdout.lock();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    out.flush()
}
