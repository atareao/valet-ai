use axum::routing::{get, put};
use axum::Router;

use crate::handlers::oximeal;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/oximeal/status", get(oximeal::status))
        .route("/api/oximeal/check", get(oximeal::check))
        .route("/api/oximeal/secret", put(oximeal::update_secret))
}
