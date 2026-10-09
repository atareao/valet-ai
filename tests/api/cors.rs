use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

fn get(origin: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri("/api/health");
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    builder.body(Body::empty()).unwrap()
}

fn preflight(origin: &str, method: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri("/api/health")
        .header("origin", origin)
        .header("access-control-request-method", method)
        .body(Body::empty())
        .unwrap()
}

fn header<'a>(response: &'a axum::response::Response, name: &str) -> Option<&'a str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// At least one CORS header is present when an Origin is sent.
#[tokio::test]
async fn test_cors_headers_present() {
    let app = valet::app().await;

    let response = app
        .oneshot(get(Some("http://localhost:5173")))
        .await
        .unwrap();

    assert!(
        response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "a known dev origin must receive an Access-Control-Allow-Origin header"
    );
}

/// A known dev origin is echoed explicitly and credentials are allowed; the
/// wildcard must never be used together with credentials.
#[tokio::test]
async fn cors_allows_explicit_dev_origin_with_credentials() {
    let app = valet::app().await;

    let response = app
        .oneshot(get(Some("http://localhost:5173")))
        .await
        .unwrap();

    assert_eq!(
        header(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()),
        Some("http://localhost:5173")
    );
    assert_ne!(
        header(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()),
        Some("*"),
        "credentials must never be combined with the `*` wildcard"
    );
    assert_eq!(
        header(&response, header::ACCESS_CONTROL_ALLOW_CREDENTIALS.as_str()),
        Some("true")
    );
}

/// An arbitrary/unknown origin must not be reflected when credentials are on.
#[tokio::test]
async fn cors_does_not_reflect_arbitrary_origin_with_credentials() {
    let app = valet::app().await;

    let response = app.oneshot(get(Some("http://evil.example"))).await.unwrap();

    assert!(
        header(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()).is_none(),
        "an unknown origin must not be reflected with credentials enabled"
    );
}

/// The CORS preflight (`OPTIONS`) is answered by the CORS layer, not the
/// session middleware.
#[tokio::test]
async fn cors_preflight_options_is_handled() {
    let app = valet::app().await;

    let response = app
        .oneshot(preflight("http://localhost:5173", "GET"))
        .await
        .unwrap();

    assert!(
        response.status().is_success(),
        "preflight must succeed, got {}",
        response.status()
    );
    assert_eq!(
        header(&response, header::ACCESS_CONTROL_ALLOW_ORIGIN.as_str()),
        Some("http://localhost:5173")
    );
    assert!(
        header(&response, header::ACCESS_CONTROL_ALLOW_METHODS.as_str()).is_some(),
        "preflight must advertise the allowed methods"
    );
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}
