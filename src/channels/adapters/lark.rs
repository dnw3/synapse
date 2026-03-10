use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::lark::bot::session::MentionInfo;
use synaptic::lark::bot::{CardActionEvent, CardActionHandler, MessageHandler};
use synaptic::lark::{
    LarkBotClient, LarkConfig, LarkLongConnListener, LarkMessageEvent, StreamingCardOptions,
    StreamingCardWriter,
};

use crate::agent;
use crate::channels::dedup::MessageDedup;
use crate::channels::formatter;
use crate::channels::handler::{AgentSession, Attachment, StreamingOutput};
use crate::config::bot::{resolve_secret, DmPolicy, GroupPolicy, GroupSessionScope, LarkRenderMode};
use crate::config::{BotAllowlist, SynapseConfig};

// ---------------------------------------------------------------------------
// Streaming output adapter
// ---------------------------------------------------------------------------

struct LarkStreamingOutput {
    writer: StreamingCardWriter,
}

#[async_trait]
impl StreamingOutput for LarkStreamingOutput {
    async fn on_token(&self, token: &str) {
        self.writer.write(token).await.ok();
    }

    async fn on_tool_call(&self, tool_name: &str) {
        self.writer
            .write(&format!("\n> Using tool: {}\n", tool_name))
            .await
            .ok();
    }

    async fn on_complete(&self, _full_response: &str) {
        self.writer.finish().await.ok();
    }

    async fn on_error(&self, error: &str) {
        self.writer
            .write(&format!("\n**Error:** {}", error))
            .await
            .ok();
        self.writer.finish().await.ok();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute session key based on chat type and configured scope.
fn compute_session_key(event: &LarkMessageEvent, scope: &GroupSessionScope) -> String {
    let chat_id = event.chat_id();
    if event.is_dm() {
        return format!("lark:dm:{}", chat_id);
    }
    match scope {
        GroupSessionScope::Group => format!("lark:grp:{}", chat_id),
        GroupSessionScope::GroupSender => {
            format!("lark:grp:{}:{}", chat_id, event.sender_open_id())
        }
        GroupSessionScope::GroupTopic => {
            if let Some(ref root) = event.root_id {
                format!("lark:grp:{}:t:{}", chat_id, root)
            } else {
                format!("lark:grp:{}", chat_id)
            }
        }
        GroupSessionScope::GroupTopicSender => {
            let topic = event.root_id.as_deref().unwrap_or("_");
            format!("lark:grp:{}:t:{}:{}", chat_id, topic, event.sender_open_id())
        }
    }
}

/// Strip bot @mention placeholders from message text.
fn strip_bot_mention(text: &str, mentions: &[MentionInfo]) -> String {
    let mut result = text.to_string();
    for m in mentions {
        result = result.replace(&m.key, "");
    }
    result.trim().to_string()
}

/// Heuristic: does the text contain rich markdown that benefits from card rendering?
fn has_rich_content(text: &str) -> bool {
    text.contains("```")
        || text.contains("| --- |")
        || text.contains("**")
        || text.lines().any(|l| l.starts_with("# "))
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

struct LarkHandler {
    agent_session: Arc<AgentSession>,
    config: Arc<LarkHandlerConfig>,
    dedup: Arc<MessageDedup>,
    bot_open_id: String,
}

/// Extracted config subset for the handler.
struct LarkHandlerConfig {
    render_mode: LarkRenderMode,
    streaming: bool,
    require_mention: bool,
    typing_indicator: bool,
    reply_in_thread: bool,
    group_session_scope: GroupSessionScope,
    dm_policy: DmPolicy,
    group_policy: GroupPolicy,
    allowlist: BotAllowlist,
    text_chunk_limit: usize,
}

impl LarkHandler {
    /// Check DM/group access policy. Returns `true` if the message is allowed.
    fn check_policy(&self, event: &LarkMessageEvent) -> bool {
        if event.is_dm() {
            match self.config.dm_policy {
                DmPolicy::Open => true,
                DmPolicy::Allowlist => {
                    self.config.allowlist.is_user_allowed(event.sender_open_id())
                }
            }
        } else {
            match self.config.group_policy {
                GroupPolicy::Open => true,
                GroupPolicy::Disabled => false,
                GroupPolicy::Allowlist => {
                    self.config.allowlist.is_channel_allowed(event.chat_id())
                }
            }
        }
    }

    /// Send a plain-text reply, respecting thread mode and chunk limits.
    async fn send_reply(
        &self,
        event: &LarkMessageEvent,
        client: &LarkBotClient,
        text: &str,
    ) -> Result<(), SynapticError> {
        let chunks = formatter::chunk_message(text, self.config.text_chunk_limit);
        for chunk in chunks {
            if self.config.reply_in_thread && event.has_thread() {
                client.reply_text_in_thread(event.message_id(), &chunk).await?;
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
            && matches!(self.config.render_mode, LarkRenderMode::Card | LarkRenderMode::Auto);

        if use_streaming {
            let writer = client
                .streaming_reply(
                    event.message_id(),
                    StreamingCardOptions::new().with_title("Synapse"),
                )
                .await?;

            let output: Arc<dyn StreamingOutput> = Arc::new(LarkStreamingOutput { writer });

            self.agent_session
                .handle_message_streaming(session_key, text, Vec::new(), output)
                .await
                .map_err(|e| SynapticError::Tool(e.to_string()))?;
        } else {
            match self.agent_session.handle_message(session_key, text).await {
                Ok(reply) => {
                    // Auto mode: use card for rich content even without streaming
                    if matches!(self.config.render_mode, LarkRenderMode::Auto)
                        && has_rich_content(&reply)
                    {
                        let writer = client
                            .streaming_reply(
                                event.message_id(),
                                StreamingCardOptions::new().with_title("Synapse"),
                            )
                            .await?;
                        writer.write(&reply).await.ok();
                        writer.finish().await.ok();
                    } else {
                        self.send_reply(event, client, &reply).await?;
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
        let image_key = match &event.image_key {
            Some(k) => k.clone(),
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

        match self
            .agent_session
            .handle_message_with_attachments(session_key, "[User sent an image]", &attachments)
            .await
        {
            Ok(reply) => self.send_reply(event, client, &reply).await?,
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
        let file_key = match &event.file_key {
            Some(k) => k.clone(),
            None => {
                tracing::warn!("file message without file_key");
                return Ok(());
            }
        };

        let filename = event.file_name.as_deref().unwrap_or("file");
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

        match self
            .agent_session
            .handle_message_with_attachments(
                session_key,
                &format!("[User sent file: {}]", filename),
                &attachments,
            )
            .await
        {
            Ok(reply) => self.send_reply(event, client, &reply).await?,
            Err(e) => {
                client
                    .reply_text(event.message_id(), &format!("Error: {}", e))
                    .await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MessageHandler for LarkHandler {
    async fn handle(
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
        if !self.check_policy(&event) {
            tracing::debug!(
                sender = %event.sender_open_id(),
                chat = %event.chat_id(),
                "message rejected by policy"
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

        // 4. Session key
        let session_key = compute_session_key(&event, &self.config.group_session_scope);

        // 5. Typing indicator (reaction)
        let typing_reaction = if self.config.typing_indicator {
            client.add_reaction(event.message_id(), "OnIt").await.ok()
        } else {
            None
        };

        // 6. Route by message type
        let result = match event.message_type.as_str() {
            "text" | "post" => {
                self.handle_text_message(&event, client, &session_key, &text)
                    .await
            }
            "image" => self.handle_image_message(&event, client, &session_key).await,
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
}

// ---------------------------------------------------------------------------
// Card action handler
// ---------------------------------------------------------------------------

struct LarkCardHandler {
    agent_session: Arc<AgentSession>,
}

#[async_trait]
impl CardActionHandler for LarkCardHandler {
    async fn handle(
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
                format!("[Card action: {} = {}]", event.action_tag, event.action_value)
            });

        let session_key = format!("lark:card:{}", event.chat_id);
        match self.agent_session.handle_message(&session_key, &text).await {
            Ok(reply) => {
                client.send_text("chat_id", &event.chat_id, &reply).await?;
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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the Lark bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lark_config = config
        .lark
        .as_ref()
        .ok_or("missing [lark] section in config")?;

    let app_secret = resolve_secret(
        lark_config.app_secret.as_deref(),
        lark_config.app_secret_env.as_deref(),
        "Lark app secret",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let lark = LarkConfig::new(&lark_config.app_id, &app_secret);
    let client = LarkBotClient::new(lark.clone());

    // Fetch bot info for mention detection
    let bot_info = client
        .get_bot_info()
        .await
        .map_err(|e| format!("failed to get bot info: {}", e))?;

    tracing::info!(
        channel = "lark",
        app_id = %lark_config.app_id,
        bot_name = %bot_info.app_name,
        bot_id = %bot_info.open_id,
        "adapter started"
    );

    let handler_config = Arc::new(LarkHandlerConfig {
        render_mode: lark_config.render_mode.clone(),
        streaming: lark_config.streaming,
        require_mention: lark_config.require_mention,
        typing_indicator: lark_config.typing_indicator,
        reply_in_thread: lark_config.reply_in_thread,
        group_session_scope: lark_config.group_session_scope.clone(),
        dm_policy: lark_config.dm_policy.clone(),
        group_policy: lark_config.group_policy.clone(),
        allowlist: lark_config.allowlist.clone(),
        text_chunk_limit: lark_config.text_chunk_limit,
    });

    let msg_handler = LarkHandler {
        agent_session: agent_session.clone(),
        config: handler_config,
        dedup: Arc::new(MessageDedup::new(2048)),
        bot_open_id: bot_info.open_id,
    };

    let card_handler = LarkCardHandler { agent_session };

    let listener = LarkLongConnListener::new(lark)
        .with_message_handler(msg_handler)
        .with_card_action_handler(card_handler);

    listener.run().await?;
    Ok(())
}
