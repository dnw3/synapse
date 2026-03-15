use serde::{Deserialize, Serialize};
use synaptic::DeliveryContext;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum RouteError {
    #[error("no delivery route available for session")]
    NoRoute,
    #[error("channel '{0}' is not registered or offline")]
    ChannelOffline(String),
    #[error("cross-channel conflict: {0}")]
    CrossChannelConflict(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionDeliveryState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_context: Option<DeliveryContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_turn_source: Option<TurnSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSource {
    pub turn_id: String,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

/// Resolve the delivery target for an agent reply.
/// Priority: active_turn_source > explicit delivery_context > last_* history.
pub fn resolve_delivery_target(
    state: &SessionDeliveryState,
    current_turn: Option<&TurnSource>,
) -> Result<DeliveryContext, RouteError> {
    // 1. Current turn source (prevents cross-channel races)
    if let Some(turn) = current_turn {
        return Ok(DeliveryContext {
            channel: turn.channel.clone(),
            to: turn.to.clone(),
            account_id: turn.account_id.clone(),
            thread_id: turn.thread_id.clone(),
            meta: None,
        });
    }

    // 2. Explicit session-level delivery context
    if let Some(ref ctx) = state.delivery_context {
        return Ok(ctx.clone());
    }

    // 3. Last-used route (implicit history)
    if let Some(ref channel) = state.last_channel {
        return Ok(DeliveryContext {
            channel: channel.clone(),
            to: state.last_to.clone(),
            account_id: state.last_account_id.clone(),
            thread_id: state.last_thread_id.clone(),
            meta: None,
        });
    }

    Err(RouteError::NoRoute)
}

/// Update last_* fields from a delivery context.
/// On channel change, replaces all fields (no cross-channel merge).
pub fn update_last_route(state: &mut SessionDeliveryState, delivery: &DeliveryContext) {
    let channel_changed = state.last_channel.as_deref() != Some(&delivery.channel);
    if channel_changed {
        state.last_to = None;
        state.last_account_id = None;
        state.last_thread_id = None;
    }
    state.last_channel = Some(delivery.channel.clone());
    if delivery.to.is_some() {
        state.last_to.clone_from(&delivery.to);
    }
    if delivery.account_id.is_some() {
        state.last_account_id.clone_from(&delivery.account_id);
    }
    if delivery.thread_id.is_some() {
        state.last_thread_id.clone_from(&delivery.thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_turn_source_highest_priority() {
        let state = SessionDeliveryState {
            last_channel: Some("slack".into()),
            last_to: Some("channel:general".into()),
            ..Default::default()
        };
        let turn = TurnSource {
            turn_id: "req-1".into(),
            channel: "telegram".into(),
            to: Some("chat:456".into()),
            account_id: None,
            thread_id: None,
        };
        let target = resolve_delivery_target(&state, Some(&turn)).unwrap();
        assert_eq!(target.channel, "telegram");
        assert_eq!(target.to.as_deref(), Some("chat:456"));
    }

    #[test]
    fn test_resolve_explicit_context_second() {
        let state = SessionDeliveryState {
            delivery_context: Some(DeliveryContext {
                channel: "discord".into(),
                to: Some("channel:lobby".into()),
                ..Default::default()
            }),
            last_channel: Some("slack".into()),
            ..Default::default()
        };
        let target = resolve_delivery_target(&state, None).unwrap();
        assert_eq!(target.channel, "discord");
    }

    #[test]
    fn test_resolve_last_route_third() {
        let state = SessionDeliveryState {
            last_channel: Some("lark".into()),
            last_to: Some("chat:789".into()),
            last_thread_id: Some("thread-1".into()),
            ..Default::default()
        };
        let target = resolve_delivery_target(&state, None).unwrap();
        assert_eq!(target.channel, "lark");
        assert_eq!(target.thread_id.as_deref(), Some("thread-1"));
    }

    #[test]
    fn test_resolve_no_route() {
        let state = SessionDeliveryState::default();
        let result = resolve_delivery_target(&state, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_last_route_same_channel() {
        let mut state = SessionDeliveryState {
            last_channel: Some("slack".into()),
            last_to: Some("channel:general".into()),
            last_thread_id: Some("old-thread".into()),
            ..Default::default()
        };
        let delivery = DeliveryContext {
            channel: "slack".into(),
            to: Some("channel:random".into()),
            thread_id: None,
            ..Default::default()
        };
        update_last_route(&mut state, &delivery);
        assert_eq!(state.last_to.as_deref(), Some("channel:random"));
        assert_eq!(state.last_thread_id.as_deref(), Some("old-thread"));
    }

    #[test]
    fn test_update_last_route_channel_change_clears() {
        let mut state = SessionDeliveryState {
            last_channel: Some("slack".into()),
            last_to: Some("channel:general".into()),
            last_thread_id: Some("slack-thread".into()),
            ..Default::default()
        };
        let delivery = DeliveryContext {
            channel: "telegram".into(),
            to: Some("chat:456".into()),
            ..Default::default()
        };
        update_last_route(&mut state, &delivery);
        assert_eq!(state.last_channel.as_deref(), Some("telegram"));
        assert_eq!(state.last_to.as_deref(), Some("chat:456"));
        assert!(state.last_thread_id.is_none());
    }
}
