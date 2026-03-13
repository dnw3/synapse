//! Gateway event names for server-push notifications.

/// All supported gateway event names.
pub const GATEWAY_EVENTS: &[&str] = &[
    "agent.message.start",
    "agent.message.delta",
    "agent.message.complete",
    "agent.tool.start",
    "agent.tool.result",
    "agent.thinking.start",
    "agent.thinking.delta",
    "agent.thinking.complete",
    "agent.error",
    "agent.turn.complete",
    "approval.requested",
    "approval.resolved",
    "presence.changed",
    "health.changed",
    "session.created",
    "session.updated",
    "session.deleted",
    "config.changed",
    "system.shutdown",
];
