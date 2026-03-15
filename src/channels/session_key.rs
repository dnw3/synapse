//! Centralized session key computation.
//!
//! Produces deterministic keys in the format:
//!   `agent:{agent_id}:{channel}:{kind}:{peer_id}[:{extras}]`
//!
//! All adapters should use this module instead of computing keys locally.

use crate::config::{DmSessionScope, GroupSessionScope};

/// Chat type for session key computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatType {
    /// Direct message (1:1 DM).
    Dm,
    /// Group conversation.
    Group,
}

/// Parameters for computing a session key.
pub struct SessionKeyParams<'a> {
    /// Agent ID (e.g. "home", "work", "default").
    pub agent_id: &'a str,
    /// Channel name (e.g. "lark", "discord", "slack").
    pub channel: &'a str,
    /// Account ID within the channel (for multi-account setups).
    pub account_id: Option<&'a str>,
    /// Whether this is a DM or group chat.
    pub chat_type: ChatType,
    /// Primary peer ID — sender_id for DMs, chat_id for groups.
    pub peer_id: &'a str,
    /// Sender ID (for group session scoping by sender).
    pub sender_id: Option<&'a str>,
    /// Thread/topic ID (for group session scoping by topic).
    pub thread_id: Option<&'a str>,
    /// DM session scope.
    pub dm_scope: &'a DmSessionScope,
    /// Group session scope.
    pub group_scope: &'a GroupSessionScope,
}

/// Compute a deterministic session key.
///
/// Format: `agent:{agent_id}:{channel}:{kind}:{peer_id}[:{extras}]`
pub fn compute(p: &SessionKeyParams) -> String {
    match p.chat_type {
        ChatType::Dm => compute_dm_key(p),
        ChatType::Group => compute_group_key(p),
    }
}

fn compute_dm_key(p: &SessionKeyParams) -> String {
    match p.dm_scope {
        DmSessionScope::Main => {
            format!("agent:{}:main", p.agent_id)
        }
        DmSessionScope::PerPeer => {
            format!("agent:{}:{}:dm:{}", p.agent_id, p.channel, p.peer_id)
        }
        DmSessionScope::PerChannelPeer => {
            format!("agent:{}:{}:dm:{}", p.agent_id, p.channel, p.peer_id)
        }
        DmSessionScope::PerAccountChannelPeer => {
            let acct = p.account_id.unwrap_or("default");
            format!(
                "agent:{}:{}:{}:dm:{}",
                p.agent_id, p.channel, acct, p.peer_id
            )
        }
    }
}

fn compute_group_key(p: &SessionKeyParams) -> String {
    let base = format!("agent:{}:{}:grp:{}", p.agent_id, p.channel, p.peer_id);
    match p.group_scope {
        GroupSessionScope::Group => base,
        GroupSessionScope::GroupSender => {
            let sender = p.sender_id.unwrap_or("unknown");
            format!("{}:sender:{}", base, sender)
        }
        GroupSessionScope::GroupTopic => {
            let topic = p.thread_id.unwrap_or("main");
            format!("{}:topic:{}", base, topic)
        }
        GroupSessionScope::GroupTopicSender => {
            let topic = p.thread_id.unwrap_or("main");
            let sender = p.sender_id.unwrap_or("unknown");
            format!("{}:topic:{}:sender:{}", base, topic, sender)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dm_main_scope() {
        let key = compute(&SessionKeyParams {
            agent_id: "home",
            channel: "lark",
            account_id: None,
            chat_type: ChatType::Dm,
            peer_id: "ou_abc123",
            sender_id: None,
            thread_id: None,
            dm_scope: &DmSessionScope::Main,
            group_scope: &GroupSessionScope::Group,
        });
        assert_eq!(key, "agent:home:main");
    }

    #[test]
    fn dm_per_peer() {
        let key = compute(&SessionKeyParams {
            agent_id: "home",
            channel: "lark",
            account_id: None,
            chat_type: ChatType::Dm,
            peer_id: "ou_abc123",
            sender_id: None,
            thread_id: None,
            dm_scope: &DmSessionScope::PerPeer,
            group_scope: &GroupSessionScope::Group,
        });
        assert_eq!(key, "agent:home:lark:dm:ou_abc123");
    }

    #[test]
    fn dm_per_channel_peer() {
        let key = compute(&SessionKeyParams {
            agent_id: "work",
            channel: "discord",
            account_id: None,
            chat_type: ChatType::Dm,
            peer_id: "user456",
            sender_id: None,
            thread_id: None,
            dm_scope: &DmSessionScope::PerChannelPeer,
            group_scope: &GroupSessionScope::Group,
        });
        assert_eq!(key, "agent:work:discord:dm:user456");
    }

    #[test]
    fn dm_per_account_channel_peer() {
        let key = compute(&SessionKeyParams {
            agent_id: "home",
            channel: "whatsapp",
            account_id: Some("personal"),
            chat_type: ChatType::Dm,
            peer_id: "+1555000",
            sender_id: None,
            thread_id: None,
            dm_scope: &DmSessionScope::PerAccountChannelPeer,
            group_scope: &GroupSessionScope::Group,
        });
        assert_eq!(key, "agent:home:whatsapp:personal:dm:+1555000");
    }

    #[test]
    fn group_basic() {
        let key = compute(&SessionKeyParams {
            agent_id: "default",
            channel: "lark",
            account_id: None,
            chat_type: ChatType::Group,
            peer_id: "oc_group123",
            sender_id: Some("ou_sender"),
            thread_id: None,
            dm_scope: &DmSessionScope::PerChannelPeer,
            group_scope: &GroupSessionScope::Group,
        });
        assert_eq!(key, "agent:default:lark:grp:oc_group123");
    }

    #[test]
    fn group_sender_scope() {
        let key = compute(&SessionKeyParams {
            agent_id: "default",
            channel: "lark",
            account_id: None,
            chat_type: ChatType::Group,
            peer_id: "oc_group123",
            sender_id: Some("ou_sender"),
            thread_id: None,
            dm_scope: &DmSessionScope::PerChannelPeer,
            group_scope: &GroupSessionScope::GroupSender,
        });
        assert_eq!(key, "agent:default:lark:grp:oc_group123:sender:ou_sender");
    }

    #[test]
    fn group_topic_sender_scope() {
        let key = compute(&SessionKeyParams {
            agent_id: "work",
            channel: "slack",
            account_id: None,
            chat_type: ChatType::Group,
            peer_id: "C12345",
            sender_id: Some("U999"),
            thread_id: Some("thread_ts"),
            dm_scope: &DmSessionScope::PerChannelPeer,
            group_scope: &GroupSessionScope::GroupTopicSender,
        });
        assert_eq!(
            key,
            "agent:work:slack:grp:C12345:topic:thread_ts:sender:U999"
        );
    }
}
