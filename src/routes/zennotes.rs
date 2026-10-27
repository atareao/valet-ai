use axum::routing::{get, put};
use axum::Router;

use crate::handlers::zennotes;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/zennotes/status", get(zennotes::status))
        .route("/api/zennotes/token", put(zennotes::update_token))
}
