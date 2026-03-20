use synaptic::lark::{LarkMessageEvent, MentionInfo};

use crate::channels::dm::{DmAccessDenied, DmPolicyEnforcer};
use crate::config::bots::{GroupPolicy, GroupSessionScope};

use super::LarkHandler;

// ---------------------------------------------------------------------------
// Policy enforcement
// ---------------------------------------------------------------------------

impl LarkHandler {
    /// Check group access policy. Returns `true` if the message is allowed.
    pub(super) fn check_group_policy(&self, event: &LarkMessageEvent) -> bool {
        match self.config.group_policy {
            GroupPolicy::Open => true,
            GroupPolicy::Disabled => false,
            GroupPolicy::Allowlist => self.config.allowlist.is_channel_allowed(event.chat_id()),
        }
    }

    /// Check DM access using the enforcer. Returns Some(reply_text) if blocked.
    pub(super) async fn check_dm_access(&self, sender_id: &str) -> Option<String> {
        match self.enforcer.check_access(sender_id, "lark").await {
            Ok(()) => None,
            Err(DmAccessDenied::NeedsPairing(challenge)) => {
                let ttl_mins = challenge.ttl_ms / 60_000;
                let ttl_desc = if ttl_mins >= 60 {
                    format!("{} \u{5c0f}\u{65f6}", ttl_mins / 60)
                } else {
                    format!("{} \u{5206}\u{949f}", ttl_mins)
                };
                Some(format!(
                    "\u{8bf7}\u{5c06}\u{4ee5}\u{4e0b}\u{914d}\u{5bf9}\u{7801}\u{53d1}\u{9001}\u{7ed9}\u{7ba1}\u{7406}\u{5458}\u{4ee5}\u{5b8c}\u{6210}\u{9a8c}\u{8bc1}\u{ff1a}\n\n\u{1f511} {}\n\n\u{914d}\u{5bf9}\u{7801}\u{6709}\u{6548}\u{671f} {}\u{3002}",
                    challenge.code, ttl_desc
                ))
            }
            Err(DmAccessDenied::NotAllowed) => Some("\u{62b1}\u{6b49}\u{ff0c}\u{60a8}\u{672a}\u{83b7}\u{5f97}\u{6388}\u{6743}\u{4f7f}\u{7528}\u{6b64}\u{673a}\u{5668}\u{4eba}\u{3002}".to_string()),
            Err(DmAccessDenied::DmDisabled) => None, // silently ignore
        }
    }
}

// ---------------------------------------------------------------------------
// Session key computation
// ---------------------------------------------------------------------------

/// Compute session key based on chat type and configured scope.
pub(super) fn compute_session_key(
    event: &LarkMessageEvent,
    scope: &GroupSessionScope,
    dm_scope: &crate::config::DmSessionScope,
    account_id: &str,
) -> String {
    use crate::channels::session_key::{self, ChatType, SessionKeyParams};

    let chat_type = if event.is_dm() {
        ChatType::Dm
    } else {
        ChatType::Group
    };
    let peer_id = if event.is_dm() {
        event.sender_open_id()
    } else {
        event.chat_id()
    };
    session_key::compute(&SessionKeyParams {
        agent_id: "default", // Will be overridden by router in handler
        channel: "lark",
        account_id: Some(account_id),
        chat_type,
        peer_id,
        sender_id: Some(event.sender_open_id()),
        thread_id: event.root_id.as_deref(),
        dm_scope,
        group_scope: scope,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Strip bot @mention placeholders from message text.
pub(super) fn strip_bot_mention(text: &str, mentions: &[MentionInfo]) -> String {
    let mut result = text.to_string();
    for m in mentions {
        result = result.replace(&m.key, "");
    }
    result.trim().to_string()
}
