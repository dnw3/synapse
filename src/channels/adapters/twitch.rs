use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig, TwitchBotConfig};

/// Run the Twitch bot adapter using IRC over TCP (irc.chat.twitch.tv:6667).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let twitch_config = config
        .twitch
        .as_ref()
        .ok_or("missing [twitch] section in config")?;

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

        tokio::spawn(async move {
            match session.handle_message(&session_key, &message).await {
                Ok(reply) => {
                    let chunks = formatter::chunk_irc(&reply);
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
