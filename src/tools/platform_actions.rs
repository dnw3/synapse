//! Platform action tool for dispatching platform-specific actions across channels.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};
use tokio::sync::RwLock;

use crate::gateway::messages::ChannelRegistry;

/// Unified tool for platform-specific actions across all channels.
/// Agent calls: platform_action(channel="discord", action="pin_message", params={msg_id: "123"})
pub struct PlatformActionTool {
    /// Live channel registry — injected at gateway startup when available.
    ///
    /// When `None` (e.g. REPL or bot-only mode), the tool reports every channel
    /// as "not_connected".  When `Some`, it queries the registry so the agent can
    /// see which channels are actually online and — once per-channel dispatch is
    /// implemented — route actions to the right adapter.
    ///
    /// # Integration path
    ///
    /// Full per-action dispatch requires each `ChannelSender` implementation to
    /// also implement a `PlatformActions` extension trait (or a similar mechanism
    /// that provides runtime type info).  Until that trait is defined and wired,
    /// the tool uses the registry only for presence/reachability checks and
    /// returns an `action_queued` stub for known channels.
    channel_registry: Option<Arc<RwLock<ChannelRegistry>>>,
}

impl PlatformActionTool {
    /// Create without a registry (REPL / bot-only contexts).
    pub fn new() -> Self {
        Self {
            channel_registry: None,
        }
    }

    /// Create with a live channel registry (gateway context).
    pub fn with_registry(registry: Arc<RwLock<ChannelRegistry>>) -> Self {
        Self {
            channel_registry: Some(registry),
        }
    }
}

impl Default for PlatformActionTool {
    fn default() -> Self {
        Self::new()
    }
}

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

        // Query the channel registry when available.
        let Some(ref registry) = self.channel_registry else {
            return Ok(json!({
                "status": "not_connected",
                "message": format!("Channel registry not available in this context. \
                    Platform action '{action}' on channel '{channel}' cannot be dispatched."),
                "channel": channel,
                "action": action,
                "params": params,
            }));
        };

        let reg = registry.read().await;
        let connected_channels: Vec<&str> = reg.list();

        let channel_connected = connected_channels.contains(&channel);

        if !channel_connected {
            let available = if connected_channels.is_empty() {
                "none".to_string()
            } else {
                let mut ch = connected_channels.clone();
                ch.sort();
                ch.join(", ")
            };
            tracing::warn!(
                channel = %channel,
                action = %action,
                available = %available,
                "platform_action: channel not in registry"
            );
            return Ok(json!({
                "status": "channel_not_found",
                "message": format!("Channel '{channel}' is not registered. \
                    Connected channels: {available}"),
                "channel": channel,
                "action": action,
                "params": params,
                "connected_channels": connected_channels,
            }));
        }

        // Channel is connected.  Full per-action dispatch requires the ChannelSender
        // to implement a PlatformActions extension trait (pending design).  Until that
        // trait is defined, we acknowledge the request and note the integration path.
        //
        // TODO(platform_actions): define `PlatformActions` trait on ChannelSender impls,
        // then call `sender.dispatch_action(action, &params).await` here.
        tracing::info!(
            channel = %channel,
            action = %action,
            "platform_action: channel connected, action dispatch pending PlatformActions trait"
        );

        Ok(json!({
            "status": "action_queued",
            "message": format!("Channel '{channel}' is connected. \
                Action '{action}' acknowledged but not yet dispatched — \
                full dispatch requires PlatformActions trait on channel adapters."),
            "channel": channel,
            "action": action,
            "params": params,
        }))
    }
}
