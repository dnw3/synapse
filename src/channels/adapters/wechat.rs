use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use tracing;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

/// WeCom Bot Webhook URL template.
const WECOM_WEBHOOK_BASE: &str = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send";

/// Shared state for the axum callback server.
#[allow(dead_code)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: BotAllowlist,
    webhook_key: String,
    token: Option<String>,
}

/// Query parameters for GET verification request.
#[derive(Debug, serde::Deserialize)]
struct VerifyQuery {
    msg_signature: Option<String>,
    timestamp: Option<String>,
    nonce: Option<String>,
    echostr: Option<String>,
}

/// Query parameters present on POST callback requests.
#[derive(Debug, serde::Deserialize)]
struct CallbackQuery {
    msg_signature: Option<String>,
    timestamp: Option<String>,
    nonce: Option<String>,
}

/// WeCom callback XML message (plain-text, non-encrypted mode).
///
/// WeCom sends messages in XML format with fields like:
/// ```xml
/// <xml>
///   <ToUserName><![CDATA[ww...]]></ToUserName>
///   <FromUserName><![CDATA[user_id]]></FromUserName>
///   <CreateTime>1234567890</CreateTime>
///   <MsgType><![CDATA[text]]></MsgType>
///   <Content><![CDATA[Hello]]></Content>
///   <MsgId>12345678</MsgId>
///   <AgentID>0</AgentID>
/// </xml>
/// ```
#[derive(Debug)]
struct WeChatMessage {
    from_user_name: String,
    msg_type: String,
    content: String,
}

/// Minimal XML parser for WeCom callback messages.
///
/// Extracts the values of `<FromUserName>`, `<MsgType>`, and `<Content>`
/// CDATA fields from the XML body without pulling in an XML dependency.
fn parse_wecom_xml(xml: &str) -> Option<WeChatMessage> {
    let from_user_name = extract_cdata(xml, "FromUserName")?;
    let msg_type = extract_cdata(xml, "MsgType")?;
    let content = extract_cdata(xml, "Content").unwrap_or_default();

    Some(WeChatMessage {
        from_user_name,
        msg_type,
        content,
    })
}

/// Extract a CDATA value for the given tag from XML.
///
/// Handles both `<tag><![CDATA[value]]></tag>` and `<tag>value</tag>` forms.
fn extract_cdata(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);

    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let raw = &xml[start..end];

    // Strip CDATA wrapper if present
    if let Some(inner) = raw
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
    {
        Some(inner.to_string())
    } else {
        Some(raw.trim().to_string())
    }
}

/// Handle the WeCom URL verification GET request.
///
/// WeCom sends a GET request with `echostr` (and optionally signature fields)
/// when registering a callback URL. We simply return the echostr value as-is
/// to confirm ownership. In production (with encryption enabled), the echostr
/// would need to be decrypted first — that requires AES-256-CBC with the
/// encoding_aes_key, which is left as a future enhancement.
async fn handle_verify(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyQuery>,
) -> Result<String, StatusCode> {
    // Log signature info for debugging (not verified in plain-text mode)
    if let (Some(sig), Some(ts), Some(nonce)) = (
        params.msg_signature.as_deref(),
        params.timestamp.as_deref(),
        params.nonce.as_deref(),
    ) {
        tracing::info!(
            channel = "wechat",
            sig = sig,
            ts = ts,
            nonce = nonce,
            "verification request"
        );
    }

    // In plain-text mode, WeCom sends the raw echostr and expects it back.
    // In safe/encrypted mode, echostr is encrypted; full AES decryption
    // is needed (future enhancement, requires encoding_aes_key).
    if state.token.is_some() {
        tracing::warn!(
            channel = "wechat",
            "token/signature verification is not yet implemented; returning echostr as-is (plain-text mode only)"
        );
    }

    match params.echostr {
        Some(echostr) if !echostr.is_empty() => Ok(echostr),
        _ => {
            tracing::error!(channel = "wechat", "verification request missing echostr");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Handle incoming WeCom callback POST.
async fn handle_callback(
    State(state): State<Arc<AppState>>,
    Query(_params): Query<CallbackQuery>,
    body: String,
) -> StatusCode {
    // Parse the XML body
    let msg = match parse_wecom_xml(&body) {
        Some(m) => m,
        None => {
            tracing::error!(channel = "wechat", body = %&body[..body.len().min(200)], "failed to parse XML body");
            return StatusCode::OK; // Always return 200 to WeCom
        }
    };

    // Only handle text messages
    if msg.msg_type != "text" {
        return StatusCode::OK;
    }

    let text = msg.content.trim().to_string();
    if text.is_empty() {
        return StatusCode::OK;
    }

    let sender_id = msg.from_user_name.clone();

    // Allowlist check — WeCom bots identify senders by user ID (FromUserName)
    if !state.allowlist.is_allowed(
        if sender_id.is_empty() {
            None
        } else {
            Some(&sender_id)
        },
        None,
    ) {
        tracing::warn!(channel = "wechat", user = %sender_id, "blocked message from unauthorized user");
        return StatusCode::OK; // Silently ignore
    }

    // Use sender ID as session key (WeCom bot webhook replies go to the group/chat,
    // not a specific user, so we maintain per-user conversation context)
    let session_key = format!("wechat:{}", sender_id);
    let webhook_key = state.webhook_key.clone();

    // Process in background so we respond to WeCom quickly (5s timeout requirement)
    let session = state.agent_session.clone();
    tokio::spawn(async move {
        let channel_info = ChannelInfo {
            platform: "wechat".into(),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(sender_id.clone()),
            ..Default::default()
        };
        let chat_info = ChatInfo {
            chat_type: "direct".into(),
            ..Default::default()
        };
        let mut msg = InboundMessage::channel(
            session_key.clone(),
            text.clone(),
            channel_info,
            sender_info,
            chat_info,
        );
        msg.finalize();
        match session.handle_message(msg, RunContext::default()).await {
            Ok(reply) => {
                let client = reqwest::Client::new();
                let webhook_url = format!("{}?key={}", WECOM_WEBHOOK_BASE, webhook_key);
                let chunks = formatter::format_for_channel(&reply.content, "wechat", 2048);
                for chunk in chunks {
                    let body = serde_json::json!({
                        "msgtype": "text",
                        "text": {
                            "content": chunk,
                        }
                    });

                    if let Err(e) = client.post(&webhook_url).json(&body).send().await {
                        tracing::error!(channel = "wechat", error = %e, "failed to send reply");
                    }
                }
            }
            Err(e) => {
                tracing::error!(channel = "wechat", error = %e, "handler error");
                // Attempt to send error back via webhook
                let client = reqwest::Client::new();
                let webhook_url = format!("{}?key={}", WECOM_WEBHOOK_BASE, webhook_key);
                let body = serde_json::json!({
                    "msgtype": "text",
                    "text": {
                        "content": format!("Error: {}", e),
                    }
                });
                let _ = client.post(&webhook_url).json(&body).send().await;
            }
        }
    });

    StatusCode::OK
}

/// Run the WeCom (WeChat Work) bot adapter.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wc_config = config
        .wechat
        .first()
        .ok_or("missing [[wechat]] section in config")?;

    let webhook_key = resolve_secret(
        wc_config.webhook_key.as_deref(),
        wc_config.webhook_key_env.as_deref(),
        "WeCom webhook key",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = wc_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true).with_channel("wechat"));

    let port = wc_config.port.unwrap_or(8076);

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "wechat",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    let state = Arc::new(AppState {
        agent_session,
        allowlist,
        webhook_key,
        token: wc_config.token.clone(),
    });

    let app = Router::new()
        .route("/callback", get(handle_verify).post(handle_callback))
        .with_state(state);

    tracing::info!(
        channel = "wechat",
        corp_id = %wc_config.corp_id.as_deref().unwrap_or("<not set>"),
        port = port,
        "adapter started"
    );

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`WeChatAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the WeChat Work (WeCom) webhook.
#[allow(dead_code)]
pub struct WeChatAdapter {
    webhook_key: String,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl WeChatAdapter {
    pub fn new(webhook_key: &str) -> Self {
        Self {
            webhook_key: webhook_key.to_string(),
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for WeChatAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "wechat".to_string(),
            name: "WeChat".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(20000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "wechat", "WeChatAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "wechat", "WeChatAdapter stopped");
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
impl Outbound for WeChatAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "wechat",
            to = %envelope.channel_id,
            "WeChatAdapter::send (placeholder)",
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for WeChatAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Probe the WeCom webhook endpoint with an empty POST; a non-5xx response
        // indicates the key is valid and the service is reachable.
        let url = format!("{}?key={}", WECOM_WEBHOOK_BASE, self.webhook_key);
        match self.client.get(&url).send().await {
            Ok(resp) if !resp.status().is_server_error() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("WeCom probe returned HTTP {}", resp.status());
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(msg)
            }
            Err(e) => {
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cdata_message() {
        let xml = r#"<xml>
<ToUserName><![CDATA[ww1234567890]]></ToUserName>
<FromUserName><![CDATA[user_abc]]></FromUserName>
<CreateTime>1234567890</CreateTime>
<MsgType><![CDATA[text]]></MsgType>
<Content><![CDATA[Hello, Synapse!]]></Content>
<MsgId>123456789</MsgId>
</xml>"#;

        let msg = parse_wecom_xml(xml).expect("should parse");
        assert_eq!(msg.from_user_name, "user_abc");
        assert_eq!(msg.msg_type, "text");
        assert_eq!(msg.content, "Hello, Synapse!");
    }

    #[test]
    fn parse_non_text_message() {
        let xml = r#"<xml>
<FromUserName><![CDATA[user_abc]]></FromUserName>
<MsgType><![CDATA[image]]></MsgType>
<PicUrl><![CDATA[http://example.com/img.jpg]]></PicUrl>
</xml>"#;

        let msg = parse_wecom_xml(xml).expect("should parse");
        assert_eq!(msg.msg_type, "image");
        // content defaults to empty for non-text types
        assert_eq!(msg.content, "");
    }

    #[test]
    fn extract_cdata_plain_text() {
        let xml = "<xml><Tag>plain value</Tag></xml>";
        assert_eq!(extract_cdata(xml, "Tag").as_deref(), Some("plain value"));
    }

    #[test]
    fn extract_cdata_missing_tag() {
        let xml = "<xml><Other><![CDATA[value]]></Other></xml>";
        assert_eq!(extract_cdata(xml, "Missing"), None);
    }
}
