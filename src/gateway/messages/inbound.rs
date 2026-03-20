//! Inbound message type aligned with OpenClaw's `MsgContext`.
//!
//! `InboundMessage` replaces `MessageEnvelope` as the unified inbound message
//! representation for all channels (web, Lark, Telegram, Discord, Slack, etc.).

use super::envelope::Attachment;
use crate::gateway::presence::now_ms;

// ---------------------------------------------------------------------------
// Sub-structs
// ---------------------------------------------------------------------------

/// Identity of the message sender.
#[derive(Default, Clone, Debug)]
pub struct SenderInfo {
    /// User/sender identifier (platform-specific ID).
    pub id: Option<String>,
    /// Human-readable display name.
    pub name: Option<String>,
    /// Username for @mention generation.
    pub username: Option<String>,
    /// Platform mention tag format (e.g. "<@U123>").
    pub tag: Option<String>,
    /// Phone number in E.164 format (Signal, WhatsApp).
    pub e164: Option<String>,
    /// Whether the sender is a bot.
    pub is_bot: bool,
}

/// Channel/platform origin information.
#[derive(Default, Clone, Debug)]
pub struct ChannelInfo {
    /// Platform code: "lark", "telegram", "discord", "slack", "web", etc.
    pub platform: String,
    /// Platform surface label (may differ from platform for sub-surfaces).
    pub surface: Option<String>,
    /// Multi-account provider ID.
    pub account_id: Option<String>,
    /// Platform-native channel ID.
    pub native_channel_id: Option<String>,
    /// Bot username for mention normalization.
    pub bot_username: Option<String>,
    /// Guild/workspace ID (Discord guild, Slack workspace).
    pub guild_id: Option<String>,
    /// Team ID (Slack, Teams).
    pub team_id: Option<String>,
}

/// Conversation / chat context.
#[derive(Default, Clone, Debug)]
pub struct ChatInfo {
    /// Chat type: "direct", "group", "channel", "forum".
    pub chat_type: String,
    /// Group/channel display name.
    pub group_name: Option<String>,
    /// Group topic/subject line.
    pub group_subject: Option<String>,
    /// Group channel label (#general).
    pub group_channel: Option<String>,
    /// Workspace/space name.
    pub workspace: Option<String>,
    /// Member list or count.
    pub group_members: Option<String>,
    /// Group-specific system prompt override.
    pub group_system_prompt: Option<String>,
    /// Conversation display label.
    pub conversation_label: Option<String>,
    /// User roles in the group (Discord roles, etc.).
    pub roles: Vec<String>,
}

/// Thread context for threaded conversations.
#[derive(Default, Clone, Debug)]
pub struct ThreadInfo {
    /// Thread identifier (Telegram topic, Matrix thread, Lark thread).
    pub thread_id: Option<String>,
    /// Parent session key (for thread-scoped sessions).
    pub parent_session_key: Option<String>,
    /// Root message ID for thread reconstruction.
    pub root_message_id: Option<String>,
    /// Discord thread parent channel ID.
    pub parent_channel_id: Option<String>,
    /// Whether this is the first message in the thread.
    pub is_first_turn: bool,
    /// Thread display label.
    pub label: Option<String>,
    /// Whether this is a Telegram forum supergroup.
    pub is_forum: bool,
    /// Topic required but missing (Telegram validation).
    pub topic_required_but_missing: bool,
    /// Thread history body (full thread context).
    pub history_body: Option<String>,
    /// Thread starter body (first message).
    pub starter_body: Option<String>,
}

/// Platform-level message identifiers and reply/forward context.
#[derive(Default, Clone, Debug)]
pub struct MessageInfo {
    /// Platform-specific message ID (short form).
    pub id: Option<String>,
    /// Full platform-specific message ID.
    pub id_full: Option<String>,
    /// Multiple message IDs (batched messages).
    pub ids: Vec<String>,
    /// First message ID in thread range.
    pub id_first: Option<String>,
    /// Last message ID in thread range.
    pub id_last: Option<String>,
    /// Whether the bot was @mentioned in this message.
    pub was_mentioned: bool,
    /// Reply context.
    pub reply_to: Option<ReplyInfo>,
    /// Forward context.
    pub forward_from: Option<ForwardInfo>,
}

/// Context for a reply-to message.
#[derive(Default, Clone, Debug)]
pub struct ReplyInfo {
    pub message_id: Option<String>,
    pub message_id_full: Option<String>,
    pub body: Option<String>,
    pub sender: Option<String>,
    pub is_quote: bool,
}

/// Context for a forwarded message.
#[derive(Default, Clone, Debug)]
pub struct ForwardInfo {
    pub sender_name: Option<String>,
    pub sender_username: Option<String>,
    pub sender_id: Option<String>,
    pub channel_name: Option<String>,
    pub message_id: Option<String>,
}

/// Media attachments and understanding results.
#[derive(Default, Clone, Debug)]
pub struct MediaInfo {
    pub paths: Vec<String>,
    pub urls: Vec<String>,
    pub types: Vec<String>,
    pub dir: Option<String>,
    pub output_dir: Option<String>,
    pub transcript: Option<String>,
    pub understanding: Vec<MediaUnderstanding>,
    pub understanding_decisions: Vec<String>,
    pub link_understanding: Vec<String>,
    pub sticker: Option<StickerInfo>,
    pub remote_host: Option<String>,
}

/// Result of media understanding (vision, transcription, etc.).
#[derive(Default, Clone, Debug)]
pub struct MediaUnderstanding {
    pub media_type: String,
    pub content: String,
}

/// Sticker metadata (Telegram, etc.).
#[derive(Default, Clone, Debug)]
pub struct StickerInfo {
    pub emoji: Option<String>,
    pub set_name: Option<String>,
    pub is_animated: bool,
    pub is_video: bool,
    pub media_included: bool,
}

/// Content variants for different consumers (agent, command parser, raw).
#[derive(Default, Clone, Debug)]
pub struct ContentVariants {
    pub body: Option<String>,
    pub body_for_agent: Option<String>,
    pub body_for_commands: Option<String>,
    pub raw_body: Option<String>,
}

/// Command system metadata.
#[derive(Default, Clone, Debug)]
pub struct CommandInfo {
    pub authorized: Option<bool>,
    pub args: Option<serde_json::Value>,
    pub source: Option<String>,
    pub target_session_key: Option<String>,
    pub owner_allow_from: Vec<String>,
    pub untrusted_context: Vec<String>,
    pub gateway_client_scopes: Vec<String>,
}

// ---------------------------------------------------------------------------
// InboundMessage
// ---------------------------------------------------------------------------

/// Unified inbound message from any channel, aligned with OpenClaw's `MsgContext`.
///
/// Replaces `MessageEnvelope`. All channel adapters construct this type.
#[derive(Default, Clone, Debug)]
pub struct InboundMessage {
    // === Core ===
    pub request_id: String,
    pub session_key: String,
    pub timestamp_ms: u64,
    pub idempotency_key: Option<String>,

    // === Content ===
    pub content: String,
    pub content_variants: ContentVariants,
    pub attachments: Vec<Attachment>,

    // === Identity ===
    pub sender: SenderInfo,
    pub channel: ChannelInfo,

    // === Conversation context ===
    pub chat: ChatInfo,
    pub thread: ThreadInfo,
    pub message: MessageInfo,

    // === Media ===
    pub media: MediaInfo,

    // === Command system ===
    pub command: CommandInfo,
}

#[allow(dead_code)]
impl InboundMessage {
    /// Create an inbound message for the web UI channel.
    pub fn web(request_id: String, session_key: String, content: String, conn_id: &str) -> Self {
        Self {
            request_id,
            session_key,
            content,
            channel: ChannelInfo {
                platform: "web".into(),
                ..Default::default()
            },
            chat: ChatInfo {
                chat_type: "direct".into(),
                ..Default::default()
            },
            sender: SenderInfo {
                id: Some(format!("conn:{}", conn_id)),
                ..Default::default()
            },
            timestamp_ms: now_ms(),
            ..Default::default()
        }
    }

    /// Create an inbound message for a bot channel adapter.
    pub fn channel(
        session_key: String,
        content: String,
        channel: ChannelInfo,
        sender: SenderInfo,
        chat: ChatInfo,
    ) -> Self {
        Self {
            session_key,
            content,
            channel,
            sender,
            chat,
            timestamp_ms: now_ms(),
            ..Default::default()
        }
    }

    /// Convert to legacy `MessageEnvelope` for backward compatibility.
    ///
    /// Used during migration — will be removed when all consumers use `InboundMessage` directly.
    /// The conversion is intentionally lossy: fields unique to `InboundMessage` (media, command,
    /// content_variants, message info, etc.) are not carried over.
    pub fn to_envelope(&self) -> super::envelope::MessageEnvelope {
        use super::envelope::RoutingMeta;
        use synaptic::{DeliveryContext, InputProvenance, ProvenanceKind};

        let delivery = DeliveryContext {
            channel: self.channel.platform.clone(),
            to: self
                .sender
                .id
                .clone()
                .map(|id| format!("{}:{}", self.chat.chat_type, id)),
            account_id: self.channel.account_id.clone(),
            thread_id: self.thread.thread_id.clone(),
            ..Default::default()
        };

        let provenance = InputProvenance {
            kind: ProvenanceKind::ExternalUser,
            source_channel: Some(self.channel.platform.clone()),
            ..Default::default()
        };

        let routing = RoutingMeta {
            guild_id: self.channel.guild_id.clone(),
            team_id: self.channel.team_id.clone(),
            roles: self.chat.roles.clone(),
            peer_kind: if self.chat.chat_type == "direct" {
                Some(crate::config::PeerKind::Direct)
            } else if self.chat.chat_type == "group" {
                Some(crate::config::PeerKind::Group)
            } else {
                None
            },
            peer_id: self.sender.id.clone(),
        };

        super::envelope::MessageEnvelope {
            request_id: self.request_id.clone(),
            session_key: self.session_key.clone(),
            content: self.content.clone(),
            attachments: self.attachments.clone(),
            delivery,
            provenance,
            idempotency_key: self.idempotency_key.clone(),
            timestamp_ms: self.timestamp_ms,
            sender_id: self.sender.id.clone(),
            routing,
        }
    }

    /// Finalize the inbound message with default-deny semantics and fallback values.
    ///
    /// After `finalize()`:
    /// - `command.authorized` is always `Some` (defaults to `false`)
    /// - `content_variants.body_for_agent` is always `Some`
    /// - `chat.chat_type` is never empty
    /// - `media.types` is padded to match `media.urls`/`media.paths` length
    pub fn finalize(&mut self) {
        if self.command.authorized.is_none() {
            self.command.authorized = Some(false);
        }
        if self.content_variants.body_for_agent.is_none() {
            self.content_variants.body_for_agent = Some(self.content.clone());
        }
        if self.chat.chat_type.is_empty() {
            self.chat.chat_type = "direct".to_string();
        }
        while self.media.types.len() < self.media.urls.len().max(self.media.paths.len()) {
            self.media
                .types
                .push("application/octet-stream".to_string());
        }
    }
}
