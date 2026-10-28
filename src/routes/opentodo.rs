use axum::routing::{get, put};
use axum::Router;

use crate::handlers::opentodo;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/opentodo/status", get(opentodo::status))
        .route("/api/opentodo/check", get(opentodo::check))
        .route("/api/opentodo/token", put(opentodo::update_token))
}
