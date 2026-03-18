use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

use crate::config::bot::LarkCardConfig;
use crate::gateway::state::AppState;

/// GET /api/config/lark-card — returns current card config
pub async fn get_lark_card_config(State(state): State<AppState>) -> Json<LarkCardConfig> {
    let card_config = state
        .config
        .lark
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
#[cfg(feature = "bot-lark")]
pub async fn preview_lark_card(
    State(state): State<AppState>,
    Json(body): Json<PreviewRequest>,
) -> Json<Value> {
    use crate::channels::card_builder::assemble_final_card;
    use synaptic::core::message_ir::{parse_markdown, RenderOptions, RenderTarget};
    use synaptic::lark::card_elements::render_lark_card_elements;

    let card_config = state
        .config
        .lark
        .first()
        .map(|lark| lark.card.clone())
        .unwrap_or_default();
    let ir = parse_markdown(&body.sample_text);
    let options = RenderOptions::new(RenderTarget::LarkCard);
    let elements = render_lark_card_elements(&ir, &options);
    let bot_name = if card_config.header_title.is_empty() {
        "Synapse"
    } else {
        &card_config.header_title
    };
    let card = assemble_final_card(elements, &card_config, bot_name);
    Json(card)
}

#[derive(serde::Deserialize)]
pub struct PreviewRequest {
    pub sample_text: String,
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
