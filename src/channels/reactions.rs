//! Bot reaction helpers — lightweight emoji feedback on messages.

/// Add a reaction to a Slack message.
#[cfg(feature = "bot-slack")]
pub async fn slack_react(bot_token: &str, channel: &str, timestamp: &str, emoji: &str) {
    let client = reqwest::Client::new();
    let _ = client
        .post("https://slack.com/api/reactions.add")
        .bearer_auth(bot_token)
        .json(&serde_json::json!({
            "channel": channel,
            "timestamp": timestamp,
            "name": emoji,
        }))
        .send()
        .await;
}

/// Add a reaction to a Telegram message.
#[cfg(feature = "bot-telegram")]
pub async fn telegram_react(base_url: &str, chat_id: i64, message_id: i64, emoji: &str) {
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{}/setMessageReaction", base_url))
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reaction": [{"type": "emoji", "emoji": emoji}],
        }))
        .send()
        .await;
}

/// Add a reaction to a Discord message.
#[cfg(feature = "bot-discord")]
pub async fn discord_react(token: &str, channel_id: &str, message_id: &str, emoji: &str) {
    let client = reqwest::Client::new();
    let encoded = urlencoding::encode(emoji);
    let _ = client
        .put(format!(
            "https://discord.com/api/v10/channels/{}/messages/{}/reactions/{}/@me",
            channel_id, message_id, encoded
        ))
        .header("Authorization", format!("Bot {}", token))
        .header("Content-Length", "0")
        .send()
        .await;
}
