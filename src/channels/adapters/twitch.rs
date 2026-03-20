use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig, TwitchBotConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

/// Run the Twitch bot adapter using IRC over TCP (irc.chat.twitch.tv:6667).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let twitch_config = config
        .twitch
        .first()
        .ok_or("missing [[twitch]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = twitch_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "twitch",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "twitch", "adapter started (IRC mode)");

    loop {
        match run_twitch_irc(twitch_config, agent_session.clone(), &allowlist).await {
            Ok(()) => break,
            Err(e) => {
                tracing::warn!(channel = "twitch", error = %e, "connection error, reconnecting in 10s");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }

    Ok(())
}

async fn run_twitch_irc(
    config: &TwitchBotConfig,
    agent_session: Arc<AgentSession>,
    allowlist: &BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "irc.chat.twitch.tv:6667";
    let stream = TcpStream::connect(addr).await?;
    let (reader_half, writer_half) = stream.into_split();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let mut writer = tokio::io::BufWriter::new(writer_half);
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let raw = format!("{}\r\n", msg);
            if writer.write_all(raw.as_bytes()).await.is_err() {
                break;
            }
            let _ = writer.flush().await;
        }
    });

    let send = {
        let tx = tx.clone();
        move |line: String| {
            let _ = tx.send(line);
        }
    };

    // Authenticate with OAuth token
    let oauth_token = resolve_secret(
        config.oauth_token.as_deref(),
        config.oauth_token_env.as_deref(),
        "Twitch OAuth token",
    )
    .map_err(|e| format!("{}", e))?;
    send(format!("PASS oauth:{}", oauth_token));
    send(format!("NICK {}", config.nick));

    tracing::info!(channel = "twitch", nick = %config.nick, "connecting");

    // Join channels
    for channel in &config.channels {
        let ch = if channel.starts_with('#') {
            channel.clone()
        } else {
            format!("#{}", channel)
        };
        send(format!("JOIN {}", ch));
        tracing::info!(channel = "twitch", twitch_channel = %ch, "joining channel");
    }

    let own_nick = config.nick.to_lowercase();
    let mut lines = BufReader::new(reader_half).lines();

    while let Some(raw_line) = lines.next_line().await? {
        let line = raw_line.trim_end_matches('\r').to_string();

        if line.starts_with("PING ") {
            let token = &line[5..];
            send(format!("PONG {}", token));
            continue;
        }

        let parsed = match parse_twitch_privmsg(&line) {
            Some(p) => p,
            None => continue,
        };

        if parsed.sender_nick.to_lowercase() == own_nick {
            continue;
        }

        if parsed.message.is_empty() {
            continue;
        }

        if !allowlist.is_allowed(Some(&parsed.sender_nick), Some(&parsed.target)) {
            continue;
        }

        let session_key = parsed.target.clone();
        let session = agent_session.clone();
        let tx_clone = tx.clone();
        let reply_target = parsed.target.clone();
        let message = parsed.message.clone();
        let sender_nick = parsed.sender_nick.clone();

        tokio::spawn(async move {
            let channel_info = ChannelInfo {
                platform: "twitch".into(),
                native_channel_id: Some(session_key.clone()),
                ..Default::default()
            };
            let sender_info = SenderInfo {
                id: Some(sender_nick),
                ..Default::default()
            };
            let chat_info = ChatInfo {
                chat_type: "channel".into(),
                ..Default::default()
            };
            let mut msg = InboundMessage::channel(
                session_key.clone(),
                message.clone(),
                channel_info,
                sender_info,
                chat_info,
            );
            msg.finalize();
            match session.handle_inbound(msg).await {
                Ok(reply) => {
                    let chunks = formatter::format_for_channel(&reply.content, "twitch", 400);
                    for chunk in chunks {
                        for irc_line in chunk.lines() {
                            let _ =
                                tx_clone.send(format!("PRIVMSG {} :{}", reply_target, irc_line));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(channel = "twitch", error = %e, "agent error");
                }
            }
        });
    }

    Ok(())
}

struct TwitchMsg {
    sender_nick: String,
    target: String,
    message: String,
}

fn parse_twitch_privmsg(line: &str) -> Option<TwitchMsg> {
    // Twitch IRC format: @tags :nick!user@host PRIVMSG #channel :message
    let line = if line.starts_with('@') {
        // Strip tags prefix
        line.splitn(2, ' ').nth(1)?
    } else {
        line
    };

    if !line.starts_with(':') {
        return None;
    }

    let mut parts = line[1..].splitn(3, ' ');
    let prefix = parts.next()?;
    let command = parts.next()?;
    let rest = parts.next()?;

    if !command.eq_ignore_ascii_case("PRIVMSG") {
        return None;
    }

    let sender_nick = prefix.split('!').next()?.to_string();
    let mut rest_parts = rest.splitn(2, " :");
    let target = rest_parts.next()?.trim().to_string();
    let message = rest_parts.next().unwrap_or("").to_string();

    Some(TwitchMsg {
        sender_nick,
        target,
        message,
    })
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`TwitchAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Twitch IRC bot.
#[allow(dead_code)]
pub struct TwitchAdapter {
    /// OAuth token used for authentication.
    oauth_token: String,
    /// Bot's Twitch nick.
    nick: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl TwitchAdapter {
    pub fn new(oauth_token: impl Into<String>, nick: impl Into<String>) -> Self {
        Self {
            oauth_token: oauth_token.into(),
            nick: nick.into(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for TwitchAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "twitch".to_string(),
            name: "Twitch".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(500),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "twitch", "TwitchAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "twitch", "TwitchAdapter stopped");
        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => ChannelStatus::Connected,
            STATUS_ERROR => ChannelStatus::Error("adapter error".to_string()),
            _ => ChannelStatus::Disconnected,
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl Outbound for TwitchAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Placeholder: a full implementation would open a TCP connection to
        // irc.chat.twitch.tv:6667 and send PRIVMSG commands.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for TwitchAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Probe by attempting a TCP connection to the Twitch IRC endpoint.
        match tokio::net::TcpStream::connect("irc.chat.twitch.tv:6667").await {
            Ok(_) => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Err(e) => {
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(e.to_string())
            }
        }
    }
}
