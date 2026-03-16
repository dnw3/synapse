//! Platform action tool for dispatching platform-specific actions across channels.

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

/// Unified tool for platform-specific actions across all channels.
/// Agent calls: platform_action(channel="discord", action="pin_message", params={msg_id: "123"})
pub struct PlatformActionTool;

#[async_trait]
impl Tool for PlatformActionTool {
    fn name(&self) -> &'static str {
        "platform_action"
    }

    fn description(&self) -> &'static str {
        "Execute a platform-specific action on a messaging channel (e.g., pin message, add reaction, create thread)"
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "channel": {
                    "type": "string",
                    "description": "Channel ID (e.g., 'discord', 'slack', 'lark')"
                },
                "action": {
                    "type": "string",
                    "description": "Action name (e.g., 'pin_message', 'create_thread')"
                },
                "params": {
                    "type": "object",
                    "description": "Action-specific parameters"
                }
            },
            "required": ["channel", "action"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let channel = args
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'channel' argument".into()))?;
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'action' argument".into()))?;
        let params = args
            .get("params")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        tracing::info!(channel = %channel, action = %action, "platform_action invoked");

        // TODO: Wire to ChannelRegistry to dispatch to actual adapter's PlatformActions impl
        // For now, return a placeholder response
        Ok(json!({
            "status": "not_connected",
            "message": format!("Platform action '{action}' on channel '{channel}' — channel registry not yet wired"),
            "channel": channel,
            "action": action,
            "params": params,
        }))
    }
}
