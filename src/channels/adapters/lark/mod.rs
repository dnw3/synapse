mod policy;
mod setup;
mod streaming;

use std::sync::Arc;
use std::sync::RwLock;

use async_trait::async_trait;
use synaptic::core::channel::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as ChannelMessageEnvelope, Outbound,
};
use synaptic::core::{RunContext, SynapticError};
use synaptic::deep::StreamingOutputHandle;
use synaptic::lark::bot::events::{LarkEvent, LarkEventHandler};
use synaptic::lark::bot::CardActionEvent;
use synaptic::lark::{LarkBotClient, LarkMessageEvent, StreamingCardOptions};

use synaptic::DeliveryContext;

use crate::channels::dedup::MessageDedup;
use crate::channels::dm::FileDmPolicyEnforcer;
use crate::channels::formatter;
use crate::channels::handler::{AgentSession, StreamingOutput};
use crate::config::bots::{DmPolicy, GroupPolicy, GroupSessionScope, LarkRenderMode};
use crate::config::BotAllowlist;
use crate::gateway::messages::sender::{ChannelSender, SendResult};
use crate::gateway::messages::{Attachment, ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use crate::gateway::presence::now_ms;

use policy::{compute_session_key, strip_bot_mention};
use streaming::LarkStreamingOutput;

pub use setup::run;

// ---------------------------------------------------------------------------
// ChannelSender implementation
// ---------------------------------------------------------------------------

/// Outbound sender for the Lark channel.
#[allow(dead_code)]
pub struct LarkSender {
    /// Lark bot client for making API calls.
    pub client: LarkBotClient,
}

// ---------------------------------------------------------------------------
// ApproveNotifier implementation
// ---------------------------------------------------------------------------

/// Sends a notification to a Lark user when their DM pairing is approved.
pub struct LarkApproveNotifier {
    pub client: LarkBotClient,
}

#[async_trait]
impl crate::channels::dm::ApproveNotifier for LarkApproveNotifier {
    async fn notify_approved(&self, sender_id: &str) -> crate::error::Result<()> {
        self.client
            .send_text(
                "open_id",
                sender_id,
                "\u{60a8}\u{7684}\u{914d}\u{5bf9}\u{8bf7}\u{6c42}\u{5df2}\u{901a}\u{8fc7}\u{ff0c}\u{73b0}\u{5728}\u{53ef}\u{4ee5}\u{5f00}\u{59cb}\u{5bf9}\u{8bdd}\u{4e86}\u{3002}",
            )
            .await
            .map_err(|e| crate::error::SynapseError::Channel(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl ChannelSender for LarkSender {
    fn channel_id(&self) -> &str {
        "lark"
    }

    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        _meta: Option<&serde_json::Value>,
    ) -> crate::error::Result<SendResult> {
        let chat_id = target
            .to
            .as_deref()
            .and_then(|s| s.strip_prefix("chat:"))
            .ok_or("missing or invalid chat_id in delivery target (expected 'chat:<id>')")?;

        self.client
            .send_text("chat_id", chat_id, content)
            .await
            .map_err(|e| crate::error::SynapseError::Channel(e.to_string()))?;

        Ok(SendResult {
            message_id: None,
            delivered_at_ms: now_ms(),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Heuristic: does the text contain rich markdown that benefits from card rendering?
fn has_rich_content(text: &str) -> bool {
    text.contains("```")
        || text.contains("| --- |")
        || text.contains("**")
        || text.lines().any(|l| l.starts_with("# "))
}

// ---------------------------------------------------------------------------
// Card builders for device pairing
// ---------------------------------------------------------------------------

/// Build a Lark interactive card for displaying a setup code.
fn build_pair_card(setup_code: &str, gateway_url: &str, template: &str) -> serde_json::Value {
    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "title": { "tag": "plain_text", "content": "Device Pairing" },
            "template": template
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!(
                        "**Setup Code:**\n```\n{setup_code}\n```\n\n**Gateway:** {gateway_url}\n\n_Expires in 10 minutes_"
                    )
                }
            }
        ]
    })
}

/// Build a Lark interactive card for pairing approval.
#[allow(dead_code)]
fn build_approval_card(
    request_id: &str,
    device_name: &str,
    platform: &str,
    template: &str,
) -> serde_json::Value {
    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "title": { "tag": "plain_text", "content": "New Device Pairing Request" },
            "template": template
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!("**Device:** {device_name}\n**Platform:** {platform}")
                }
            },
            {
                "tag": "action",
                "actions": [
                    {
                        "tag": "button",
                        "text": { "tag": "plain_text", "content": "Approve" },
                        "type": "primary",
                        "value": format!("pair_approve:{request_id}")
                    },
                    {
                        "tag": "button",
                        "text": { "tag": "plain_text", "content": "Reject" },
                        "type": "danger",
                        "value": format!("pair_reject:{request_id}")
                    }
                ]
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

pub(crate) struct LarkHandler {
    pub(crate) agent_session: Arc<AgentSession>,
    pub(crate) config: Arc<LarkHandlerConfig>,
    pub(crate) dedup: Arc<MessageDedup>,
    pub(crate) bot_open_id: String,
    pub(crate) account_id: String,
    pub(crate) enforcer: Arc<FileDmPolicyEnforcer>,
    pub(crate) gateway_port: u16,
    #[allow(dead_code)]
    pub(crate) owner_chat_id: Option<String>,
}

/// Extracted config subset for the handler.
#[allow(dead_code)]
pub(crate) struct LarkHandlerConfig {
    pub(crate) render_mode: LarkRenderMode,
    pub(crate) streaming: bool,
    pub(crate) require_mention: bool,
    pub(crate) typing_indicator: bool,
    pub(crate) reply_in_thread: bool,
    pub(crate) group_session_scope: GroupSessionScope,
    pub(crate) dm_scope: crate::config::DmSessionScope,
    pub(crate) dm_policy: DmPolicy,
    pub(crate) group_policy: GroupPolicy,
    pub(crate) allowlist: BotAllowlist,
    pub(crate) text_chunk_limit: usize,
    pub(crate) card: crate::config::bots::LarkCardConfig,
    pub(crate) bot_name: String,
}

impl LarkHandler {
    /// Send a plain-text reply, respecting thread mode and chunk limits.
    async fn send_reply(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        text: &str,
    ) -> Result<(), SynapticError> {
        let chunks = formatter::format_for_channel(text, "lark", self.config.text_chunk_limit);
        for chunk in chunks {
            if self.config.reply_in_thread && event.has_thread() {
                client
                    .reply_text_in_thread(event.message_id(), &chunk)
                    .await?;
            } else {
                client.reply_text(event.message_id(), &chunk).await?;
            }
        }
        Ok(())
    }

    /// Handle text/post messages with streaming or plain reply.
    async fn handle_text_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
        text: &str,
    ) -> Result<(), SynapticError> {
        let use_streaming = self.config.streaming
            && matches!(
                self.config.render_mode,
                LarkRenderMode::Card | LarkRenderMode::Auto
            );

        let build_inbound = || {
            let channel_info = ChannelInfo {
                platform: "lark".into(),
                account_id: Some(self.account_id.clone()),
                native_channel_id: Some(event.chat_id().to_string()),
                ..Default::default()
            };
            let sender_info = SenderInfo {
                id: Some(event.sender_open_id().to_string()),
                ..Default::default()
            };
            let chat_info = ChatInfo {
                chat_type: if event.is_dm() {
                    "direct".to_string()
                } else {
                    "group".to_string()
                },
                ..Default::default()
            };
            let mut msg = InboundMessage::channel(
                session_key.to_string(),
                text.to_string(),
                channel_info,
                sender_info,
                chat_info,
            );
            msg.message.id = Some(event.message_id().to_string());
            msg.thread.thread_id = event.root_id.clone();
            msg.finalize();
            msg
        };

        if use_streaming {
            let card_cfg = &self.config.card;
            let title = if card_cfg.header_title.is_empty() {
                self.config.bot_name.clone()
            } else {
                card_cfg.header_title.clone()
            };
            let options = StreamingCardOptions::new()
                .with_title(title)
                .with_template(card_cfg.template.clone())
                .with_icon(card_cfg.header_icon.clone());

            let writer = client.streaming_reply(event.message_id(), options).await?;

            let output: Arc<dyn StreamingOutput> = Arc::new(LarkStreamingOutput {
                writer,
                card_config: self.config.card.clone(),
                bot_name: self.config.bot_name.clone(),
                reasoning_buffer: Arc::new(tokio::sync::RwLock::new(String::new())),
            });

            let msg = build_inbound();
            let streaming_handle = StreamingOutputHandle::new(output);
            let ctx = RunContext {
                cancel_token: None,
                streaming_output: Some(Arc::new(streaming_handle)),
            };
            self.agent_session
                .handle_message(msg, ctx)
                .await
                .map_err(|e| SynapticError::Tool(e.to_string()))?;
        } else {
            let msg = build_inbound();
            match self
                .agent_session
                .handle_message(msg, RunContext::default())
                .await
            {
                Ok(reply) => {
                    // Auto mode: use card for rich content even without streaming
                    if matches!(self.config.render_mode, LarkRenderMode::Auto)
                        && has_rich_content(&reply.content)
                    {
                        let card_cfg = &self.config.card;
                        let title = if card_cfg.header_title.is_empty() {
                            self.config.bot_name.clone()
                        } else {
                            card_cfg.header_title.clone()
                        };
                        let opts = StreamingCardOptions::new()
                            .with_title(title)
                            .with_template(card_cfg.template.clone())
                            .with_icon(card_cfg.header_icon.clone());
                        let writer = client.streaming_reply(event.message_id(), opts).await?;
                        writer.write(&reply.content).await.ok();
                        writer.finish().await.ok();
                    } else {
                        self.send_reply(event, client, &reply.content).await?;
                    }
                }
                Err(e) => {
                    client
                        .reply_text(event.message_id(), &format!("Error: {}", e))
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Handle image messages: download and pass as attachment.
    async fn handle_image_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
    ) -> Result<(), SynapticError> {
        let image_key = match event.image_key() {
            Some(k) => k.to_string(),
            None => {
                tracing::warn!("image message without image_key");
                return Ok(());
            }
        };

        let bytes = client.download_image(&image_key).await?;
        let tmp = std::env::temp_dir().join(format!("lark_img_{}.png", event.message_id()));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| SynapticError::Tool(format!("failed to write image: {}", e)))?;

        let attachments = vec![Attachment {
            filename: "image.png".into(),
            url: format!("file://{}", tmp.display()),
            mime_type: Some("image/png".into()),
        }];

        let channel_info = ChannelInfo {
            platform: "lark".into(),
            account_id: Some(self.account_id.clone()),
            native_channel_id: Some(event.chat_id().to_string()),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(event.sender_open_id().to_string()),
            ..Default::default()
        };
        let chat_info = ChatInfo {
            chat_type: if event.is_dm() {
                "direct".to_string()
            } else {
                "group".to_string()
            },
            ..Default::default()
        };
        let mut msg = InboundMessage::channel(
            session_key.to_string(),
            "[User sent an image]".to_string(),
            channel_info,
            sender_info,
            chat_info,
        );
        msg.attachments = attachments;
        msg.message.id = Some(event.message_id().to_string());
        msg.thread.thread_id = event.root_id.clone();
        msg.finalize();

        match self
            .agent_session
            .handle_message(msg, RunContext::default())
            .await
        {
            Ok(reply) => self.send_reply(event, client, &reply.content).await?,
            Err(e) => {
                client
                    .reply_text(event.message_id(), &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }

    /// Handle file messages: download and pass as attachment.
    async fn handle_file_message(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        session_key: &str,
    ) -> Result<(), SynapticError> {
        let file_key = match event.file_key() {
            Some(k) => k.to_string(),
            None => {
                tracing::warn!("file message without file_key");
                return Ok(());
            }
        };

        let filename = event.file_name().unwrap_or("file");
        let bytes = client
            .download_resource(event.message_id(), &file_key, "file")
            .await?;
        let tmp = std::env::temp_dir().join(format!("lark_file_{}", filename));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| SynapticError::Tool(format!("failed to write file: {}", e)))?;

        let attachments = vec![Attachment {
            filename: filename.into(),
            url: format!("file://{}", tmp.display()),
            mime_type: None,
        }];

        let channel_info = ChannelInfo {
            platform: "lark".into(),
            account_id: Some(self.account_id.clone()),
            native_channel_id: Some(event.chat_id().to_string()),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(event.sender_open_id().to_string()),
            ..Default::default()
        };
        let chat_info = ChatInfo {
            chat_type: if event.is_dm() {
                "direct".to_string()
            } else {
                "group".to_string()
            },
            ..Default::default()
        };
        let mut msg = InboundMessage::channel(
            session_key.to_string(),
            format!("[User sent file: {}]", filename),
            channel_info,
            sender_info,
            chat_info,
        );
        msg.attachments = attachments;
        msg.message.id = Some(event.message_id().to_string());
        msg.thread.thread_id = event.root_id.clone();
        msg.finalize();

        match self
            .agent_session
            .handle_message(msg, RunContext::default())
            .await
        {
            Ok(reply) => self.send_reply(event, client, &reply.content).await?,
            Err(e) => {
                client
                    .reply_text(event.message_id(), &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }
}

impl LarkHandler {
    /// Handle an incoming message event (dispatched from LarkEventHandler).
    async fn handle_message(
        &self,
        event: LarkMessageEvent,
        client: &LarkBotClient,
    ) -> Result<(), SynapticError> {
        // 1. Dedup
        if !self.dedup.check_and_mark(event.event_id()) {
            tracing::debug!(event_id = %event.event_id(), "duplicate event, skipping");
            return Ok(());
        }

        // 2. Policy
        if event.is_dm() {
            // DM policy: use enforcer
            if let Some(reply) = self.check_dm_access(event.sender_open_id()).await {
                if !reply.is_empty() {
                    client
                        .send_text("chat_id", event.chat_id(), &reply)
                        .await
                        .ok();
                }
                return Ok(());
            }
        } else if !self.check_group_policy(&event) {
            tracing::debug!(
                sender = %event.sender_open_id(),
                chat = %event.chat_id(),
                "message rejected by group policy"
            );
            return Ok(());
        }

        // 3. Mention gate (group only)
        let text = if event.is_group() && self.config.require_mention {
            if !event.mentions_bot(&self.bot_open_id) {
                return Ok(());
            }
            strip_bot_mention(event.text(), &event.mentions)
        } else {
            event.text().to_string()
        };

        if text.is_empty() && event.is_text() {
            return Ok(());
        }

        // 3.5 Command interception: /pair or 配对
        let text_trimmed = text.trim();
        if text_trimmed == "/pair" || text_trimmed == "\u{914d}\u{5bf9}" {
            let mut bootstrap = crate::gateway::nodes::BootstrapStore::new();
            let token = bootstrap.issue();
            let gateway_url = format!("ws://localhost:{}/ws", self.gateway_port);
            let setup_code =
                crate::gateway::nodes::bootstrap::encode_setup_code(&gateway_url, &token);
            let card = build_pair_card(&setup_code, &gateway_url, &self.config.card.template);
            client
                .send_card("chat_id", event.chat_id(), &card)
                .await
                .ok();
            return Ok(());
        }

        // 4. Session key (using centralized computation)
        let session_key = compute_session_key(
            &event,
            &self.config.group_session_scope,
            &self.config.dm_scope,
            &self.account_id,
        );

        // 5. Typing indicator (reaction)
        let typing_reaction = if self.config.typing_indicator {
            client.add_reaction(event.message_id(), "OnIt").await.ok()
        } else {
            None
        };

        // 6. Route by message type
        let result = match event.message_type_str() {
            "text" | "post" => {
                self.handle_text_message(&event, client, &session_key, &text)
                    .await
            }
            "image" => {
                self.handle_image_message(&event, client, &session_key)
                    .await
            }
            "file" => self.handle_file_message(&event, client, &session_key).await,
            other => {
                tracing::debug!(message_type = %other, "unsupported message type");
                Ok(())
            }
        };

        // 7. Remove typing indicator
        if let Some(reaction_id) = typing_reaction {
            client
                .remove_reaction(event.message_id(), &reaction_id)
                .await
                .ok();
        }

        if let Err(ref e) = result {
            tracing::error!(error = %e, "lark message handling failed");
        }
        result
    }

    /// Handle a card action event (dispatched from LarkEventHandler).
    async fn handle_card_action(
        &self,
        event: CardActionEvent,
        client: &LarkBotClient,
    ) -> Result<(), SynapticError> {
        let text = event
            .action_value
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| event.action_value.get("command").and_then(|v| v.as_str()))
            .map(String::from)
            .unwrap_or_else(|| {
                format!(
                    "[Card action: {} = {}]",
                    event.action_tag, event.action_value
                )
            });

        let session_key = format!("lark:card:{}", event.chat_id);
        let channel_info = ChannelInfo {
            platform: "lark".into(),
            native_channel_id: Some(event.chat_id.clone()),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(event.operator_open_id.clone()),
            ..Default::default()
        };
        let chat_info = ChatInfo {
            chat_type: "group".to_string(),
            ..Default::default()
        };
        let mut msg =
            InboundMessage::channel(session_key, text, channel_info, sender_info, chat_info);
        msg.finalize();
        match self
            .agent_session
            .handle_message(msg, RunContext::default())
            .await
        {
            Ok(reply) => {
                client
                    .send_text("chat_id", &event.chat_id, &reply.content)
                    .await?;
            }
            Err(e) => {
                client
                    .send_text("chat_id", &event.chat_id, &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl LarkEventHandler for LarkHandler {
    async fn handle(&self, event: LarkEvent, client: &LarkBotClient) -> Result<(), SynapticError> {
        match event {
            LarkEvent::Message(msg) => self.handle_message(msg, client).await,
            LarkEvent::CardAction(card) => self.handle_card_action(card, client).await,
            LarkEvent::BotAdded(e) => {
                tracing::info!(chat_id = %e.chat_id, operator = %e.operator_open_id, "Bot added to group");
                Ok(())
            }
            LarkEvent::BotRemoved(e) => {
                tracing::info!(chat_id = %e.chat_id, operator = %e.operator_open_id, "Bot removed from group");
                Ok(())
            }
            LarkEvent::GroupDisbanded(e) => {
                tracing::info!(chat_id = %e.chat_id, "Group disbanded");
                Ok(())
            }
            LarkEvent::ReactionCreated(e) => {
                tracing::debug!(message_id = %e.message_id, emoji = %e.emoji_type, "Reaction added");
                Ok(())
            }
            _ => {
                tracing::debug!("Unhandled lark event");
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// A thin adapter struct that wraps a [`LarkBotClient`] to expose the
/// synaptic channel traits without disturbing the existing event-loop code.
#[allow(dead_code)]
pub struct LarkChannelAdapter {
    client: LarkBotClient,
    status: RwLock<ChannelStatus>,
}

#[allow(dead_code)]
impl LarkChannelAdapter {
    pub fn new(client: LarkBotClient) -> Self {
        Self {
            client,
            status: RwLock::new(ChannelStatus::Disconnected),
        }
    }
}

#[async_trait]
impl ChannelAdapter for LarkChannelAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "lark".to_string(),
            name: "Lark / Feishu".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Threading,
                ChannelCap::Reactions,
                ChannelCap::Mentions,
                ChannelCap::Health,
            ],
            message_limit: Some(4096),
            supports_streaming: true,
            supports_threads: true,
            supports_reactions: true,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), SynapticError> {
        *self.status.write().unwrap() = ChannelStatus::Connecting;
        // The actual long-poll loop is managed externally via `run()`.
        // Mark as connected optimistically; the loop will surface errors.
        *self.status.write().unwrap() = ChannelStatus::Connected;
        Ok(())
    }

    async fn stop(&self) -> Result<(), SynapticError> {
        *self.status.write().unwrap() = ChannelStatus::Disconnected;
        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        self.status.read().unwrap().clone()
    }
}

#[async_trait]
impl Outbound for LarkChannelAdapter {
    /// Send a text message to `envelope.channel_id` (interpreted as a Lark
    /// chat_id) with the textual content from `envelope.content`.
    async fn send(&self, envelope: &ChannelMessageEnvelope) -> Result<(), SynapticError> {
        self.client
            .send_text("chat_id", &envelope.channel_id, &envelope.content)
            .await?;
        Ok(())
    }

    /// Edit a previously sent interactive card.  `msg_id` must be the Lark
    /// card token; `content` is used as the new markdown body.
    /// Sequence is set to 0 (initial update); callers needing strict ordering
    /// should use the Lark client API directly.
    async fn edit(&self, msg_id: &str, content: &str) -> Result<(), SynapticError> {
        let card = serde_json::json!({
            "config": { "wide_screen_mode": true },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": content
                    }
                }
            ]
        });
        self.client.update_card(msg_id, 0, &card).await?;
        Ok(())
    }
}

#[async_trait]
impl ChannelHealth for LarkChannelAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.client.get_bot_info().await {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Unhealthy(format!("Lark API unreachable: {}", e)),
        }
    }
}
