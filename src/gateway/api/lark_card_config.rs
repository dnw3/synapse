use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

use crate::config::bots::LarkCardConfig;
use crate::gateway::state::AppState;

/// GET /api/config/lark-card — returns current card config
pub async fn get_lark_card_config(State(state): State<AppState>) -> Json<LarkCardConfig> {
    let lark_configs: Vec<crate::config::LarkBotConfig> = state.core.config.channel_configs("lark");
    let card_config = lark_configs
        .first()
        .map(|lark| lark.card.clone())
        .unwrap_or_default();
    Json(card_config)
}

/// PUT /api/config/lark-card — updates card config (persists to file)
pub async fn update_lark_card_config(
    State(_state): State<AppState>,
    Json(card_config): Json<LarkCardConfig>,
) -> Json<LarkCardConfig> {
    // Persist to ~/.synapse/lark_card_config.json
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse")
        .join("lark_card_config.json");
    if let Ok(json) = serde_json::to_string_pretty(&card_config) {
        let _ = tokio::fs::write(&config_path, json).await;
    }
    Json(card_config)
}

/// POST /api/config/lark-card/preview — preview a card with sample text
///
/// Accepts an optional inline card config so the preview reflects unsaved changes.
/// Falls back to the server's current config when fields are omitted.
#[cfg(feature = "bot-lark")]
pub async fn preview_lark_card(
    State(state): State<AppState>,
    Json(body): Json<PreviewRequest>,
) -> Json<Value> {
    use synaptic::core::message_ir::{parse_markdown, RenderOptions, RenderTarget};
    use synaptic::lark::card_elements::render_lark_card_elements;

    // Use inline config from request body, fall back to server config.
    let card_config = body.config.unwrap_or_else(|| {
        let lark_configs: Vec<crate::config::LarkBotConfig> =
            state.core.config.channel_configs("lark");
        lark_configs
            .first()
            .map(|lark| lark.card.clone())
            .unwrap_or_default()
    });
    let ir = parse_markdown(&body.sample_text);
    let options = RenderOptions::new(RenderTarget::LarkCard);
    let elements = render_lark_card_elements(&ir, &options);
    let title = if card_config.header_title.is_empty() {
        "Synapse"
    } else {
        &card_config.header_title
    };
    let framework_config = synaptic::lark::CardConfig {
        header_title: title.to_string(),
        template: card_config.template.clone(),
        header_icon: card_config.header_icon.clone(),
    };
    let card = synaptic::lark::assemble_card(elements, &framework_config);
    Json(card)
}

#[derive(serde::Deserialize)]
pub struct PreviewRequest {
    pub sample_text: String,
    /// Optional inline card config for previewing unsaved changes.
    pub config: Option<LarkCardConfig>,
}

pub fn routes() -> Router<AppState> {
    let router = Router::new().route(
        "/config/lark-card",
        get(get_lark_card_config).put(update_lark_card_config),
    );

    #[cfg(feature = "bot-lark")]
    let router = router.route("/config/lark-card/preview", post(preview_lark_card));

    router
}
