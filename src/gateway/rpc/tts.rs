//! TTS/Voice RPC stubs.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_status(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({
        "enabled": false,
        "provider": null,
        "voice": null,
    }))
}

pub async fn handle_providers(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({
        "providers": [],
    }))
}

pub async fn handle_enable(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({"ok": false, "message": "TTS not yet implemented"}))
}

pub async fn handle_disable(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({"ok": true}))
}

pub async fn handle_convert(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Err(RpcError {
        code: 501,
        message: "TTS conversion not yet implemented".to_string(),
        details: None,
        retryable: false,
        retry_after_ms: None,
    })
}

pub async fn handle_set_provider(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({"ok": false, "message": "TTS not yet implemented"}))
}
