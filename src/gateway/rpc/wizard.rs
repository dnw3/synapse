//! RPC handlers for the setup wizard (wizard.start, wizard.next, wizard.cancel, wizard.status).

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// Wizard types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub field_type: String, // "text", "select", "toggle", "password"
    pub options: Option<Vec<String>>,
    pub default_value: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardSession {
    pub id: String,
    pub mode: String,
    pub status: String, // "running", "done", "cancelled"
    pub current_step: usize,
    pub answers: HashMap<String, Value>,
}

// ---------------------------------------------------------------------------
// Setup wizard steps
// ---------------------------------------------------------------------------

fn setup_steps() -> Vec<WizardStep> {
    vec![
        WizardStep {
            id: "provider".to_string(),
            title: "Model Provider".to_string(),
            description: "Select the AI model provider you want to use.".to_string(),
            field_type: "select".to_string(),
            options: Some(vec![
                "openai".to_string(),
                "anthropic".to_string(),
                "ollama".to_string(),
                "gemini".to_string(),
                "deepseek".to_string(),
                "groq".to_string(),
                "mistral".to_string(),
            ]),
            default_value: Some("anthropic".to_string()),
            required: true,
        },
        WizardStep {
            id: "api_key".to_string(),
            title: "API Key".to_string(),
            description: "Enter your API key for the selected provider.".to_string(),
            field_type: "password".to_string(),
            options: None,
            default_value: None,
            required: true,
        },
        WizardStep {
            id: "model_name".to_string(),
            title: "Model Name".to_string(),
            description: "Enter the model identifier to use.".to_string(),
            field_type: "text".to_string(),
            options: None,
            default_value: Some("claude-3-5-sonnet-20241022".to_string()),
            required: true,
        },
        WizardStep {
            id: "agent_name".to_string(),
            title: "Agent Name".to_string(),
            description: "Give your agent a display name.".to_string(),
            field_type: "text".to_string(),
            options: None,
            default_value: Some("synapse".to_string()),
            required: false,
        },
    ]
}

/// Return a default model name for the given provider.
fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => "gpt-4o",
        "anthropic" => "claude-3-5-sonnet-20241022",
        "ollama" => "llama3.2",
        "gemini" => "gemini-2.0-flash",
        "deepseek" => "deepseek-chat",
        "groq" => "llama-3.3-70b-versatile",
        "mistral" => "mistral-large-latest",
        _ => "gpt-4o",
    }
}

// ---------------------------------------------------------------------------
// wizard.start
// ---------------------------------------------------------------------------

pub async fn handle_start(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let mode = params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("setup")
        .to_string();

    let session_id = uuid::Uuid::new_v4().to_string();
    let steps = setup_steps();
    let first_step = steps[0].clone();

    let session = WizardSession {
        id: session_id.clone(),
        mode,
        status: "running".to_string(),
        current_step: 0,
        answers: HashMap::new(),
    };

    {
        let mut sessions = ctx.state.session.wizard_sessions.write().await;
        sessions.insert(session_id.clone(), session);
    }

    Ok(json!({
        "session_id": session_id,
        "step": first_step,
        "step_index": 0,
        "total_steps": steps.len(),
    }))
}

// ---------------------------------------------------------------------------
// wizard.next
// ---------------------------------------------------------------------------

pub async fn handle_next(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id'"))?
        .to_string();

    let answer = params
        .get("answer")
        .ok_or_else(|| RpcError::invalid_request("missing 'answer'"))?;

    let step_id = answer
        .get("step_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'answer.step_id'"))?
        .to_string();

    let value = answer.get("value").cloned().unwrap_or(Value::Null);

    let mut sessions = ctx.state.session.wizard_sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| RpcError::invalid_request("wizard session not found"))?;

    if session.status != "running" {
        return Err(RpcError::invalid_request(format!(
            "session is {}",
            session.status
        )));
    }

    // Store the answer
    session.answers.insert(step_id.clone(), value.clone());

    // If answering provider, update the model_name step default
    let steps = setup_steps();
    let mut updated_steps = steps.clone();
    if step_id == "provider" {
        if let Some(provider_str) = value.as_str() {
            let default_model = default_model_for_provider(provider_str);
            if let Some(mn_step) = updated_steps.iter_mut().find(|s| s.id == "model_name") {
                mn_step.default_value = Some(default_model.to_string());
            }
        }
    }

    // Advance step
    session.current_step += 1;

    if session.current_step >= updated_steps.len() {
        // Wizard complete — build config patch
        session.status = "done".to_string();

        let provider = session
            .answers
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic")
            .to_string();
        let api_key = session
            .answers
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let model_name = session
            .answers
            .get("model_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| default_model_for_provider(&provider).to_string());
        let agent_name = session
            .answers
            .get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("synapse")
            .to_string();

        let config_patch = json!({
            "model.name": model_name,
            "model.provider": provider,
            "model.api_key": api_key,
            "agent.name": agent_name,
        });

        return Ok(json!({
            "done": true,
            "config_patch": config_patch,
            "session_id": session_id,
        }));
    }

    let next_step = updated_steps[session.current_step].clone();
    let step_index = session.current_step;

    Ok(json!({
        "done": false,
        "step": next_step,
        "step_index": step_index,
        "total_steps": updated_steps.len(),
        "session_id": session_id,
    }))
}

// ---------------------------------------------------------------------------
// wizard.cancel
// ---------------------------------------------------------------------------

pub async fn handle_cancel(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id'"))?;

    let removed = {
        let mut sessions = ctx.state.session.wizard_sessions.write().await;
        sessions.remove(session_id).is_some()
    };

    Ok(json!({ "ok": removed, "session_id": session_id }))
}

// ---------------------------------------------------------------------------
// wizard.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id'"))?;

    let sessions = ctx.state.session.wizard_sessions.read().await;
    let session = sessions
        .get(session_id)
        .ok_or_else(|| RpcError::invalid_request("wizard session not found"))?;

    let steps = setup_steps();
    let current_step = if session.current_step < steps.len() {
        Some(serde_json::to_value(&steps[session.current_step]).unwrap_or(Value::Null))
    } else {
        None
    };

    Ok(json!({
        "session_id": session.id,
        "mode": session.mode,
        "status": session.status,
        "current_step": session.current_step,
        "total_steps": steps.len(),
        "step": current_step,
        "answers_count": session.answers.len(),
    }))
}
