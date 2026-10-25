mod common;
use common::TestApp;

use axum::http::StatusCode;

#[tokio::test]
async fn test_list_events_in_range() {
    // Given an event exists with start_time="2026-09-25T10:00:00Z"
    // When GET /api/events?start=...&end=... is called
    // Then returns 200 with an array containing that event
    let app = TestApp::new_empty().await;

    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Test Event",
            "start_time": "2026-09-25T10:00:00Z",
            "end_time": "2026-09-25T11:00:00Z"
        }))
        .send()
        .await;

    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = app
        .get("/api/events?start=2026-09-25T00:00:00Z&end=2026-09-25T23:59:59Z")
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.is_array());
    assert!(!body.as_array().unwrap().is_empty());
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Test Event"),
        "Expected created event in list"
    );
}

#[tokio::test]
async fn test_list_events_empty_range() {
    // Given no events in the database
    // When GET /api/events?start=...&end=... is called
    // Then returns 200 with an empty array
    let app = TestApp::new_empty().await;

    let resp = app
        .get("/api/events?start=2026-09-25T00:00:00Z&end=2026-09-25T23:59:59Z")
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_events_missing_params() {
    // Given no query parameters
    // When GET /api/events is called without start/end
    // Then returns 422 Unprocessable Entity
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/events").await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_event() {
    // Given a valid JSON payload with title, start_time, end_time
    // When POST /api/events is called
    // Then returns 201 with the created event (including id, created_at)
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Team Standup",
            "start_time": "2026-09-25T09:00:00Z",
            "end_time": "2026-09-25T09:30:00Z"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["id"].is_string(), "Expected id field");
    assert_eq!(body["title"], "Team Standup");
    assert!(body["created_at"].is_string(), "Expected created_at field");
}

#[tokio::test]
async fn test_create_event_missing_fields() {
    // When POST /api/events is called with empty body `{}`
    // Then returns 422 Unprocessable Entity
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/events")
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_event() {
    // Given an event exists
    // When PUT /api/events/:id is called with a new title
    // Then returns 200 with the updated event
    let app = TestApp::new_empty().await;

    // Create an event first
    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Original Title",
            "start_time": "2026-09-25T10:00:00Z",
            "end_time": "2026-09-25T11:00:00Z"
        }))
        .send()
        .await;

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = create_resp.json::<serde_json::Value>().await;
    let event_id = created["id"].as_str().unwrap().to_string();

    // Update the event
    let resp = app
        .put(&format!("/api/events/{}", event_id))
        .json(&serde_json::json!({
            "title": "Updated Title"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["title"], "Updated Title");
    assert_eq!(body["id"], event_id);
}

#[tokio::test]
async fn test_update_event_not_found() {
    // Given no event with that id exists
    // When PUT /api/events/nonexistent is called
    // Then returns 404
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/events/nonexistent")
        .json(&serde_json::json!({
            "title": "X"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_event() {
    // Given an event exists
    // When DELETE /api/events/:id is called
    // Then returns 204 No Content
    let app = TestApp::new_empty().await;

    // Create an event first
    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Event to Delete",
            "start_time": "2026-09-25T10:00:00Z",
            "end_time": "2026-09-25T11:00:00Z"
        }))
        .send()
        .await;

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = create_resp.json::<serde_json::Value>().await;
    let event_id = created["id"].as_str().unwrap().to_string();

    // Delete the event
    let resp = app.delete(&format!("/api/events/{}", event_id)).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_event_not_found() {
    // Given no event with that id exists
    // When DELETE /api/events/nonexistent is called
    // Then returns 404 Not Found
    let app = TestApp::new_empty().await;

    let resp = app.delete("/api/events/nonexistent").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── calendar-range-boundaries ──────────────────────────────────────────

#[tokio::test]
async fn test_list_events_bare_date_covers_last_day() {
    // Regression: two events on the last day of a bare-date range must appear.
    let app = TestApp::new_empty().await;

    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Rodaje cómodo",
            "start_time": "2026-10-31T08:00:00Z",
            "end_time": "2026-10-31T09:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = app.get("/api/events?start=2026-09-27&end=2026-10-31").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["title"].as_str())
        .collect();
    assert!(
        titles.contains(&"Rodaje cómodo"),
        "the event ending on 2026-10-31 must be in the range, got {titles:?}"
    );
}

#[tokio::test]
async fn test_list_events_unparseable_bound_returns_422() {
    let app = TestApp::new_empty().await;

    let resp = app
        .get("/api/events?start=2026-09-25&end=no-es-una-fecha")
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_list_events_offset_bound_matches_instant() {
    let app = TestApp::new_empty().await;

    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Con offset",
            "start_time": "2026-10-31T08:00:00Z",
            "end_time": "2026-10-31T09:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = app
        .get("/api/events?start=2026-10-31T00:00:00%2B02:00&end=2026-10-31T23:59:59%2B01:00")
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["title"].as_str())
        .collect();
    assert!(titles.contains(&"Con offset"));
}

#[tokio::test]
async fn test_list_events_partial_overlap() {
    let app = TestApp::new_empty().await;

    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Solape",
            "start_time": "2026-09-26T23:00:00Z",
            "end_time": "2026-09-27T01:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);

    let resp = app
        .get("/api/events?start=2026-09-27T00:00:00Z&end=2026-09-28T00:00:00Z")
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["title"].as_str())
        .collect();
    assert!(titles.contains(&"Solape"));
}

#[tokio::test]
async fn test_create_event_normalizes_instant_to_utc() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Normalizado",
            "start_time": "2026-10-06T19:00:00+02:00",
            "end_time": "2026-10-06T20:00:00+02:00"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["start_time"], "2026-10-06T17:00:00Z");
    assert_eq!(body["end_time"], "2026-10-06T18:00:00Z");
}

#[tokio::test]
async fn test_create_event_unparseable_start_returns_422() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Mala fecha",
            "start_time": "no-es-una-fecha",
            "end_time": "2026-10-06T18:00:00Z"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_event_normalizes_instant_to_utc() {
    let app = TestApp::new_empty().await;

    let create_resp = app
        .post("/api/events")
        .json(&serde_json::json!({
            "title": "Original",
            "start_time": "2026-10-06T10:00:00Z",
            "end_time": "2026-10-06T11:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = create_resp.json::<serde_json::Value>().await;
    let id = created["id"].as_str().unwrap().to_string();

    let resp = app
        .put(&format!("/api/events/{id}"))
        .json(&serde_json::json!({
            "start_time": "2026-10-06T19:00:00+02:00"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["start_time"], "2026-10-06T17:00:00Z");
}
