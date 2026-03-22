pub mod dashboard;
pub mod files;
pub mod lark_callback;
pub mod lark_card_config;
pub mod logs;
pub mod traces;
pub mod upload;

use axum::Router;

use super::state::AppState;

/// Create the REST API router.
pub fn create_router(state: AppState) -> Router {
    Router::new().nest("/api", api_routes()).with_state(state)
}

fn api_routes() -> Router<AppState> {
    Router::new()
        .merge(files::routes())
        .merge(dashboard::routes())
        .merge(logs::routes())
        .merge(traces::routes())
        .merge(upload::routes())
        .merge(lark_callback::routes())
        .merge(lark_card_config::routes())
}
