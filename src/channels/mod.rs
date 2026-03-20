pub mod adapters;
pub mod dedup;
pub mod dm;
pub mod formatter;
pub mod handler;
pub mod platform_renderers;
pub mod reactions;
pub mod session_key;

use crate::config::SynapseConfig;

/// Run a bot adapter by platform name.
pub async fn run_bot(
    config: &SynapseConfig,
    platform: &str,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match platform {
        #[cfg(feature = "bot-lark")]
        "lark" | "feishu" => adapters::lark::run(config, model_override, None, None, None).await,

        #[cfg(feature = "bot-slack")]
        "slack" => adapters::slack::run(config, model_override).await,

        #[cfg(feature = "bot-telegram")]
        "telegram" => adapters::telegram::run(config, model_override).await,

        #[cfg(feature = "bot-discord")]
        "discord" => adapters::discord::run(config, model_override).await,

        #[cfg(feature = "bot-dingtalk")]
        "dingtalk" => adapters::dingtalk::run(config, model_override).await,

        #[cfg(feature = "bot-mattermost")]
        "mattermost" => adapters::mattermost::run(config, model_override).await,

        #[cfg(feature = "bot-matrix")]
        "matrix" => adapters::matrix::run(config, model_override).await,

        #[cfg(feature = "bot-teams")]
        "teams" => adapters::teams::run(config, model_override).await,

        #[cfg(feature = "bot-whatsapp")]
        "whatsapp" => adapters::whatsapp::run(config, model_override).await,

        #[cfg(feature = "bot-signal")]
        "signal" => adapters::signal::run(config, model_override).await,

        #[cfg(feature = "bot-imessage")]
        "imessage" => adapters::imessage::run(config, model_override).await,

        #[cfg(feature = "bot-line")]
        "line" => adapters::line::run(config, model_override).await,

        #[cfg(feature = "bot-googlechat")]
        "googlechat" | "gchat" => adapters::googlechat::run(config, model_override).await,

        #[cfg(feature = "bot-wechat")]
        "wechat" | "wecom" => adapters::wechat::run(config, model_override).await,

        #[cfg(feature = "bot-irc")]
        "irc" => adapters::irc::run(config, model_override).await,

        #[cfg(feature = "bot-webchat")]
        "webchat" => adapters::webchat::run(config, model_override).await,

        #[cfg(feature = "bot-twitch")]
        "twitch" => adapters::twitch::run(config, model_override).await,

        #[cfg(feature = "bot-nostr")]
        "nostr" => adapters::nostr::run(config, model_override).await,

        #[cfg(feature = "bot-nextcloud")]
        "nextcloud" => adapters::nextcloud::run(config, model_override).await,

        #[cfg(feature = "bot-synology")]
        "synology" => adapters::synology::run(config, model_override).await,

        #[cfg(feature = "bot-tlon")]
        "tlon" | "urbit" => adapters::tlon::run(config, model_override).await,

        #[cfg(feature = "bot-zalo")]
        "zalo" => adapters::zalo::run(config, model_override).await,

        _ => {
            let available = available_platforms();
            Err(format!(
                "unknown platform '{}'. Available: {}",
                platform,
                if available.is_empty() {
                    "none (enable bot features)".to_string()
                } else {
                    available.join(", ")
                }
            )
            .into())
        }
    }
}

#[allow(clippy::vec_init_then_push)]
fn available_platforms() -> Vec<&'static str> {
    let mut platforms = Vec::new();
    #[cfg(feature = "bot-lark")]
    platforms.push("lark");
    #[cfg(feature = "bot-slack")]
    platforms.push("slack");
    #[cfg(feature = "bot-telegram")]
    platforms.push("telegram");
    #[cfg(feature = "bot-discord")]
    platforms.push("discord");
    #[cfg(feature = "bot-dingtalk")]
    platforms.push("dingtalk");
    #[cfg(feature = "bot-mattermost")]
    platforms.push("mattermost");
    #[cfg(feature = "bot-matrix")]
    platforms.push("matrix");
    #[cfg(feature = "bot-teams")]
    platforms.push("teams");
    #[cfg(feature = "bot-whatsapp")]
    platforms.push("whatsapp");
    #[cfg(feature = "bot-signal")]
    platforms.push("signal");
    #[cfg(feature = "bot-imessage")]
    platforms.push("imessage");
    #[cfg(feature = "bot-line")]
    platforms.push("line");
    #[cfg(feature = "bot-googlechat")]
    platforms.push("googlechat");
    #[cfg(feature = "bot-wechat")]
    platforms.push("wechat");
    #[cfg(feature = "bot-irc")]
    platforms.push("irc");
    #[cfg(feature = "bot-webchat")]
    platforms.push("webchat");
    #[cfg(feature = "bot-twitch")]
    platforms.push("twitch");
    #[cfg(feature = "bot-nostr")]
    platforms.push("nostr");
    #[cfg(feature = "bot-nextcloud")]
    platforms.push("nextcloud");
    #[cfg(feature = "bot-synology")]
    platforms.push("synology");
    #[cfg(feature = "bot-tlon")]
    platforms.push("tlon");
    #[cfg(feature = "bot-zalo")]
    platforms.push("zalo");
    platforms
}
