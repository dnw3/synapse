#[cfg(feature = "bot-lark")]
pub mod lark;

#[cfg(feature = "bot-slack")]
pub mod slack;

#[cfg(feature = "bot-telegram")]
pub mod telegram;

#[cfg(feature = "bot-discord")]
pub mod discord;

#[cfg(feature = "bot-dingtalk")]
pub mod dingtalk;

#[cfg(feature = "bot-mattermost")]
pub mod mattermost;

#[cfg(feature = "bot-matrix")]
pub mod matrix;

#[cfg(feature = "bot-teams")]
pub mod teams;

#[cfg(feature = "bot-whatsapp")]
pub mod whatsapp;

#[cfg(feature = "bot-signal")]
pub mod signal;

#[cfg(feature = "bot-imessage")]
pub mod bluebubbles;

#[cfg(feature = "bot-imessage")]
pub mod imessage;

#[cfg(feature = "bot-line")]
pub mod line;

#[cfg(feature = "bot-googlechat")]
pub mod googlechat;

#[cfg(feature = "bot-wechat")]
pub mod wechat;

#[cfg(feature = "bot-irc")]
pub mod irc;

#[cfg(feature = "bot-webchat")]
pub mod webchat;

#[cfg(feature = "bot-twitch")]
pub mod twitch;

#[cfg(feature = "bot-nostr")]
pub mod nostr;

#[cfg(feature = "bot-nextcloud")]
pub mod nextcloud;

#[cfg(feature = "bot-synology")]
pub mod synology;

#[cfg(feature = "bot-tlon")]
pub mod tlon;

#[cfg(feature = "bot-zalo")]
pub mod zalo;
