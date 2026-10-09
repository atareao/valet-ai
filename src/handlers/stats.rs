use axum::body::Body;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::repos::stats::StatsRepo;
use crate::errors::AppError;
use crate::models::stats::{
    BackgroundStats, DayStats, LastApiCall, MemoryStats, ModelStats, StatsSummary, TableSize,
    ToolStats,
};
use crate::AppState;

// ── Request / Response types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DayParams {
    pub days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct RetentionResponse {
    pub days: u32,
}

#[derive(Debug, Deserialize)]
pub struct RetentionRequest {
    pub days: u32,
}

/// A CSV string that is returned with the correct Content-Type and
/// Content-Disposition headers so the browser offers it as a download.
pub struct CsvResponse(pub String);

impl IntoResponse for CsvResponse {
    fn into_response(self) -> Response {
        Response::builder()
            .header("Content-Type", "text/csv; charset=utf-8")
            .header(
                "Content-Disposition",
                "attachment; filename=\"valet-llm-requests.csv\"",
            )
            .body(Body::from(self.0))
            .unwrap()
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// Global aggregate summary over all LLM requests.
pub async fn summary_handler(
    State(state): State<AppState>,
) -> Result<Json<StatsSummary>, AppError> {
    let summary = StatsRepo::summary(&state.db).await?;
    Ok(Json(summary))
}

/// Per-model breakdown of LLM usage, ordered by total cost descending.
pub async fn by_model_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<ModelStats>>, AppError> {
    let stats = StatsRepo::by_model(&state.db).await?;
    Ok(Json(stats))
}

/// Daily time-series for the last N days (default 30).
pub async fn by_day_handler(
    State(state): State<AppState>,
    Query(params): Query<DayParams>,
) -> Result<Json<Vec<DayStats>>, AppError> {
    let days = params.days.unwrap_or(30);
    let stats = StatsRepo::by_day(&state.db, days).await?;
    Ok(Json(stats))
}

/// Frequency of tool calls across all LLM requests.
pub async fn tools_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<ToolStats>>, AppError> {
    let stats = StatsRepo::tools_summary(&state.db).await?;
    Ok(Json(stats))
}

/// Row counts for all domain database tables.
pub async fn db_sizes_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<TableSize>>, AppError> {
    let sizes = StatsRepo::db_sizes(&state.db).await?;
    Ok(Json(sizes))
}

/// Export all LLM request records as a downloadable CSV file.
pub async fn export_csv_handler(State(state): State<AppState>) -> Result<CsvResponse, AppError> {
    let csv = StatsRepo::export_csv(&state.db).await?;
    Ok(CsvResponse(csv))
}

/// Read the current retention-days setting (defaults to 30).
pub async fn get_retention_handler(
    State(state): State<AppState>,
) -> Result<Json<RetentionResponse>, AppError> {
    let days = StatsRepo::get_retention_days(&state.db).await?;
    Ok(Json(RetentionResponse { days }))
}

/// Save a new retention-days value.
pub async fn set_retention_handler(
    State(state): State<AppState>,
    Json(body): Json<RetentionRequest>,
) -> Result<Json<Value>, AppError> {
    StatsRepo::set_retention_days(&state.db, body.days).await?;
    Ok(Json(serde_json::json!({ "days": body.days })))
}

/// Aggregate statistics over episodic memory.
pub async fn memory_handler(State(state): State<AppState>) -> Result<Json<MemoryStats>, AppError> {
    let stats = StatsRepo::memory_summary(&state.db).await?;
    Ok(Json(stats))
}

/// Per-origin aggregate of background LLM usage (router, archivist,
/// consolidator, collapse), one entry per origin.
pub async fn background_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<BackgroundStats>>, AppError> {
    let stats = StatsRepo::background_summary(&state.db).await?;
    Ok(Json(stats))
}

/// Return the most recent OpenRouter API call stored in-memory.
///
/// Returns `None` if no call has been recorded yet.
pub async fn last_call_handler(
    State(state): State<AppState>,
) -> Result<Json<Option<LastApiCall>>, AppError> {
    let data = state.last_api_call.read().unwrap();
    Ok(Json(data.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    use crate::AppState;

    /// Helper: build an in-memory AppState with migrations applied and a
    /// default profile inserted (needed for FK constraints on llm_requests).
    async fn setup_state() -> AppState {
        let state = AppState::new_in_memory_empty().await;
        // Seed a profile for FK references
        sqlx::query(
            "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-hdl', 'HandlerTest', '{}')",
        )
        .execute(&state.db)
        .await
        .unwrap();
        state
    }

    /// Convenience: insert a single LLM request row.
    #[allow(clippy::too_many_arguments)]
    async fn insert_request(
        db: &SqlitePool,
        id: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        cached_tokens: i64,
        reasoning_tokens: i64,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        error_message: Option<&str>,
        tool_calls: Option<&str>,
        created_at: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO llm_requests (id, model, provider, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, is_byok, duration_ms, cache_hit, status, error_message, tool_calls, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, COALESCE(?17, datetime('now')))",
        )
        .bind(id)
        .bind(model)
        .bind(Option::<String>::None) // provider
        .bind("prof-hdl")
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(cached_tokens)
        .bind(reasoning_tokens)
        .bind(cost)
        .bind(0i64) // is_byok
        .bind(duration_ms)
        .bind(0i64) // cache_hit
        .bind(status)
        .bind(error_message)
        .bind(tool_calls)
        .bind(created_at)
        .execute(db)
        .await
        .unwrap();
    }

    // ── summary_handler ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_summary_handler_empty() {
        let state = setup_state().await;
        let res = summary_handler(State(state)).await.unwrap();
        assert_eq!(res.0.total_calls, 0);
        assert_eq!(res.0.total_cost, 0.0);
        assert!(res.0.avg_duration_ms.is_none());
    }

    #[tokio::test]
    async fn test_summary_handler_with_data() {
        let state = setup_state().await;

        insert_request(
            &state.db,
            "h1",
            "gpt-4o",
            100,
            50,
            150,
            10,
            5,
            0.01,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &state.db,
            "h2",
            "gpt-4o",
            200,
            100,
            300,
            20,
            10,
            0.02,
            Some(300),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &state.db,
            "h3",
            "claude-3",
            0,
            0,
            0,
            0,
            0,
            0.0,
            None,
            "error",
            Some("timeout"),
            None,
            None,
        )
        .await;

        let res = summary_handler(State(state)).await.unwrap();
        assert_eq!(res.0.total_calls, 3);
        assert_eq!(res.0.total_tokens, 450);
        assert!((res.0.total_cost - 0.03).abs() < f64::EPSILON);
        assert_eq!(res.0.total_errors, 1);
        assert!((res.0.avg_duration_ms.unwrap() - 250.0).abs() < f64::EPSILON);
    }

    // ── by_model_handler ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_by_model_handler() {
        let state = setup_state().await;

        insert_request(
            &state.db,
            "m1",
            "gpt-4o",
            100,
            50,
            150,
            10,
            5,
            0.02,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &state.db,
            "m2",
            "gpt-4o",
            50,
            25,
            75,
            5,
            3,
            0.01,
            Some(100),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &state.db,
            "m3",
            "claude-3",
            200,
            100,
            300,
            20,
            10,
            0.04,
            Some(400),
            "success",
            None,
            None,
            None,
        )
        .await;

        let res = by_model_handler(State(state)).await.unwrap();
        assert_eq!(res.0.len(), 2);
        assert_eq!(res.0[0].model, "claude-3");
        assert_eq!(res.0[0].calls, 1);
        assert_eq!(res.0[1].model, "gpt-4o");
        assert_eq!(res.0[1].calls, 2);
    }

    // ── by_day_handler ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_by_day_handler() {
        let state = setup_state().await;

        for i in 1..=5 {
            let date = format!("2026-09-{:02}T10:00:00", 20 + i);
            let id = format!("dh-{i}");
            insert_request(
                &state.db,
                &id,
                "gpt-4o",
                100,
                50,
                150,
                10,
                5,
                0.01,
                Some(100),
                "success",
                None,
                None,
                Some(&date),
            )
            .await;
        }

        let params = DayParams { days: Some(30) };
        let res = by_day_handler(State(state), Query(params)).await.unwrap();
        assert_eq!(res.0.len(), 5);
        for i in 0..res.0.len() - 1 {
            assert!(res.0[i].date <= res.0[i + 1].date);
        }
    }

    // ── tools_handler ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_tools_handler() {
        let state = setup_state().await;

        for i in 0..3 {
            let id = format!("th-{i}");
            insert_request(
                &state.db,
                &id,
                "gpt-4o",
                100,
                50,
                150,
                0,
                0,
                0.01,
                None,
                "success",
                None,
                Some(r#"[{"name":"get_weather"}]"#),
                None,
            )
            .await;
        }
        for i in 0..2 {
            let id = format!("ts-{i}");
            insert_request(
                &state.db,
                &id,
                "gpt-4o",
                100,
                50,
                150,
                0,
                0,
                0.01,
                None,
                "success",
                None,
                Some(r#"[{"name":"search_web"}]"#),
                None,
            )
            .await;
        }

        let res = tools_handler(State(state)).await.unwrap();
        assert_eq!(res.0.len(), 2);
        assert_eq!(res.0[0].tool, "get_weather");
        assert_eq!(res.0[0].count, 3);
        assert_eq!(res.0[1].tool, "search_web");
        assert_eq!(res.0[1].count, 2);
    }

    // ── get_retention_handler / set_retention_handler ───────────────────────

    #[tokio::test]
    async fn test_get_retention_default() {
        let state = setup_state().await;
        let res = get_retention_handler(State(state)).await.unwrap();
        assert_eq!(res.0.days, 30);
    }

    #[tokio::test]
    async fn test_set_and_get_retention() {
        let state = setup_state().await;
        let req = RetentionRequest { days: 60 };
        let res = set_retention_handler(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert_eq!(res.0["days"], 60);

        let get_res = get_retention_handler(State(state)).await.unwrap();
        assert_eq!(get_res.0.days, 60);
    }

    // ── memory_handler ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_memory_handler_empty() {
        let state = setup_state().await;
        let res = memory_handler(State(state)).await.unwrap();
        assert_eq!(res.0.total_memories, 0);
        assert_eq!(res.0.total_tokens, 0);
        assert_eq!(res.0.messages_indexed, 0);
        assert_eq!(res.0.messages_total, 0);
    }

    #[tokio::test]
    async fn test_memory_handler_with_data() {
        let state = setup_state().await;

        sqlx::query("INSERT INTO memory (id, content, tokens_count) VALUES ('hm1', 'mem1', 100)")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memory (id, content, tokens_count) VALUES ('hm2', 'mem2', 200)")
            .execute(&state.db)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('hmsg1', 'user', 'hello', 1)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('hmsg2', 'assistant', 'hi', 0)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('hmsg3', 'user', 'howdy', 1)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let res = memory_handler(State(state)).await.unwrap();
        assert_eq!(res.0.total_memories, 2);
        assert_eq!(res.0.total_tokens, 300);
        assert_eq!(res.0.messages_indexed, 2);
        assert_eq!(res.0.messages_total, 3);
    }

    // ── last_call_handler ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_last_call_handler_none() {
        let state = setup_state().await;
        let res = last_call_handler(State(state)).await.unwrap();
        assert!(res.0.is_none(), "Expected None when no last call stored");
    }

    #[tokio::test]
    async fn test_last_call_handler_with_data() {
        let state = setup_state().await;
        // Manually set the last_api_call
        let last = crate::models::stats::LastApiCall {
            model: "gpt-4o".into(),
            request_body: Some(r#"{"model":"gpt-4o"}"#.into()),
            response_body: Some(r#"{"choices":[{"message":{"content":"Hello"}}]}"#.into()),
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_tokens: 10,
            reasoning_tokens: 5,
            cost: 0.01,
            duration_ms: Some(200),
            status: "success".into(),
            error_message: None,
            tool_calls: None,
            created_at: "2026-09-27T10:00:00Z".into(),
        };
        *state.last_api_call.write().unwrap() = Some(last);

        let res = last_call_handler(State(state)).await.unwrap();
        let data = res.0.expect("Expected Some LastApiCall");
        assert_eq!(data.model, "gpt-4o");
        assert_eq!(data.request_body, Some(r#"{"model":"gpt-4o"}"#.into()));
        assert_eq!(data.total_tokens, 150);
        assert_eq!(data.cost, 0.01);
        assert_eq!(data.status, "success");
    }

    /// Integration test via the full HTTP router to prove the Arc is shared.
    #[tokio::test]
    async fn test_last_call_via_http_router() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let state = setup_state().await;

        // Save data into state (same as what Orchestrator::save_last_call does)
        let last = crate::models::stats::LastApiCall {
            model: "router-test-model".into(),
            request_body: Some(r#"{"model":"router-test"}"#.into()),
            response_body: Some(r#"{"choices":[{"message":{"content":"Hi"}}]}"#.into()),
            prompt_tokens: 50,
            completion_tokens: 25,
            total_tokens: 75,
            cached_tokens: 5,
            reasoning_tokens: 3,
            cost: 0.005,
            duration_ms: Some(150),
            status: "success".into(),
            error_message: None,
            tool_calls: None,
            created_at: "2026-09-27T12:00:00Z".into(),
        };
        *state.last_api_call.write().unwrap() = Some(last);

        // Build the full router and issue an HTTP GET
        let app = crate::app_with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/stats/llm/last-call")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Expected 200 OK, got {}",
            response.status()
        );

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body_str).unwrap();

        assert!(!parsed.is_null(), "Response body should not be null");
        assert_eq!(parsed["model"], "router-test-model");
        assert_eq!(parsed["total_tokens"], 75);
        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["request_body"], r#"{"model":"router-test"}"#);
        assert_eq!(
            parsed["response_body"],
            r#"{"choices":[{"message":{"content":"Hi"}}]}"#
        );
    }
}
