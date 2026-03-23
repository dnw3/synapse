use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::{SynapseConfig, WebChatBotConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

#[derive(Clone)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: crate::config::BotAllowlist,
    widget_title: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    session_id: String,
    message: String,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
    response: String,
    session_id: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Run the WebChat bot adapter — serves an HTTP API + embeddable widget.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wc_configs: Vec<crate::config::WebChatBotConfig> = config.channel_configs("webchat");
    let wc_config = wc_configs
        .first()
        .ok_or("missing [[channels.webchat]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = wc_config.port.unwrap_or(8090);
    let widget_title = wc_config
        .widget_title
        .clone()
        .unwrap_or_else(|| "Synapse Chat".to_string());

    let state = AppState {
        agent_session,
        allowlist: wc_config.allowlist.clone(),
        widget_title,
    };

    // Build CORS layer
    let cors = build_cors(&wc_config.allowed_origins);

    let app = Router::new()
        .route("/api/chat", post(handle_chat))
        .route("/widget.js", get(handle_widget_js))
        .route("/health", get(handle_health))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!(channel = "webchat", addr = %addr, "adapter started");
    tracing::info!(
        channel = "webchat",
        port = port,
        "embed widget: <script src=\"http://localhost:{}/widget.js\"></script>",
        port
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_cors(allowed_origins: &[String]) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    if allowed_origins.is_empty() {
        cors.allow_origin(tower_http::cors::Any)
    } else {
        let origins: Vec<HeaderValue> = allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors.allow_origin(origins)
    }
}

async fn handle_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Allowlist check
    if !state.allowlist.is_allowed(req.user_id.as_deref(), None) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!(ErrorResponse {
                error: "not allowed".to_string()
            })),
        );
    }

    let session_id = req.session_id.clone();
    let channel_info = ChannelInfo {
        platform: "webchat-bot".into(),
        ..Default::default()
    };
    let sender_info = SenderInfo {
        id: req.user_id.clone(),
        ..Default::default()
    };
    let chat_info = ChatInfo {
        chat_type: "direct".into(),
        ..Default::default()
    };
    let mut msg = InboundMessage::channel(
        session_id.clone(),
        req.message.clone(),
        channel_info,
        sender_info,
        chat_info,
    );
    msg.finalize();
    match state
        .agent_session
        .handle_message(msg, RunContext::default())
        .await
    {
        Ok(reply) => (
            StatusCode::OK,
            Json(serde_json::json!(ChatResponse {
                response: reply.content,
                session_id: req.session_id,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorResponse {
                error: format!("agent error: {}", e),
            })),
        ),
    }
}

async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

async fn handle_widget_js(State(state): State<AppState>) -> impl IntoResponse {
    let title = state.widget_title;
    let js = format!(
        r#"(function(){{
  if(document.getElementById('synapse-chat-widget'))return;
  var c=document.createElement('div');
  c.id='synapse-chat-widget';
  c.innerHTML='<div id="synapse-chat-btn" style="position:fixed;bottom:20px;right:20px;width:56px;height:56px;border-radius:50%;background:#4F46E5;color:#fff;display:flex;align-items:center;justify-content:center;cursor:pointer;box-shadow:0 4px 12px rgba(0,0,0,0.15);z-index:9999;font-size:24px;" onclick="window.__synapse_toggle()">💬</div><div id="synapse-chat-box" style="display:none;position:fixed;bottom:90px;right:20px;width:380px;height:520px;border-radius:12px;overflow:hidden;box-shadow:0 8px 30px rgba(0,0,0,0.12);z-index:9999;background:#fff;font-family:system-ui,sans-serif;"><div style="background:#4F46E5;color:#fff;padding:14px 16px;font-weight:600;">{title}</div><div id="synapse-messages" style="height:400px;overflow-y:auto;padding:12px;"></div><div style="padding:8px;border-top:1px solid #e5e7eb;display:flex;"><input id="synapse-input" type="text" placeholder="Type a message..." style="flex:1;padding:8px 12px;border:1px solid #d1d5db;border-radius:8px;outline:none;" onkeydown="if(event.key===\'Enter\')window.__synapse_send()"/><button onclick="window.__synapse_send()" style="margin-left:8px;padding:8px 16px;background:#4F46E5;color:#fff;border:none;border-radius:8px;cursor:pointer;">Send</button></div></div>';
  document.body.appendChild(c);
  var sid='webchat_'+Math.random().toString(36).substr(2,9);
  window.__synapse_toggle=function(){{var b=document.getElementById('synapse-chat-box');b.style.display=b.style.display==='none'?'flex':'none';b.style.flexDirection='column';}};
  window.__synapse_send=function(){{var i=document.getElementById('synapse-input');var m=i.value.trim();if(!m)return;i.value='';var msgs=document.getElementById('synapse-messages');msgs.innerHTML+='<div style="margin:4px 0;text-align:right;"><span style="background:#4F46E5;color:#fff;padding:6px 12px;border-radius:12px;display:inline-block;max-width:80%;text-align:left;">'+m+'</span></div>';msgs.scrollTop=msgs.scrollHeight;fetch('/api/chat',{{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{session_id:sid,message:m}})}}).then(r=>r.json()).then(d=>{{msgs.innerHTML+='<div style="margin:4px 0;"><span style="background:#f3f4f6;padding:6px 12px;border-radius:12px;display:inline-block;max-width:80%;">'+(d.response||d.error)+'</span></div>';msgs.scrollTop=msgs.scrollHeight;}}).catch(e=>{{msgs.innerHTML+='<div style="margin:4px 0;color:red;">Error: '+e+'</div>';msgs.scrollTop=msgs.scrollHeight;}});}};
}})();"#,
        title = title
    );
    (
        StatusCode::OK,
        [("content-type", "application/javascript; charset=utf-8")],
        js,
    )
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`WebChatAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the embeddable WebChat widget.
#[allow(dead_code)]
pub struct WebChatAdapter {
    client: reqwest::Client,
    /// Port the HTTP server is listening on (for health probing).
    port: u16,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl WebChatAdapter {
    pub fn new(port: u16) -> Self {
        Self {
            client: reqwest::Client::new(),
            port,
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for WebChatAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "webchat".to_string(),
            name: "WebChat".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(65536),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "webchat", "WebChatAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "webchat", "WebChatAdapter stopped");
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
impl Outbound for WebChatAdapter {
    async fn send(
        &self,
        _envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // WebChat replies are delivered synchronously in the HTTP response body
        // of /api/chat — no separate outbound push is required.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for WebChatAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("http://127.0.0.1:{}/health", self.port);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("/health returned HTTP {}", resp.status());
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
