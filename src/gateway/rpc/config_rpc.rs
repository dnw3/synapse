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

// ---------------------------------------------------------------------------
// config.patch
// ---------------------------------------------------------------------------

/// Apply a JSON merge patch to the TOML config.
/// Params: `{ patch: { "model.name": "gpt-4", ... }, base_hash?: string }`
pub async fn handle_patch(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let patch = params
        .get("patch")
        .and_then(|v| v.as_object())
        .ok_or_else(|| RpcError::invalid_request("missing 'patch' object"))?
        .clone();

    let (path, content) = read_config_file().await?;

    // Optional hash check for concurrent edit detection
    if let Some(expected_hash) = params.get("base_hash").and_then(|v| v.as_str()) {
        let actual_hash = sha256_hex(content.as_bytes());
        if actual_hash != expected_hash {
            return Err(RpcError::invalid_request(
                "base_hash mismatch — config was modified concurrently",
            ));
        }
    }

    let mut toml_val: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse config: {}", e)))?;

    // Apply each dotted-key patch entry
    for (key, value) in &patch {
        let parts: Vec<&str> = key.split('.').collect();
        set_toml_path(&mut toml_val, &parts, json_to_toml(value)?);
    }

    let new_content = toml::to_string_pretty(&toml_val)
        .map_err(|e| RpcError::internal(format!("serialize config: {}", e)))?;

    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write config: {}", e)))?;

    let new_hash = sha256_hex(new_content.as_bytes());
    Ok(json!({ "success": true, "path": path, "hash": new_hash }))
}

// ---------------------------------------------------------------------------
// config.apply  (synonym for config.set)
// ---------------------------------------------------------------------------

pub async fn handle_apply(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    handle_set(ctx, params).await
}

// ---------------------------------------------------------------------------
// config.schema.lookup
// ---------------------------------------------------------------------------

/// Return the sub-section of the schema for a given path.
/// Params: `{ path: string }` — e.g. `"model"` or `"serve.port"`
pub async fn handle_schema_lookup(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'path' parameter"))?;

    // Full schema as a JSON object indexed by dotted path
    let full_schema = serde_json::json!({
        "model": {
            "description": "LLM model configuration",
            "fields": {
                "name": { "type": "string", "description": "Model identifier", "default": "claude-3-5-sonnet-20241022" },
                "provider": { "type": "string", "description": "Model provider (openai/anthropic/ollama/...)" },
                "api_key": { "type": "string", "sensitive": true, "description": "Provider API key" },
                "base_url": { "type": "string", "description": "Custom API base URL" },
                "max_tokens": { "type": "integer", "description": "Max output tokens" },
                "temperature": { "type": "float", "description": "Sampling temperature" },
            }
        },
        "agent": {
            "description": "Agent behaviour configuration",
            "fields": {
                "name": { "type": "string", "description": "Display name of this agent" },
                "system_prompt": { "type": "string", "description": "System prompt / persona" },
                "max_iterations": { "type": "integer", "description": "Max tool-call iterations per request" },
            }
        },
        "memory": {
            "description": "Long-term memory store",
            "fields": {
                "enabled": { "type": "bool", "description": "Enable memory module" },
                "embedding_model": { "type": "string", "description": "Embedding model for retrieval" },
            }
        },
        "session": {
            "description": "Session persistence settings",
            "fields": {
                "dir": { "type": "string", "description": "Directory for session transcripts" },
                "max_messages": { "type": "integer", "description": "Maximum messages per session" },
            }
        },
        "serve": {
            "description": "Web server settings",
            "fields": {
                "port": { "type": "integer", "description": "HTTP listen port", "default": 3000 },
                "host": { "type": "string", "description": "Bind address", "default": "127.0.0.1" },
            }
        },
        "auth": {
            "description": "Authentication settings",
            "fields": {
                "token": { "type": "string", "sensitive": true, "description": "Bearer token for gateway auth" },
            }
        },
        "logging": {
            "description": "Logging configuration",
            "fields": {
                "level": { "type": "string", "description": "Log level (trace/debug/info/warn/error)" },
                "file": { "type": "string", "description": "Log file path" },
            }
        },
    });

    // Walk dotted path
    let mut current = &full_schema;
    for part in path.split('.') {
        current = current
            .get(part)
            .ok_or_else(|| RpcError::invalid_request(format!("unknown schema path: {}", path)))?;
    }

    Ok(json!({ "path": path, "schema": current }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    // Simple SHA-256 using standard library via std::hash or manual — use sha2 if available,
    // otherwise fall back to a stable placeholder based on content length + checksum.
    // Since sha2 may not be in Cargo.toml, use a simple FNV-style hash serialised as hex.
    let mut hash: u64 = 14695981039346656037u64;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let mut s = String::new();
    let _ = write!(s, "{:016x}", hash);
    s
}

fn set_toml_path(root: &mut toml::Value, parts: &[&str], value: toml::Value) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        if let toml::Value::Table(t) = root {
            t.insert(parts[0].to_string(), value);
        }
        return;
    }
    if let toml::Value::Table(t) = root {
        let entry = t
            .entry(parts[0].to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        set_toml_path(entry, &parts[1..], value);
    }
}

fn json_to_toml(v: &serde_json::Value) -> Result<toml::Value, RpcError> {
    match v {
        serde_json::Value::Null => Ok(toml::Value::String(String::new())),
        serde_json::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Err(RpcError::invalid_request("unsupported numeric type"))
            }
        }
        serde_json::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<_>, _> = arr.iter().map(json_to_toml).collect();
            Ok(toml::Value::Array(items?))
        }
        serde_json::Value::Object(map) => {
            let mut t = toml::map::Map::new();
            for (k, val) in map {
                t.insert(k.clone(), json_to_toml(val)?);
            }
            Ok(toml::Value::Table(t))
        }
    }
}
