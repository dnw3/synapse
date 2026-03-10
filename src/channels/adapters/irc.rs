use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{BotAllowlist, IrcBotConfig, SynapseConfig};

/// Run the IRC bot adapter using raw TCP.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let irc_config = config
        .irc
        .as_ref()
        .ok_or("missing [irc] section in config")?;

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
        let password = resolve_secret(irc_config.password.as_deref(), irc_config.password_env.as_deref(), "IRC password")?;
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

        // Spawn agent processing in the background.
        let session = agent_session.clone();
        let tx_clone = tx.clone();
        // Use reply_target as the per-conversation session key.
        let session_key = reply_target.clone();

        tokio::spawn(async move {
            match session.handle_message(&session_key, &message).await {
                Ok(reply) => {
                    let chunks = formatter::chunk_irc(&reply);
                    for chunk in chunks {
                        // Each chunk is already ≤400 chars; send line-by-line
                        // in case the agent included embedded newlines.
                        for irc_line in chunk.lines() {
                            let _ = tx_clone
                                .send(format!("PRIVMSG {} :{}", reply_target, irc_line));
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
    let rest = parts.next()?;   // <target> :<text>

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
