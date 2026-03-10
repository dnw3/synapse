pub mod conversations;
pub mod dashboard;
pub mod files;
pub mod logs;
pub mod messages;
pub mod upload;

use axum::Router;

use super::state::AppState;

/// Create the REST API router.
pub fn create_router(state: AppState) -> Router {
    Router::new().nest("/api", api_routes()).with_state(state)
}

fn api_routes() -> Router<AppState> {
    Router::new()
        .merge(conversations::routes())
        .merge(messages::routes())
        .merge(files::routes())
        .merge(dashboard::routes())
        .merge(logs::routes())
        .merge(upload::routes())
}
