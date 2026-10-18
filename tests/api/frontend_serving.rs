use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

/// RED phase test: asserts that GET / returns index.html.
/// This will FAIL because the router doesn't have a ServeDir fallback yet.
#[tokio::test]
async fn test_root_returns_index_html() {
    let app = valet::app().await;

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/html"),
        "Expected text/html, got: {content_type}"
    );
}

/// RED phase test: asserts that GET /some/spa/route returns index.html (SPA fallback).
/// This will FAIL because the router doesn't have a ServeFile fallback.
#[tokio::test]
async fn test_spa_fallback_returns_index_html() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/some/spa/route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/html"),
        "Expected text/html for SPA fallback, got: {content_type}"
    );
}

/// RED phase test: asserts that GET /api/health still returns JSON even with
/// the static file fallback in place.
#[tokio::test]
async fn test_api_health_unaffected_by_fallback() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("json"),
        "Expected application/json for API, got: {content_type}"
    );
}

/// RED phase test: asserts that GET /static/index.html serves the actual file.
/// This will FAIL because the router doesn't have a ServeDir fallback.
#[tokio::test]
async fn test_static_file_is_served() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(
        body_str.contains("Valet"),
        "Expected index.html content to contain 'Valet', got: {body_str}"
    );
}

/// Asserts that GET /favicon.ico serves the icon and NOT the SPA fallback.
#[tokio::test]
async fn test_favicon_is_served() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("image/x-icon") || content_type.contains("image/vnd.microsoft.icon"),
        "Expected image/x-icon or image/vnd.microsoft.icon, got: {content_type}"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(
        !body_str.to_ascii_lowercase().contains("<html"),
        "Expected favicon binary, but body looked like the SPA fallback HTML"
    );
}

/// Asserts that GET /icon-512.png serves the PWA icon with a non-empty body.
#[tokio::test]
async fn test_pwa_icon_512_is_served() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/icon-512.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("image/png"),
        "Expected image/png, got: {content_type}"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(
        body_bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Expected the body to start with the PNG signature, got {} bytes",
        body_bytes.len()
    );
}

/// Asserts that GET /manifest.webmanifest serves the PWA manifest.
#[tokio::test]
async fn test_manifest_is_served() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/manifest.webmanifest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("json"),
        "Expected a JSON manifest content-type, got: {content_type}"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(
        body_str.contains("\"name\""),
        "Expected manifest JSON to contain a \"name\" field, got: {body_str}"
    );
}

/// Asserts that GET /sw.js serves the real service worker as JavaScript and NOT
/// the SPA fallback.
///
/// Regression guard: if `static/sw.js` were missing, the SPA fallback would serve
/// `static/index.html` (`text/html`) at `/sw.js`. The browser would then reject
/// the service-worker registration, leaving the Notifications tab waiting forever.
#[tokio::test]
async fn test_sw_js_is_served_as_javascript() {
    let app = valet::app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/sw.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("javascript"),
        "Expected a JavaScript content-type, got: {content_type}"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(
        body_str.contains("addEventListener"),
        "Expected the service worker source to contain 'addEventListener', got: {body_str}"
    );
    assert!(
        !body_str.to_ascii_lowercase().contains("<html"),
        "Expected the service worker source, but body looked like the SPA fallback HTML"
    );
}
