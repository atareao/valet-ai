//! Integration tests for the Mastodon settings surface: `GET /api/mastodon/status`,
//! `GET /api/mastodon/check` y `PUT /api/mastodon/token`, además de la
//! sensibilidad de `mastodon_token` en `GET`/`PUT /api/settings`.
//!
//! Ver `openspec/changes/mastodon-mentions/specs/mastodon-api/spec.md`.

mod common;
use common::TestApp;
use serde_json::json;
use valet::db::repos::settings::SettingsRepo;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn seed(app: &TestApp, key: &str, value: &str) {
    SettingsRepo::set(&app.db, key, value)
        .await
        .expect("seeding a setting must succeed");
}

// ---------------------------------------------------------------------------
// GET/PUT /api/settings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn settings_omits_the_token_and_keeps_the_public_url() {
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "MASTODON_SECRET").await;
    seed(&app, "mastodon_base_url", "https://social.example").await;

    let text = app.get("/api/settings").await.text().await;

    assert!(
        text.contains("mastodon_base_url"),
        "la URL pública debe viajar: {text}"
    );
    assert!(
        !text.contains("mastodon_token") && !text.contains("MASTODON_SECRET"),
        "el token no debe aparecer en la lectura de ajustes: {text}"
    );
}

#[tokio::test]
async fn put_settings_persists_the_url_and_ignores_the_token() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/settings")
        .json(&json!({
            "mastodon_base_url": "https://social.example",
            "mastodon_token": "SHOULD-BE-IGNORED"
        }))
        .send()
        .await;
    assert!(
        resp.status().is_success(),
        "guardar ajustes debe ir bien: {}",
        resp.status()
    );

    assert_eq!(
        SettingsRepo::get(&app.db, "mastodon_base_url")
            .await
            .unwrap()
            .as_deref(),
        Some("https://social.example")
    );
    assert!(
        SettingsRepo::get(&app.db, "mastodon_token")
            .await
            .unwrap()
            .unwrap_or_default()
            .is_empty(),
        "`mastodon_token` debe ignorarse en `PUT /api/settings`"
    );
}

// ---------------------------------------------------------------------------
// GET /api/mastodon/status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_reports_not_configured_without_token() {
    let app = TestApp::new_empty().await;

    let body = app
        .get("/api/mastodon/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(body.get("configured"), Some(&json!(false)), "{body}");
    assert!(
        body.get("token").is_none(),
        "no debe exponer el token: {body}"
    );
}

#[tokio::test]
async fn status_reports_configured_and_hides_the_token() {
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "MASTODON_SECRET").await;
    seed(&app, "mastodon_base_url", "https://social.example").await;

    let body = app
        .get("/api/mastodon/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(body.get("configured"), Some(&json!(true)), "{body}");
    assert_eq!(
        body.get("base_url"),
        Some(&json!("https://social.example")),
        "{body}"
    );
    assert!(
        body.get("token").is_none() && !body.to_string().contains("MASTODON_SECRET"),
        "el estado no debe filtrar el token: {body}"
    );
}

// ---------------------------------------------------------------------------
// PUT /api/mastodon/token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_token_rejects_blank_and_absent_without_changing_it() {
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "OLD_TOKEN").await;

    for body in [json!({"token": "   "}), json!({})] {
        let resp = app.put("/api/mastodon/token").json(&body).send().await;
        assert_eq!(resp.status(), 400, "cuerpo {body} debe ser 400");
    }

    assert_eq!(
        SettingsRepo::get(&app.db, "mastodon_token")
            .await
            .unwrap()
            .as_deref(),
        Some("OLD_TOKEN"),
        "un token en blanco no debe modificar el guardado"
    );
}

#[tokio::test]
async fn put_token_stores_it_trimmed_and_returns_204() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/mastodon/token")
        .json(&json!({ "token": "  mastodon-secret  " }))
        .send()
        .await;

    assert_eq!(resp.status(), 204, "guardar un token válido debe ser 204");
    assert!(
        resp.text().await.is_empty(),
        "la respuesta no debe traer cuerpo"
    );
    assert_eq!(
        SettingsRepo::get(&app.db, "mastodon_token")
            .await
            .unwrap()
            .as_deref(),
        Some("mastodon-secret")
    );
}

// ---------------------------------------------------------------------------
// GET /api/mastodon/check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn check_without_token_reports_incomplete_configuration_without_network() {
    let server = MockServer::start().await;
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_base_url", &server.uri()).await;

    let resp = app.get("/api/mastodon/check").await;
    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!("configuración incompleta")),
        "{body}"
    );
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty(),
        "sin token no se toca la red"
    );
}

#[tokio::test]
async fn check_without_base_url_asks_for_configuration_without_network() {
    let server = MockServer::start().await;
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "MASTODON_SECRET").await;

    let body = app
        .get("/api/mastodon/check")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    let detail = body["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("mastodon_base_url") && !detail.contains("MASTODON_SECRET"),
        "detalle accionable y sin secretos: {detail}"
    );
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty(),
        "sin URL base no se toca la red"
    );
}

#[tokio::test]
async fn check_with_valid_token_reports_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/notifications"))
        .and(query_param("limit", "1"))
        .and(query_param("types[]", "mention"))
        .and(header("authorization", "Bearer MASTODON_SECRET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "MASTODON_SECRET").await;
    seed(&app, "mastodon_base_url", &server.uri()).await;

    let resp = app.get("/api/mastodon/check").await;

    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body.get("ok"), Some(&json!(true)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!("conexión correcta")),
        "{body}"
    );
}

#[tokio::test]
async fn check_with_rejected_token_reports_it_without_leaking_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/notifications"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let app = TestApp::new_empty().await;
    seed(&app, "mastodon_token", "MASTODON_SECRET").await;
    seed(&app, "mastodon_base_url", &server.uri()).await;

    let resp = app.get("/api/mastodon/check").await;

    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    let detail = body["detail"].as_str().unwrap_or_default();
    assert!(
        detail.to_lowercase().contains("token") && !detail.contains("MASTODON_SECRET"),
        "detalle accionable y sin secretos: {detail}"
    );
}
