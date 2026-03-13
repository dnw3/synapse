//! RPC handlers for configuration management.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

fn config_file_path() -> String {
    if std::path::Path::new("synapse.toml").exists() {
        "synapse.toml".to_string()
    } else {
        "synapse.toml.example".to_string()
    }
}

async fn read_config_file() -> Result<(String, String), RpcError> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| RpcError::internal(format!("read config: {}", e)))?;
    Ok((path, content))
}

// ---------------------------------------------------------------------------
// config.get
// ---------------------------------------------------------------------------

pub async fn handle_get(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let (path, content) = read_config_file().await?;
    Ok(json!({ "content": content, "path": path }))
}

// ---------------------------------------------------------------------------
// config.set
// ---------------------------------------------------------------------------

pub async fn handle_set(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    // Validate TOML syntax
    toml::from_str::<toml::Value>(content)
        .map_err(|e| RpcError::invalid_request(format!("invalid TOML: {}", e)))?;

    let path = config_file_path();
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| RpcError::internal(format!("write failed: {}", e)))?;

    Ok(json!({ "success": true, "path": path }))
}

// ---------------------------------------------------------------------------
// config.schema
// ---------------------------------------------------------------------------

pub async fn handle_schema(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Return a simplified schema description
    Ok(json!({
        "sections": [
            { "key": "model", "label": "Model", "order": 10 },
            { "key": "agent", "label": "Agent", "order": 20 },
            { "key": "memory", "label": "Memory", "order": 30 },
            { "key": "session", "label": "Session", "order": 40 },
            { "key": "serve", "label": "Web Server", "order": 50 },
            { "key": "auth", "label": "Authentication", "order": 55 },
            { "key": "logging", "label": "Logging", "order": 100 },
        ],
        "sensitive_patterns": [
            "api_key", "token", "secret", "password",
            "app_secret", "signing_secret", "bot_token", "webhook_secret"
        ],
    }))
}

// ---------------------------------------------------------------------------
// config.validate
// ---------------------------------------------------------------------------

pub async fn handle_validate(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    // TOML syntax check
    let toml_val = match toml::from_str::<toml::Value>(content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(json!({
                "valid": false,
                "errors": [format!("TOML syntax: {}", e)],
            }));
        }
    };

    let mut errors = Vec::new();

    // Structural validation
    match toml::from_str::<crate::config::SynapseConfig>(content) {
        Ok(_) => {}
        Err(e) => {
            errors.push(format!("Config structure: {}", e));
        }
    }

    // Warn about sensitive fields in clear text
    if let Some(table) = toml_val.as_table() {
        check_sensitive(table, "", &mut errors);
    }

    Ok(json!({
        "valid": errors.is_empty(),
        "errors": errors,
    }))
}

fn check_sensitive(
    table: &toml::map::Map<String, toml::Value>,
    path: &str,
    warnings: &mut Vec<String>,
) {
    let sensitive_keys = [
        "api_key",
        "token",
        "secret",
        "password",
        "app_secret",
        "signing_secret",
        "bot_token",
    ];
    for (k, v) in table {
        let full_path = if path.is_empty() {
            k.clone()
        } else {
            format!("{}.{}", path, k)
        };
        if sensitive_keys.iter().any(|s| k.contains(s)) {
            if let toml::Value::String(val) = v {
                if !val.is_empty() && !val.starts_with("${") && !val.ends_with("_env") {
                    warnings.push(format!("Sensitive value in clear text: {}", full_path));
                }
            }
        }
        if let toml::Value::Table(sub) = v {
            check_sensitive(sub, &full_path, warnings);
        }
    }
}

// ---------------------------------------------------------------------------
// config.reload
// ---------------------------------------------------------------------------

pub async fn handle_reload(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Placeholder — config reload requires mutable config reference
    Ok(json!({ "ok": true }))
}
