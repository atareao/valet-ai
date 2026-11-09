//! Integration tests for the Blog settings surface:
//! `GET /api/blog/status`, `GET /api/blog/check` and `PUT /api/blog/password`,
//! plus the sensitivity of `blog_app_password` in `GET /api/settings`.
//!
//! Ver `openspec/changes/wordpress-credentials-settings/specs/blog-api/spec.md`:
//! la Application Password **nunca** viaja en la respuesta del estado, no
//! aparece en `GET /api/settings`, y un valor vacío o ausente se rechaza sin
//! escribir nada. La sonda responde siempre `200`.

mod common;
use common::TestApp;
use serde_json::json;
use valet::db::repos::settings::SettingsRepo;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// URL base por defecto del blog.
const DEFAULT_BASE_URL: &str = "https://atareao.es";

/// Escribe una clave de `settings` como lo haría el flujo de configuración.
async fn seed(app: &TestApp, key: &str, value: &str) {
    SettingsRepo::set(&app.db, key, value)
        .await
        .expect("seeding a setting must succeed");
}

// ---------------------------------------------------------------------------
// GET /api/settings — la contraseña no viaja
// ---------------------------------------------------------------------------

/// Scenario: la contraseña no aparece en la lectura de ajustes.
#[tokio::test]
async fn settings_omits_the_password_and_keeps_the_public_keys() {
    let app = TestApp::new_empty().await;
    seed(&app, "blog_app_password", "SECRET").await;
    seed(&app, "blog_service_user", "alice").await;

    let text = app.get("/api/settings").await.text().await;

    assert!(
        text.contains("blog_base_url"),
        "el ajuste público debe viajar: {text}"
    );
    assert!(
        text.contains("blog_service_user"),
        "el usuario de servicio debe viajar: {text}"
    );
    assert!(
        !text.contains("blog_app_password") && !text.contains("SECRET"),
        "la contraseña no debe aparecer en la lectura de ajustes: {text}"
    );
}

// ---------------------------------------------------------------------------
// GET /api/blog/status
// ---------------------------------------------------------------------------

/// Scenario: el estado informa sin configurar cuando faltan las credenciales.
#[tokio::test]
async fn status_reports_not_configured_without_credentials() {
    let app = TestApp::new_empty().await;

    let body = app
        .get("/api/blog/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(body.get("configured"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("base_url"),
        Some(&json!(DEFAULT_BASE_URL)),
        "{body}"
    );
    assert!(
        body.get("password").is_none(),
        "no debe exponer la contraseña"
    );
}

/// Scenario: el estado informa sin revelar la contraseña.
#[tokio::test]
async fn status_reports_configured_and_hides_the_password() {
    let app = TestApp::new_empty().await;
    seed(&app, "blog_service_user", "alice").await;
    seed(&app, "blog_app_password", "SECRET").await;

    let text = app.get("/api/blog/status").await.text().await;

    assert!(text.contains("\"configured\":true"), "{text}");
    assert!(
        text.contains("alice"),
        "el estado debe traer el usuario: {text}"
    );
    assert!(
        !text.contains("SECRET"),
        "el estado no debe filtrar la contraseña"
    );
}

/// Scenario: la `base_url` del estado es la del ajuste cuando está fijado.
#[tokio::test]
async fn status_uses_the_configured_base_url() {
    let app = TestApp::new_empty().await;
    seed(&app, "blog_base_url", "https://mi-blog.example").await;

    let body = app
        .get("/api/blog/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(
        body.get("base_url"),
        Some(&json!("https://mi-blog.example")),
        "{body}"
    );
}

// ---------------------------------------------------------------------------
// PUT /api/blog/password
// ---------------------------------------------------------------------------

/// Scenario: una contraseña vacía se rechaza.
#[tokio::test]
async fn put_password_rejects_blank() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/blog/password")
        .json(&json!({ "password": "   " }))
        .send()
        .await;

    assert_eq!(resp.status(), 400, "una contraseña en blanco debe ser 400");
    assert!(
        SettingsRepo::get(&app.db, "blog_app_password")
            .await
            .expect("reading the password must not fail")
            .unwrap_or_default()
            .is_empty(),
        "una contraseña en blanco no debe guardarse"
    );
}

/// Scenario: sin el campo `password` también es ausente → 400.
#[tokio::test]
async fn put_password_rejects_absent() {
    let app = TestApp::new_empty().await;

    let resp = app.put("/api/blog/password").json(&json!({})).send().await;

    assert_eq!(resp.status(), 400, "sin `password` debe ser 400");
}

/// Scenario: una contraseña válida se guarda.
#[tokio::test]
async fn put_password_stores_it_trimmed_and_returns_204() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/blog/password")
        .json(&json!({ "password": "  app-pass  " }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        204,
        "guardar una contraseña válida debe ser 204"
    );
    let body = resp.text().await;
    assert!(body.is_empty(), "la respuesta no debe traer cuerpo: {body}");

    assert_eq!(
        SettingsRepo::get(&app.db, "blog_app_password")
            .await
            .expect("reading the password must not fail"),
        Some("app-pass".to_string()),
        "la contraseña debe quedar guardada y recortada"
    );
}

// ---------------------------------------------------------------------------
// GET /api/blog/check
// ---------------------------------------------------------------------------

/// Scenario: sonda correcta con credenciales válidas.
#[tokio::test]
async fn check_reports_a_working_connection() {
    let app = TestApp::new_empty().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wp-json/wp/v2/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": 1 })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/wp-json/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "routes": { "/wp/v2/comments": { "methods": ["GET", "POST"] } }
        })))
        .expect(1)
        .mount(&server)
        .await;
    seed(&app, "blog_base_url", &server.uri()).await;
    seed(&app, "blog_service_user", "alice").await;
    seed(&app, "blog_app_password", "SECRET").await;

    let resp = app.get("/api/blog/check").await;
    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let text = resp.text().await;

    assert!(text.contains("\"ok\":true"), "{text}");
    assert!(text.contains("conexión correcta"), "{text}");
    assert!(
        !text.contains("SECRET"),
        "la contraseña no debe viajar: {text}"
    );
}

/// Scenario: sin credenciales la sonda lo dice sin tocar la red.
#[tokio::test]
async fn check_reports_missing_credentials_without_network() {
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/blog/check").await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!(
            "faltan el usuario o la contraseña de aplicación del blog"
        )),
        "{body}"
    );
}

/// Scenario: unas credenciales inválidas se distinguen de una caída.
#[tokio::test]
async fn check_reports_invalid_credentials() {
    let app = TestApp::new_empty().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wp-json/wp/v2/users/me"))
        .respond_with(
            ResponseTemplate::new(401).set_body_string("nope: SECRET\nAuthorization: Basic xyz"),
        )
        .mount(&server)
        .await;
    seed(&app, "blog_base_url", &server.uri()).await;
    seed(&app, "blog_service_user", "alice").await;
    seed(&app, "blog_app_password", "SECRET").await;

    let resp = app.get("/api/blog/check").await;
    assert_eq!(resp.status(), 200);
    let text = resp.text().await;

    assert!(text.contains("\"ok\":false"), "{text}");
    assert!(
        text.contains("el usuario o la contraseña de aplicación no son válidos"),
        "{text}"
    );
    assert!(
        !text.contains("SECRET"),
        "no debe filtrar la contraseña: {text}"
    );
}

/// Scenario: la vía de respuesta cerrada se distingue de unas credenciales válidas.
#[tokio::test]
async fn check_reports_reply_route_unavailable() {
    let app = TestApp::new_empty().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wp-json/wp/v2/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": 1 })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/wp-json/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "routes": {} })))
        .mount(&server)
        .await;
    seed(&app, "blog_base_url", &server.uri()).await;
    seed(&app, "blog_service_user", "alice").await;
    seed(&app, "blog_app_password", "SECRET").await;

    let resp = app.get("/api/blog/check").await;
    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let text = resp.text().await;

    assert!(text.contains("\"ok\":false"), "{text}");
    assert!(
        text.contains(
            "las credenciales son válidas, pero la ruta de respuesta del blog no \
             está disponible: revisa la política REST del plugin"
        ),
        "{text}"
    );
    assert!(
        !text.contains("SECRET"),
        "no debe filtrar la contraseña: {text}"
    );
}

/// Scenario: un sitio caído no es un error de Valet.
#[tokio::test]
async fn check_reports_an_unreachable_server() {
    let app = TestApp::new_empty().await;
    // Puerto cerrado: fallo de red inmediato, sin tocar ninguna instancia real.
    seed(&app, "blog_base_url", "http://127.0.0.1:1").await;
    seed(&app, "blog_service_user", "alice").await;
    seed(&app, "blog_app_password", "SECRET").await;

    let resp = app.get("/api/blog/check").await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!("el blog no responde")),
        "{body}"
    );
}
