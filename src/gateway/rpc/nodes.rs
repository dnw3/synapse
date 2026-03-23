//! RPC handlers for node pairing, registry, and invocation.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;
use crate::gateway::nodes::pairing::PendingNodeRequest;
use crate::gateway::nodes::registry::NodeSession;
use crate::gateway::presence::now_ms;

// ---------------------------------------------------------------------------
// Pairing
// ---------------------------------------------------------------------------

pub async fn handle_pair_request(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed")
        .to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let platform = params
        .get("platform")
        .and_then(|v| v.as_str())
        .map(String::from);
    let req = PendingNodeRequest {
        request_id: request_id.clone(),
        node_name: name.clone(),
        public_key: params
            .get("public_key")
            .and_then(|v| v.as_str())
            .map(String::from),
        device_id: params
            .get("device_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        platform: platform.clone(),
        ip: params.get("ip").and_then(|v| v.as_str()).map(String::from),
        created_at: now_ms(),
    };
    ctx.state.network.pairing_store.write().await.request(req);

    // Notify operators of the new pairing request
    ctx.broadcaster
        .broadcast(
            "node.pair.pending",
            json!({
                "request_id": request_id,
                "node_name": name,
                "platform": platform,
            }),
        )
        .await;

    Ok(json!({"request_id": request_id}))
}

pub async fn handle_pair_approve(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let paired = ctx
        .state
        .network
        .pairing_store
        .write()
        .await
        .approve(request_id)
        .ok_or_else(|| RpcError::not_found("pending request not found"))?;
    Ok(serde_json::to_value(&paired).unwrap_or_default())
}

pub async fn handle_pair_reject(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let removed = ctx
        .state
        .network
        .pairing_store
        .write()
        .await
        .reject(request_id);
    if removed {
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("pending request not found"))
    }
}

pub async fn handle_pair_verify(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let valid = ctx.state.network.pairing_store.read().await.verify(node_id);
    Ok(json!({"valid": valid}))
}

pub async fn handle_pair_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let pending = ctx.state.network.pairing_store.write().await.list_pending();
    let paired = ctx.state.network.pairing_store.read().await.list_paired();
    Ok(json!({
        "pending": pending,
        "paired": paired,
    }))
}

// ---------------------------------------------------------------------------
// Node registry
// ---------------------------------------------------------------------------

pub async fn handle_node_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let nodes = ctx.state.network.node_registry.read().await.list();
    Ok(serde_json::to_value(&nodes).unwrap_or_default())
}

pub async fn handle_node_describe(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let registry = ctx.state.network.node_registry.read().await;
    let node = registry
        .get(node_id)
        .ok_or_else(|| RpcError::not_found("node not found"))?;
    Ok(serde_json::to_value(node).unwrap_or_default())
}

pub async fn handle_node_rename(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing name"))?;

    // Rename in both registry and pairing store
    ctx.state
        .network
        .node_registry
        .write()
        .await
        .rename(node_id, name);
    ctx.state
        .network
        .pairing_store
        .write()
        .await
        .rename(node_id, name);

    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------------------
// Node invocation
// ---------------------------------------------------------------------------

pub async fn handle_node_invoke(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?
        .to_string();
    let method = params
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing method"))?
        .to_string();
    let invoke_params = params.get("params").cloned().unwrap_or(Value::Null);
    let invoke_id = uuid::Uuid::new_v4().to_string();
    let timeout_ms = params
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(30_000);

    // Verify node is connected
    {
        let registry = ctx.state.network.node_registry.read().await;
        if registry.get(&node_id).is_none() {
            return Err(RpcError::not_found("node not connected"));
        }
    }

    let rx = ctx.state.network.node_registry.write().await.add_invoke(
        invoke_id.clone(),
        node_id.clone(),
        method.clone(),
        invoke_params.clone(),
    );

    // Notify the node about the pending invoke
    ctx.broadcaster
        .broadcast(
            "node.invoke.pending",
            json!({
                "invoke_id": invoke_id,
                "node_id": node_id,
                "method": method,
                "params": invoke_params,
            }),
        )
        .await;

    // Wait for result with timeout
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx).await {
        Ok(Ok(result)) => Ok(json!({"invoke_id": invoke_id, "result": result})),
        Ok(Err(_)) => Err(RpcError::internal("invoke channel closed")),
        Err(_) => Err(RpcError {
            code: 408,
            message: "invoke timed out".to_string(),
            details: None,
            retryable: true,
            retry_after_ms: Some(1000),
        }),
    }
}

pub async fn handle_invoke_result(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let invoke_id = params
        .get("invoke_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing invoke_id"))?;
    let result = params.get("result").cloned().unwrap_or(Value::Null);
    let resolved = ctx
        .state
        .network
        .node_registry
        .write()
        .await
        .resolve_invoke(invoke_id, result);
    if resolved {
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("invoke not found"))
    }
}

pub async fn handle_pending_pull(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let registry = ctx.state.network.node_registry.read().await;
    let pending: Vec<Value> = registry
        .pending_for_node(node_id)
        .iter()
        .map(|(id, method, params)| {
            json!({
                "invoke_id": id,
                "method": method,
                "params": params,
            })
        })
        .collect();
    Ok(json!({"pending": pending}))
}

pub async fn handle_pending_drain(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'node_id'"))?;
    // Placeholder: would clear all pending invocations for this node
    Ok(json!({ "ok": true, "drained": 0, "node_id": node_id }))
}

pub async fn handle_pending_enqueue(
    _ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'node_id'"))?;
    let method = params
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'method'"))?;
    let _task_params = params.get("params").cloned().unwrap_or(json!({}));
    let task_id = uuid::Uuid::new_v4().to_string();
    // Placeholder: would enqueue to the node's pending queue
    Ok(json!({ "ok": true, "task_id": task_id, "node_id": node_id, "method": method }))
}

pub async fn handle_pending_ack(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Acknowledge receipt of pending invokes (no-op for now, invokes are resolved via result)
    let _invoke_ids: Vec<String> = params
        .get("invoke_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(json!({"ok": true}))
}

pub async fn handle_node_event(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let event_type = params
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("node.event");
    ctx.broadcaster
        .broadcast(
            event_type,
            json!({
                "node_id": params.get("node_id"),
                "data": params.get("data").cloned().unwrap_or(Value::Null),
            }),
        )
        .await;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------------------
// Node registration (called by nodes on connect)
// ---------------------------------------------------------------------------

pub async fn handle_node_register(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?
        .to_string();

    // Verify the node is paired
    if !ctx
        .state
        .network
        .pairing_store
        .read()
        .await
        .verify(&node_id)
    {
        return Err(RpcError::forbidden("node not paired"));
    }

    let session = NodeSession {
        node_id: node_id.clone(),
        conn_id: ctx.conn_id.clone(),
        name: params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_string(),
        capabilities: params
            .get("capabilities")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        connected_at: now_ms(),
    };

    ctx.state
        .network
        .node_registry
        .write()
        .await
        .register(session);
    ctx.broadcaster
        .broadcast("node.connected", json!({"node_id": node_id}))
        .await;

    Ok(json!({"ok": true}))
}
