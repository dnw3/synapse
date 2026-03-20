mod streaming_output;
mod types;
mod utils;
mod v3;

#[allow(unused_imports)]
pub use streaming_output::WsStreamingOutput;

use axum::{routing::get, Router};

use crate::gateway::state::AppState;

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(v3::ws_handler))
        .with_state(state)
}
