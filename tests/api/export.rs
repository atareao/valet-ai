use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn test_export_returns_json() {
    let app = valet::app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/export")
                .header("Content-Type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    // All core tables should be present in the export
    assert!(data.get("profiles").is_some(), "missing profiles");
    assert!(data.get("messages").is_some(), "missing messages");
    assert!(data.get("events").is_some(), "missing events");
    assert!(data.get("tasks").is_some(), "missing tasks");
    assert!(data.get("notes").is_some(), "missing notes");
    assert!(data.get("reminders").is_some(), "missing reminders");
    assert!(data.get("memory").is_some(), "missing memory");
    assert!(data.get("tools").is_some(), "missing tools");
}

#[tokio::test]
async fn test_export_contains_seeded_data() {
    let app = valet::app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/export")
                .header("Content-Type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();

    // Seeded data should be present
    let profiles = data["profiles"].as_array().unwrap();
    assert!(!profiles.is_empty(), "expected at least one profile");
    assert_eq!(profiles[0]["name"], "Test User");
}
