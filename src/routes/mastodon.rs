use axum::routing::{get, put};
use axum::Router;

use crate::handlers::mastodon;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/mastodon/status", get(mastodon::status))
        .route("/api/mastodon/check", get(mastodon::check))
        .route("/api/mastodon/token", put(mastodon::update_token))
}
