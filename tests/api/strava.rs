//! Integration tests for the Strava diagnostics HTTP surface.
//!
//! These cases avoid the network entirely: with no connection configured, the
//! handlers must answer from local state (not connected), never reaching out to
//! Strava, and must never leak the OAuth tokens.
//!
//! Ver `openspec/changes/strava-diagnostics/specs/tools/strava/spec.md`.

mod common;
use common::TestApp;
use serde_json::json;

/// `GET /api/strava/check` sin conexión: `200`, `ok=false`, un mensaje que pide
/// conectar la cuenta y sin tokens en el cuerpo.
#[tokio::test]
async fn check_without_connection_asks_to_connect_and_hides_tokens() {
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/strava/check").await;
    assert_eq!(resp.status(), 200, "el sondeo debe responder 200");

    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(
        body["ok"],
        json!(false),
        "sin conexión no hay diagnóstico ok"
    );

    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("conecta") || error.contains("conectar"),
        "el error debe pedir conectar la cuenta: {body}"
    );

    assert!(
        body.get("access_token").is_none(),
        "el cuerpo no debe exponer el access token: {body}"
    );
    assert!(
        body.get("refresh_token").is_none(),
        "el cuerpo no debe exponer el refresh token: {body}"
    );
}

/// `POST /api/strava/disconnect` sin conexión: `200`, `connected=false` y sin
/// aviso (no hay nada que revocar y no se sale a la red).
#[tokio::test]
async fn disconnect_without_connection_reports_not_connected_without_warning() {
    let app = TestApp::new_empty().await;

    let resp = app.post("/api/strava/disconnect").send().await;
    assert_eq!(resp.status(), 200, "la desconexión debe responder 200");

    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["connected"], json!(false), "queda desconectada");
    assert!(
        body.get("warning").is_none() || body["warning"].is_null(),
        "sin nada que revocar no debe haber aviso: {body}"
    );
}
