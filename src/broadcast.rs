//! Broadcast messaging — send to multiple channels simultaneously.

use colored::Colorize;

use crate::config::BroadcastGroup;

/// Send a message to all targets in a broadcast group.
pub async fn broadcast(
    group: &BroadcastGroup,
    message: &str,
    tokens: &BroadcastTokens,
) -> Vec<BroadcastResult> {
    let mut results = Vec::new();

    for target in &group.targets {
        let (platform, channel_id) = match target.split_once(':') {
            Some(pair) => pair,
            None => {
                results.push(BroadcastResult {
                    target: target.clone(),
                    success: false,
                    error: Some(format!(
                        "invalid target format '{}' (expected platform:channel_id)",
                        target
                    )),
                });
                continue;
            }
        };

        let result = match platform {
            "slack" => send_slack(channel_id, message, tokens.slack_token.as_deref()).await,
            "telegram" => {
                send_telegram(channel_id, message, tokens.telegram_token.as_deref()).await
            }
            "discord" => {
                send_discord(channel_id, message, tokens.discord_token.as_deref()).await
            }
            _ => Err(format!("unsupported platform '{}'", platform)),
        };

        results.push(BroadcastResult {
            target: target.clone(),
            success: result.is_ok(),
            error: result.err(),
        });
    }

    results
}

/// Tokens needed for broadcast sending.
pub struct BroadcastTokens {
    pub slack_token: Option<String>,
    pub telegram_token: Option<String>,
    pub discord_token: Option<String>,
}

impl BroadcastTokens {
    /// Load tokens from the environment using config env var names.
    pub fn from_config(config: &crate::config::SynapseConfig) -> Self {
        let slack_token = config
            .slack
            .as_ref()
            .and_then(|c| std::env::var(&c.bot_token_env).ok());
        let telegram_token = config
            .telegram
            .as_ref()
            .and_then(|c| std::env::var(&c.bot_token_env).ok());
        let discord_token = config
            .discord
            .as_ref()
            .and_then(|c| std::env::var(&c.bot_token_env).ok());

        Self {
            slack_token,
            telegram_token,
            discord_token,
        }
    }
}

pub struct BroadcastResult {
    pub target: String,
    pub success: bool,
    pub error: Option<String>,
}

async fn send_slack(channel: &str, message: &str, token: Option<&str>) -> Result<(), String> {
    let token = token.ok_or("Slack bot token not configured")?;
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&serde_json::json!({
            "channel": channel,
            "text": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Slack send failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Slack API returned {}", resp.status()));
    }
    Ok(())
}

async fn send_telegram(chat_id: &str, message: &str, token: Option<&str>) -> Result<(), String> {
    let token = token.ok_or("Telegram bot token not configured")?;
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
        }))
        .send()
        .await
        .map_err(|e| format!("Telegram send failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Telegram API returned {}", resp.status()));
    }
    Ok(())
}

async fn send_discord(
    channel_id: &str,
    message: &str,
    token: Option<&str>,
) -> Result<(), String> {
    let token = token.ok_or("Discord bot token not configured")?;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        ))
        .header("Authorization", format!("Bot {}", token))
        .json(&serde_json::json!({"content": message}))
        .send()
        .await
        .map_err(|e| format!("Discord send failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Discord API returned {}", resp.status()));
    }
    Ok(())
}

/// Display broadcast results.
pub fn display_results(results: &[BroadcastResult]) {
    for r in results {
        if r.success {
            eprintln!("  {} {}", "✓".green(), r.target);
        } else {
            eprintln!(
                "  {} {} — {}",
                "✗".red(),
                r.target,
                r.error.as_deref().unwrap_or("unknown error")
            );
        }
    }
}
