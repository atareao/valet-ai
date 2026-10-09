use axum::routing::get;
use axum::Router;

use crate::handlers::stats;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/stats/llm/summary", get(stats::summary_handler))
        .route("/api/stats/llm/by-model", get(stats::by_model_handler))
        .route("/api/stats/llm/by-day", get(stats::by_day_handler))
        .route("/api/stats/llm/tools", get(stats::tools_handler))
        .route("/api/stats/db/sizes", get(stats::db_sizes_handler))
        .route("/api/stats/llm/export", get(stats::export_csv_handler))
        .route(
            "/api/stats/retention",
            get(stats::get_retention_handler).put(stats::set_retention_handler),
        )
        .route("/api/stats/memory", get(stats::memory_handler))
        .route("/api/stats/llm/last-call", get(stats::last_call_handler))
        .route("/api/stats/llm/background", get(stats::background_handler))
}
