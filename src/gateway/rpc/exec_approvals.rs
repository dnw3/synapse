//! RPC handlers for exec approval management.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;
use crate::gateway::exec_approvals::manager::{ApprovalDecision, ApprovalRequestPayload};
use crate::gateway::presence::now_ms;

pub async fn handle_approval_request(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing command"))?
        .to_string();
    let args: Vec<String> = params
        .get("args")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let cwd = params.get("cwd").and_then(|v| v.as_str()).map(String::from);
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    let request_id = uuid::Uuid::new_v4().to_string();

    // Check policy first
    let config = ctx.state.channel.exec_approvals_config.read().await;
    let policy_result =
        crate::gateway::exec_approvals::policy::evaluate(&config, &command, node_id.as_deref());
    drop(config);

    match policy_result {
        crate::gateway::exec_approvals::policy::PolicyResult::Allow => {
            return Ok(json!({"decision": "allow", "request_id": request_id}));
        }
        crate::gateway::exec_approvals::policy::PolicyResult::Deny => {
            return Ok(json!({"decision": "deny", "request_id": request_id}));
        }
        crate::gateway::exec_approvals::policy::PolicyResult::Ask => {
            // Check session allows
            let mgr = ctx.state.channel.exec_approval_manager.read().await;
            if mgr.is_session_allowed(&command) {
                return Ok(
                    json!({"decision": "allow", "request_id": request_id, "source": "session"}),
                );
            }
            drop(mgr);
        }
    }

    let payload = ApprovalRequestPayload {
        request_id: request_id.clone(),
        command: command.clone(),
        args,
        cwd,
        node_id,
        created_at: now_ms(),
    };

    let _rx = ctx
        .state
        .channel.exec_approval_manager
        .write()
        .await
        .create(payload);

    // Notify operators
    ctx.broadcaster
        .broadcast(
            "exec.approval.pending",
            json!({
                "request_id": request_id,
                "command": command,
            }),
        )
        .await;

    Ok(json!({"decision": "pending", "request_id": request_id}))
}

pub async fn handle_approval_resolve(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let decision_str = params
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing decision"))?;

    let decision = match decision_str {
        "allow" => ApprovalDecision::Allow,
        "allow_once" => ApprovalDecision::AllowOnce,
        "allow_session" => ApprovalDecision::AllowSession,
        "deny" => ApprovalDecision::Deny,
        _ => return Err(RpcError::invalid_request("invalid decision value")),
    };

    let resolved = ctx
        .state
        .channel.exec_approval_manager
        .write()
        .await
        .resolve(request_id, decision);
    if resolved {
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("approval request not found"))
    }
}

pub async fn handle_wait_decision(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?
        .to_string();
    let timeout_ms = params
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(60_000);

    // Get the snapshot to check if it exists
    let snapshot = ctx.state.channel.exec_approval_manager.write().await.get_snapshot();
    if !snapshot.iter().any(|p| p.request_id == request_id) {
        return Err(RpcError::not_found(
            "approval request not found or already resolved",
        ));
    }

    // Poll for resolution (simplified — real impl would use the oneshot channel)
    let start = now_ms();
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let snapshot = ctx.state.channel.exec_approval_manager.write().await.get_snapshot();
        if !snapshot.iter().any(|p| p.request_id == request_id) {
            // Request was resolved (no longer pending)
            return Ok(json!({"resolved": true, "request_id": request_id}));
        }
        if now_ms().saturating_sub(start) > timeout_ms {
            return Ok(json!({"resolved": false, "request_id": request_id, "reason": "timeout"}));
        }
    }
}

pub async fn handle_approvals_get(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let config = ctx.state.channel.exec_approvals_config.read().await;
    let pending = ctx.state.channel.exec_approval_manager.write().await.get_snapshot();
    Ok(json!({
        "mode": config.mode,
        "ask": config.ask,
        "allowlist": config.allowlist,
        "config_hash": config.config_hash,
        "pending": pending,
    }))
}

pub async fn handle_approvals_set(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let expected_hash = params
        .get("config_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let new_mode = params
        .get("mode")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let new_ask = params
        .get("ask")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let new_allowlist = params
        .get("allowlist")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mut config = ctx.state.channel.exec_approvals_config.write().await;
    config
        .cas_update(expected_hash, new_mode, new_ask, new_allowlist)
        .map_err(RpcError::invalid_request)?;

    Ok(json!({
        "ok": true,
        "config_hash": config.config_hash,
    }))
}

pub async fn handle_node_approvals_get(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let config = ctx.state.channel.exec_approvals_config.read().await;
    let node_config = config.node_overrides.get(node_id);
    Ok(serde_json::to_value(node_config).unwrap_or(Value::Null))
}

pub async fn handle_node_approvals_set(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?
        .to_string();

    let node_config: crate::gateway::exec_approvals::config::NodeExecConfig =
        serde_json::from_value(params.get("config").cloned().unwrap_or(Value::Null))
            .map_err(|e| RpcError::invalid_request(format!("invalid config: {e}")))?;

    let mut config = ctx.state.channel.exec_approvals_config.write().await;
    config.node_overrides.insert(node_id, node_config);
    config.save();

    Ok(json!({"ok": true}))
}
