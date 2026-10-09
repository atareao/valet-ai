//! Integration tests for the streaming chat endpoint.
//!
//! These tests verify that the SSE streaming endpoint and the approval
//! resolution endpoint are properly wired and return the expected HTTP
//! responses.

mod common;
use common::TestApp;
use serde_json::json;

// ---------------------------------------------------------------------------
// Chat / stream endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_stream_endpoint_route_exists() {
    // Given a running app
    // When POST /api/chat/stream with valid JSON
    // Then the response is NOT 404 (the route exists)
    let app = TestApp::new().await;

    let resp = app
        .post("/api/chat/stream")
        .json(&json!({"content": "Hola"}))
        .send()
        .await;

    assert!(
        resp.status() != 404,
        "Stream endpoint returned 404 — route not registered"
    );
    assert!(
        resp.status().is_success(),
        "Stream endpoint failed with status: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_stream_endpoint_content_type_is_sse() {
    // Given a running app
    // When POST /api/chat/stream
    // Then the Content-Type header is text/event-stream
    let app = TestApp::new().await;

    let resp = app
        .post("/api/chat/stream")
        .json(&json!({"content": "Hello"}))
        .send()
        .await;

    let headers = resp.headers();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    assert!(
        content_type.contains("text/event-stream"),
        "Expected text/event-stream, got: {}",
        content_type
    );
}

// ---------------------------------------------------------------------------
// Approval endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_approval_endpoint_route_exists() {
    // Given a running app
    // When POST /api/approval/test-request
    // Then 200 OK is returned
    let app = TestApp::new().await;

    let resp = app
        .post("/api/approval/test-request")
        .json(&json!({"approved": true}))
        .send()
        .await;

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_approval_endpoint_returns_expected_body() {
    // Given a running app
    // When POST /api/approval/req-1 with approved: true
    // Then the body contains status: "resolved" and approved: true
    let app = TestApp::new().await;

    let resp = app
        .post("/api/approval/req-1")
        .json(&json!({"approved": true}))
        .send()
        .await;

    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["status"], "resolved");
    assert_eq!(body["approved"], true);
}

#[tokio::test]
async fn test_approval_endpoint_with_deny() {
    // Given a running app
    // When POST /api/approval/req-2 with approved: false
    // Then the body contains status: "resolved" and approved: false
    let app = TestApp::new().await;

    let resp = app
        .post("/api/approval/req-2")
        .json(&json!({"approved": false}))
        .send()
        .await;

    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["status"], "resolved");
    assert_eq!(body["approved"], false);
}

#[tokio::test]
async fn test_approval_missing_field_returns_error() {
    // Given a running app
    // When POST /api/approval/req-1 with empty body
    // Then a client error is returned
    let app = TestApp::new().await;

    let resp = app
        .post("/api/approval/req-1")
        .json(&json!({}))
        .send()
        .await;

    assert!(
        resp.status().is_client_error(),
        "Expected client error for missing field, got: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Extended scenarios: override commands, method validation, contract checks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_stream_with_override_historico() {
    // Given a running app
    // When the message starts with !historico override prefix
    // Then the stream endpoint still accepts it and returns 200 OK
    let app = TestApp::new().await;

    let resp = app
        .post("/api/chat/stream")
        .json(&json!({"content": "!historico ¿qué pasó ayer?"}))
        .send()
        .await;

    assert!(
        resp.status().is_success(),
        "Stream endpoint rejected !historico prefix: {}",
        resp.status()
    );
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/event-stream"),
        "Expected SSE content-type with !historico, got: {}",
        content_type
    );
}

#[tokio::test]
async fn test_stream_missing_content_returns_error() {
    // Given a running app
    // When POST /api/chat/stream with empty JSON body
    // Then a 4xx client error is returned (content field is required)
    let app = TestApp::new().await;

    let resp = app.post("/api/chat/stream").json(&json!({})).send().await;

    assert!(
        resp.status().is_client_error(),
        "Expected client error for missing content field, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_stream_wrong_method_returns_405() {
    // Given a running app
    // When GET /api/chat/stream (only POST is allowed)
    // Then 405 Method Not Allowed is returned
    let app = TestApp::new().await;

    let resp = app.get("/api/chat/stream").await;

    assert_eq!(
        resp.status(),
        405,
        "Expected 405 Method Not Allowed for GET on stream endpoint"
    );
}

#[tokio::test]
async fn test_messages_endpoint_still_works() {
    // Given a running app with seeded data
    // When GET /api/messages
    // Then 200 OK is returned
    let app = TestApp::new().await;

    let resp = app.get("/api/messages").await;

    assert_eq!(resp.status(), 200, "GET messages endpoint no longer works");
}
