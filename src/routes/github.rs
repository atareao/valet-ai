use axum::routing::{get, put};
use axum::Router;

use crate::handlers::github;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/github/status", get(github::status))
        .route("/api/github/check", get(github::check))
        .route("/api/github/token", put(github::update_token))
}
