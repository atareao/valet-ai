mod common;
use common::TestApp;

use axum::http::StatusCode;
use serde_json::Value;

/// Helper: create a TestApp with no seed data and return it alongside a
/// writable database pool so tests can insert LLM request rows directly.
async fn setup() -> (sqlx::SqlitePool, TestApp) {
    let state = valet::AppState::new_in_memory_empty().await;
    let db = state.db.clone();
    let router = valet::app_with_state(state);
    (db.clone(), TestApp { router, db })
}

// ── 1. GET /api/stats/llm/summary — sin datos ─────────────────────────────

#[tokio::test]
async fn test_summary_empty() {
    // Given no LLM request data
    // When GET /api/stats/llm/summary is called
    // Then returns 200 with total_calls=0, total_cost=0.0
    let (_db, app) = setup().await;

    let resp = app.get("/api/stats/llm/summary").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    assert_eq!(body["total_calls"], 0);
    assert_eq!(body["total_cost"], 0.0);
    assert_eq!(body["total_tokens"], 0);
    assert_eq!(body["total_errors"], 0);
    assert!(body["avg_duration_ms"].is_null());
}

// ── 2. GET /api/stats/llm/summary — con datos ─────────────────────────────

#[tokio::test]
async fn test_summary_with_data() {
    // Given 3 LLM request rows inserted (2 success, 1 error)
    // When GET /api/stats/llm/summary is called
    // Then returns correct aggregate values
    let (db, app) = setup().await;

    // Seed a profile for FK
    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    // Two successful calls
    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
    )
    .bind("stats-req-1")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(100i64)
    .bind(50i64)
    .bind(150i64)
    .bind(10i64)
    .bind(5i64)
    .bind(0.01)
    .bind(200i64)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
    )
    .bind("stats-req-2")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(200i64)
    .bind(100i64)
    .bind(300i64)
    .bind(20i64)
    .bind(10i64)
    .bind(0.02)
    .bind(300i64)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    // One error
    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, error_message, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))",
    )
    .bind("stats-req-3")
    .bind("claude-3")
    .bind("prof-stats")
    .bind(0i64)
    .bind(0i64)
    .bind(0i64)
    .bind(0i64)
    .bind(0i64)
    .bind(0.0)
    .bind(Option::<i64>::None)
    .bind("error")
    .bind("timeout")
    .execute(&db)
    .await
    .unwrap();

    let resp = app.get("/api/stats/llm/summary").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    assert_eq!(body["total_calls"], 3);
    assert_eq!(body["total_prompt_tokens"], 300);
    assert_eq!(body["total_completion_tokens"], 150);
    assert_eq!(body["total_tokens"], 450);
    assert_eq!(body["total_cached_tokens"], 30);
    assert_eq!(body["total_reasoning_tokens"], 15);
    assert!((body["total_cost"].as_f64().unwrap() - 0.03).abs() < f64::EPSILON);
    assert_eq!(body["total_errors"], 1);
    // avg_duration_ms = (200 + 300 + NULL) / 2 = 250.0
    let avg = body["avg_duration_ms"].as_f64().unwrap();
    assert!((avg - 250.0).abs() < f64::EPSILON);
}

// ── 3. GET /api/stats/llm/by-model — con datos ────────────────────────────

#[tokio::test]
async fn test_by_model() {
    // Given 2 calls to gpt-4o and 1 call to claude-3
    // When GET /api/stats/llm/by-model is called
    // Then returns 2 rows ordered by cost descending
    let (db, app) = setup().await;

    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
    )
    .bind("m1")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(100i64)
    .bind(50i64)
    .bind(150i64)
    .bind(10i64)
    .bind(5i64)
    .bind(0.02)
    .bind(200i64)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
    )
    .bind("m2")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(50i64)
    .bind(25i64)
    .bind(75i64)
    .bind(5i64)
    .bind(3i64)
    .bind(0.01)
    .bind(100i64)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
    )
    .bind("m3")
    .bind("claude-3")
    .bind("prof-stats")
    .bind(200i64)
    .bind(100i64)
    .bind(300i64)
    .bind(20i64)
    .bind(10i64)
    .bind(0.04)
    .bind(400i64)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    let resp = app.get("/api/stats/llm/by-model").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    let models = body.as_array().unwrap();
    assert_eq!(models.len(), 2);

    // Ordered by total_cost DESC: claude-3 (0.04) first, gpt-4o (0.03) second
    assert_eq!(models[0]["model"], "claude-3");
    assert_eq!(models[0]["calls"], 1);
    assert!((models[0]["total_cost"].as_f64().unwrap() - 0.04).abs() < f64::EPSILON);

    assert_eq!(models[1]["model"], "gpt-4o");
    assert_eq!(models[1]["calls"], 2);
    assert!((models[1]["total_cost"].as_f64().unwrap() - 0.03).abs() < f64::EPSILON);
}

// ── 4. GET /api/stats/llm/by-day?days=30 ──────────────────────────────────

#[tokio::test]
async fn test_by_day() {
    // Given requests inserted over several days
    // When GET /api/stats/llm/by-day?days=30 is called
    // Then returns a time series with correct daily breakdown
    let (db, app) = setup().await;

    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    // Insert one request per day for 5 days (Sep 21-25)
    for i in 1..=5 {
        let date = format!("2026-09-{:02}T10:00:00", 20 + i);
        let id = format!("day-{i}");
        sqlx::query(
            "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )
        .bind(&id)
        .bind("gpt-4o")
        .bind("prof-stats")
        .bind(100i64)
        .bind(50i64)
        .bind(150i64)
        .bind(10i64)
        .bind(5i64)
        .bind(0.01)
        .bind(100i64)
        .bind("success")
        .bind(&date)
        .execute(&db)
        .await
        .unwrap();
    }

    let resp = app.get("/api/stats/llm/by-day?days=30").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    let days = body.as_array().unwrap();
    assert_eq!(days.len(), 5);

    // Dates should be in ascending order
    for i in 0..days.len() - 1 {
        assert!(days[i]["date"].as_str().unwrap() <= days[i + 1]["date"].as_str().unwrap());
    }

    // Each day should have 1 call
    for day in days {
        assert_eq!(day["calls"], 1);
        assert_eq!(day["total_tokens"], 150);
    }
}

// ── 5. GET /api/stats/llm/tools ───────────────────────────────────────────

#[tokio::test]
async fn test_tools_summary() {
    // Given requests with tool_calls
    // When GET /api/stats/llm/tools is called
    // Then returns tool frequencies sorted by count descending
    let (db, app) = setup().await;

    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    // 3 calls with get_weather
    for i in 0..3 {
        sqlx::query(
            "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cost, status, tool_calls, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
        )
        .bind(format!("tw-{i}"))
        .bind("gpt-4o")
        .bind("prof-stats")
        .bind(100i64)
        .bind(50i64)
        .bind(150i64)
        .bind(0.01)
        .bind("success")
        .bind(r#"[{"name":"get_weather"}]"#)
        .execute(&db)
        .await
        .unwrap();
    }

    // 2 calls with search_web
    for i in 0..2 {
        sqlx::query(
            "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cost, status, tool_calls, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
        )
        .bind(format!("ts-{i}"))
        .bind("gpt-4o")
        .bind("prof-stats")
        .bind(100i64)
        .bind(50i64)
        .bind(150i64)
        .bind(0.01)
        .bind("success")
        .bind(r#"[{"name":"search_web"}]"#)
        .execute(&db)
        .await
        .unwrap();
    }

    // 1 null tool_calls (should be ignored)
    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cost, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
    )
    .bind("t-null")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(100i64)
    .bind(50i64)
    .bind(150i64)
    .bind(0.01)
    .bind("success")
    .execute(&db)
    .await
    .unwrap();

    let resp = app.get("/api/stats/llm/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    let tools = body.as_array().unwrap();
    assert_eq!(tools.len(), 2);

    // Sorted by count desc: get_weather=3, search_web=2
    assert_eq!(tools[0]["tool"], "get_weather");
    assert_eq!(tools[0]["count"], 3);
    assert_eq!(tools[1]["tool"], "search_web");
    assert_eq!(tools[1]["count"], 2);
}

// ── 6. GET /api/stats/db/sizes ────────────────────────────────────────────

#[tokio::test]
async fn test_db_sizes() {
    // Given tables with data
    // When GET /api/stats/db/sizes is called
    // Then returns row counts for all tables, some > 0
    let (db, app) = setup().await;

    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    sqlx::query("INSERT INTO messages (id, role, content) VALUES ('sm1', 'user', 'Hello')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO messages (id, role, content) VALUES ('sm2', 'assistant', 'Hi')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO memory (id, content, tokens_count, created_at, metadata) VALUES ('smem1', 'A memory', 0, datetime('now'), '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    let resp = app.get("/api/stats/db/sizes").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    let tables = body.as_array().unwrap();

    // Should include all domain tables
    let messages_size = tables.iter().find(|t| t["table"] == "messages").unwrap();
    assert_eq!(messages_size["rows"], 2);

    let memory_size = tables.iter().find(|t| t["table"] == "memory").unwrap();
    assert_eq!(memory_size["rows"], 1);

    let profiles_size = tables.iter().find(|t| t["table"] == "profiles").unwrap();
    assert_eq!(profiles_size["rows"], 1);
}

// ── 7. GET /api/stats/llm/export — CSV ────────────────────────────────────

#[tokio::test]
async fn test_export_csv() {
    // Given LLM request data
    // When GET /api/stats/llm/export is called
    // Then returns CSV with header and data rows
    let (db, app) = setup().await;

    sqlx::query(
        "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('prof-stats', 'StatsTest', '{}')",
    )
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, tool_calls, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )
    .bind("csv-1")
    .bind("gpt-4o")
    .bind("prof-stats")
    .bind(100i64)
    .bind(50i64)
    .bind(150i64)
    .bind(10i64)
    .bind(5i64)
    .bind(0.01)
    .bind(200i64)
    .bind("success")
    .bind(r#"[{"name":"get_weather"}]"#)
    .bind("2026-09-24T10:00:00")
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, error_message, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )
    .bind("csv-2")
    .bind("claude-3")
    .bind("prof-stats")
    .bind(200i64)
    .bind(100i64)
    .bind(300i64)
    .bind(20i64)
    .bind(10i64)
    .bind(0.02)
    .bind(Option::<i64>::None)
    .bind("error")
    .bind("timeout")
    .bind("2026-09-25T12:00:00")
    .execute(&db)
    .await
    .unwrap();

    let resp = app.get("/api/stats/llm/export").await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Check content-type header
    let content_type = resp.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("text/csv"));

    // Check content-disposition header
    let disposition = resp.headers().get("content-disposition").unwrap();
    assert!(disposition
        .to_str()
        .unwrap()
        .contains("valet-llm-requests.csv"));

    // Read the body as text (CSV)
    let csv_text = resp.text().await;
    let lines: Vec<&str> = csv_text.trim().lines().collect();
    assert!(
        lines.len() >= 3,
        "expected header + 2 rows, got {}",
        lines.len()
    );

    let header = lines[0];
    assert!(header.starts_with("id,"));
    assert!(header.contains("model"));
    assert!(header.contains("status"));

    let row1 = lines[1];
    assert!(row1.starts_with("csv-1,"));
    assert!(row1.contains("gpt-4o"));

    let row2 = lines[2];
    assert!(row2.starts_with("csv-2,"));
    assert!(row2.contains("claude-3"));
    assert!(row2.contains("timeout"));
}

// ── 8. GET /api/stats/retention — default 30 ──────────────────────────────

#[tokio::test]
async fn test_get_retention_default() {
    // Given no retention setting in the database
    // When GET /api/stats/retention is called
    // Then returns 200 with days=30
    let (_db, app) = setup().await;

    let resp = app.get("/api/stats/retention").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    assert_eq!(body["days"], 30);
}

// ── 9. PUT /api/stats/retention + GET verify ──────────────────────────────

#[tokio::test]
async fn test_set_and_get_retention() {
    // Given no retention configured
    // When PUT /api/stats/retention with {"days": 45}
    // Then returns 200 with the saved value
    // And GET /api/stats/retention returns days=45
    let (_db, app) = setup().await;

    let put_resp = app
        .put("/api/stats/retention")
        .json(&serde_json::json!({"days": 45}))
        .send()
        .await;
    assert_eq!(put_resp.status(), StatusCode::OK);
    let put_body: Value = put_resp.json().await;
    assert_eq!(put_body["days"], 45);

    // GET should return the confirmed value
    let get_resp = app.get("/api/stats/retention").await;
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body: Value = get_resp.json().await;
    assert_eq!(get_body["days"], 45);
}

// ── 10. GET /api/stats/llm/background ─────────────────────────────────────

#[tokio::test]
async fn test_background_summary() {
    // Given no background LLM request data
    // When GET /api/stats/llm/background is called
    // Then returns 200 with one entry per non-chat origin
    let (_db, app) = setup().await;

    let resp = app.get("/api/stats/llm/background").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: Value = resp.json().await;
    let origins = body
        .as_array()
        .expect("the background summary must be a JSON array");
    assert_eq!(
        origins.len(),
        4,
        "one entry per non-chat origin (router, archivist, consolidator, collapse)"
    );

    let mut kinds: Vec<&str> = origins
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        vec!["archivist", "collapse", "consolidator", "router"],
        "every non-chat origin must appear"
    );
}
