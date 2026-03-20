use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};
use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, IrcBotConfig, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

/// Run the IRC bot adapter using raw TCP.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let irc_config = config
        .irc
        .first()
        .ok_or("missing [[irc]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = irc_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "irc",
            nicks = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "irc", "adapter started (TCP mode)");

    loop {
        match run_tcp(irc_config, agent_session.clone(), &allowlist).await {
            Ok(()) => break,
            Err(e) => {
                tracing::warn!(channel = "irc", error = %e, "connection error, reconnecting in 10s");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }

    Ok(())
}

/// Connect to the IRC server over TCP, register, join channels, and process messages.
///
/// A tokio mpsc channel separates the read path (main loop) from the write
/// path (background agent tasks), avoiding the need to share a mutex around
/// the writer half of the TCP stream.
async fn run_tcp(
    irc_config: &IrcBotConfig,
    agent_session: Arc<AgentSession>,
    allowlist: &BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = irc_config.port.unwrap_or(6667);
    let addr = format!("{}:{}", irc_config.server, port);

    let stream = TcpStream::connect(&addr).await?;
    let (reader_half, writer_half) = stream.into_split();

    // mpsc channel: clones of `tx` go to spawned agent tasks; `rx` drives the writer task.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // Writer task — receives raw IRC command strings and flushes them to the socket.
    let mut writer = tokio::io::BufWriter::new(writer_half);
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let raw = format!("{}\r\n", msg);
            if writer.write_all(raw.as_bytes()).await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    });

    // Convenience closure: enqueue a line for sending.
    let send = {
        let tx = tx.clone();
        move |line: String| {
            let _ = tx.send(line);
        }
    };

    // Optional server password (must precede NICK/USER per RFC 1459).
    if irc_config.password.is_some() || irc_config.password_env.is_some() {
        let password = resolve_secret(
            irc_config.password.as_deref(),
            irc_config.password_env.as_deref(),
            "IRC password",
        )?;
        send(format!("PASS {}", password));
    }

    // Registration.
    send(format!("NICK {}", irc_config.nick));
    send(format!("USER {} 0 * :Synapse AI Bot", irc_config.nick));

    tracing::info!(channel = "irc", addr = %addr, nick = %irc_config.nick, "connecting");

    let own_nick = irc_config.nick.clone();
    let channels = irc_config.channels.clone();
    let mut registered = false;
    let mut lines = BufReader::new(reader_half).lines();

    while let Some(raw_line) = lines.next_line().await? {
        let line = raw_line.trim_end_matches('\r').to_string();

        // Keep-alive: respond to server PING immediately.
        if line.starts_with("PING ") {
            let token = &line[5..];
            send(format!("PONG {}", token));
            continue;
        }

        // Wait for RPL_WELCOME (001) before joining channels.
        if !registered {
            if is_numeric(&line, "001") {
                registered = true;
                tracing::info!(channel = "irc", "registered, joining channels");
                for channel in &channels {
                    send(format!("JOIN {}", channel));
                    tracing::info!(channel = "irc", irc_channel = %channel, "joined channel");
                }
            }
            continue;
        }

        // Only handle PRIVMSG lines.
        let parsed = match parse_privmsg(&line) {
            Some(p) => p,
            None => continue,
        };

        // Skip own messages.
        if parsed.sender_nick.eq_ignore_ascii_case(&own_nick) {
            continue;
        }

        // Determine reply target: channel messages reply to the channel;
        // private messages (target == own nick) reply to the sender.
        let reply_target = if parsed.target.eq_ignore_ascii_case(&own_nick) {
            parsed.sender_nick.clone()
        } else {
            parsed.target.clone()
        };

        let message = parsed.message.clone();
        let sender_nick = parsed.sender_nick.clone();
        let channel_or_nick = parsed.target.clone();

        if message.is_empty() {
            continue;
        }

        // Allowlist: sender nick → allowed_users, channel/target → allowed_channels.
        if !allowlist.is_allowed(Some(&sender_nick), Some(&channel_or_nick)) {
            continue;
        }

        // Private messages (target == own nick) are DMs; channel messages are channels.
        let is_dm = parsed.target.eq_ignore_ascii_case(&own_nick);

        // Spawn agent processing in the background.
        let session = agent_session.clone();
        let tx_clone = tx.clone();
        // Use reply_target as the per-conversation session key.
        let session_key = reply_target.clone();

        tokio::spawn(async move {
            let mut envelope = MessageEnvelope::channel(
                session_key.clone(),
                message.clone(),
                DeliveryContext {
                    channel: "irc".into(),
                    to: Some(format!("channel:{}", session_key)),
                    account_id: None,
                    thread_id: None,
                    meta: None,
                },
            );
            envelope.sender_id = Some(sender_nick);
            envelope.routing.peer_kind = Some(if is_dm {
                crate::config::PeerKind::Direct
            } else {
                crate::config::PeerKind::Channel
            });
            envelope.routing.peer_id = Some(session_key.clone());
            match session.handle_message(envelope).await {
                Ok(reply) => {
                    let chunks = formatter::format_for_channel(&reply.content, "irc", 400);
                    for chunk in chunks {
                        // Each chunk is already ≤400 chars; send line-by-line
                        // in case the agent included embedded newlines.
                        for irc_line in chunk.lines() {
                            let _ =
                                tx_clone.send(format!("PRIVMSG {} :{}", reply_target, irc_line));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(channel = "irc", error = %e, "agent error");
                }
            }
        });
    }

    Ok(())
}

/// Parsed fields from an IRC PRIVMSG line.
struct PrivMsg {
    sender_nick: String,
    target: String,
    message: String,
}

/// Parse an IRC PRIVMSG line.
///
/// Expected format: `:nick!user@host PRIVMSG <target> :<text>`
fn parse_privmsg(line: &str) -> Option<PrivMsg> {
    if !line.starts_with(':') {
        return None;
    }

    let mut parts = line[1..].splitn(3, ' ');
    let prefix = parts.next()?; // nick!user@host
    let command = parts.next()?; // PRIVMSG
    let rest = parts.next()?; // <target> :<text>

    if !command.eq_ignore_ascii_case("PRIVMSG") {
        return None;
    }

    let sender_nick = prefix.split('!').next()?.to_string();

    let mut rest_parts = rest.splitn(2, " :");
    let target = rest_parts.next()?.trim().to_string();
    let message = rest_parts.next().unwrap_or("").to_string();

    Some(PrivMsg {
        sender_nick,
        target,
        message,
    })
}

/// Return true if `line` carries the given IRC numeric reply code.
///
/// Format: `:server <code> <nick> ...`
fn is_numeric(line: &str, code: &str) -> bool {
    if !line.starts_with(':') {
        return false;
    }
    let parts: Vec<&str> = line[1..].splitn(3, ' ').collect();
    parts.get(1).map(|c| *c == code).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`IrcAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the IRC bot.
#[allow(dead_code)]
pub struct IrcAdapter {
    /// IRC server host, e.g. `"irc.libera.chat"`.
    server: String,
    /// IRC server port (default 6667).
    port: u16,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl IrcAdapter {
    pub fn new(server: impl Into<String>, port: u16) -> Self {
        Self {
            server: server.into(),
            port,
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for IrcAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "irc".to_string(),
            name: "IRC".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(512),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "irc", "IrcAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "irc", "IrcAdapter stopped");
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
impl Outbound for IrcAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Placeholder: a full implementation would open a TCP connection to
        // `self.server:self.port` and send a PRIVMSG command.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for IrcAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Probe by attempting a TCP connection to the IRC server.
        let addr = format!("{}:{}", self.server, self.port);
        match tokio::net::TcpStream::connect(&addr).await {
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
