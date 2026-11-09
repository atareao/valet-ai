use axum::routing::{get, put};
use axum::Router;

use crate::handlers::blog;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/blog/status", get(blog::status))
        .route("/api/blog/check", get(blog::check))
        .route("/api/blog/password", put(blog::update_password))
}
