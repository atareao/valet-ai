use axum::routing::{get, post};
use axum::Router;

use crate::handlers::strava;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/strava/authorize", get(strava::authorize))
        .route("/api/strava/callback", get(strava::callback))
        .route("/api/strava/status", get(strava::status))
        .route("/api/strava/check", get(strava::check))
        .route("/api/strava/disconnect", post(strava::disconnect))
}
