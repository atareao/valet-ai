//! Integration tests for the OpenTodo settings surface:
//! `GET /api/opentodo/status`, `GET /api/opentodo/check` and
//! `PUT /api/opentodo/token`.
//!
//! Ver `openspec/changes/opentodo-integration/specs/opentodo-api/spec.md`
//! (requisito del token sensible con endpoints dedicados): el token **nunca**
//! viaja en la respuesta del estado ni se devuelve al guardarlo, y un token
//! vacío o ausente se rechaza sin escribir nada.

mod common;
use common::TestApp;
use serde_json::json;
use serial_test::serial;
use valet::db::repos::settings::SettingsRepo;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// URL base por defecto de OpenTodo.
const DEFAULT_BASE_URL: &str = "https://todo.territoriolinux.es";

/// Escribe una clave de `settings` como lo haría el flujo de configuración.
async fn seed(app: &TestApp, key: &str, value: &str) {
    SettingsRepo::set(&app.db, key, value)
        .await
        .expect("seeding a setting must succeed");
}

/// Aísla el test de las variables de entorno que manejan los tests del servicio.
fn clear_env() {
    std::env::remove_var("OPENTODO_TOKEN");
    std::env::remove_var("OPENTODO_BASE_URL");
}

/// Scenario: el estado informa sin configurar cuando no hay token, con la URL
/// base por defecto y **sin** el token.
#[tokio::test]
#[serial]
async fn status_reports_not_configured_without_token() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/opentodo/status").await;
    assert_eq!(resp.status(), 200, "el estado debe responder 200");
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(
        body.get("configured"),
        Some(&json!(false)),
        "sin token no está configurado: {body}"
    );
    assert_eq!(
        body.get("base_url"),
        Some(&json!(DEFAULT_BASE_URL)),
        "la URL base debe ser la resuelta por defecto: {body}"
    );
    assert!(
        body.get("token").is_none(),
        "el estado no debe exponer el token: {body}"
    );
}

/// Scenario: con token, el estado dice `configured: true`, devuelve la `base_url`
/// resuelta y **no** filtra el token.
#[tokio::test]
#[serial]
async fn status_reports_configured_and_hides_the_token() {
    clear_env();
    let app = TestApp::new_empty().await;
    seed(&app, "opentodo_token", "SECRET").await;

    let resp = app.get("/api/opentodo/status").await;
    assert_eq!(resp.status(), 200);
    let text = resp.text().await;

    assert!(
        text.contains("\"configured\":true"),
        "con token debe informar `configured: true`: {text}"
    );
    assert!(
        text.contains(DEFAULT_BASE_URL),
        "el estado debe traer la URL base resuelta: {text}"
    );
    assert!(
        !text.contains("SECRET"),
        "el estado no debe filtrar el token: {text}"
    );
}

/// Scenario: la `base_url` del estado es la del ajuste cuando está fijado.
#[tokio::test]
#[serial]
async fn status_uses_the_configured_base_url() {
    clear_env();
    let app = TestApp::new_empty().await;
    seed(&app, "opentodo_base_url", "https://todo.midominio.es").await;

    let body = app
        .get("/api/opentodo/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(
        body.get("base_url"),
        Some(&json!("https://todo.midominio.es")),
        "el estado debe usar la URL base del ajuste: {body}"
    );
}

/// Un token resuelto del entorno también cuenta como configurado.
#[tokio::test]
#[serial]
async fn status_counts_the_environment_token() {
    clear_env();
    std::env::set_var("OPENTODO_TOKEN", "SECRET_FROM_ENV");
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/opentodo/status").await;
    let text = resp.text().await;
    clear_env();

    assert!(
        text.contains("\"configured\":true"),
        "un token del entorno configura el servicio: {text}"
    );
    assert!(
        !text.contains("SECRET_FROM_ENV"),
        "el estado no debe filtrar el token del entorno: {text}"
    );
}

/// Scenario: un token vacío se rechaza.
#[tokio::test]
#[serial]
async fn put_token_rejects_empty() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/opentodo/token")
        .json(&json!({ "token": "" }))
        .send()
        .await;

    assert_eq!(resp.status(), 400, "un token vacío debe ser 400");
    assert!(
        SettingsRepo::get(&app.db, "opentodo_token")
            .await
            .expect("reading the token must not fail")
            .unwrap_or_default()
            .is_empty(),
        "un token vacío no debe guardarse"
    );
}

/// Scenario: un token en blanco se rechaza.
#[tokio::test]
#[serial]
async fn put_token_rejects_blank() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/opentodo/token")
        .json(&json!({ "token": "   " }))
        .send()
        .await;

    assert_eq!(resp.status(), 400, "un token en blanco debe ser 400");
    assert!(
        SettingsRepo::get(&app.db, "opentodo_token")
            .await
            .expect("reading the token must not fail")
            .unwrap_or_default()
            .is_empty(),
        "un token en blanco no debe guardarse"
    );
}

/// Scenario: sin el campo `token` también es un token ausente → 400.
#[tokio::test]
#[serial]
async fn put_token_rejects_absent() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app.put("/api/opentodo/token").json(&json!({})).send().await;

    assert_eq!(resp.status(), 400, "sin `token` debe ser 400");
    assert_eq!(
        SettingsRepo::get(&app.db, "opentodo_token")
            .await
            .expect("reading the token must not fail"),
        Some(String::new()),
        "el token sembrado vacío debe seguir vacío"
    );
}

/// Scenario: un token válido se guarda recortado y la respuesta es `204` sin
/// devolverlo.
#[tokio::test]
#[serial]
async fn put_token_stores_it_trimmed_and_returns_204() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/opentodo/token")
        .json(&json!({ "token": "  new-token  " }))
        .send()
        .await;

    assert_eq!(resp.status(), 204, "guardar un token válido debe ser 204");
    let body = resp.text().await;
    assert!(body.is_empty(), "la respuesta no debe traer cuerpo: {body}");
    assert!(
        !body.contains("new-token"),
        "la respuesta no debe devolver el token: {body}"
    );

    assert_eq!(
        SettingsRepo::get(&app.db, "opentodo_token")
            .await
            .expect("reading the token must not fail"),
        Some("new-token".to_string()),
        "el token debe quedar guardado y recortado"
    );
}

/// El token guardado se refleja en el estado: `PUT` y luego `GET` se cuentan
/// una sola historia, y el token sigue sin aparecer.
#[tokio::test]
#[serial]
async fn put_token_then_status_reports_configured() {
    clear_env();
    let app = TestApp::new_empty().await;

    let stored = app
        .put("/api/opentodo/token")
        .json(&json!({ "token": "SECRET" }))
        .send()
        .await;
    assert_eq!(stored.status(), 204);

    let text = app.get("/api/opentodo/status").await.text().await;
    assert!(
        text.contains("\"configured\":true"),
        "tras guardar el token el servicio está configurado: {text}"
    );
    assert!(!text.contains("SECRET"), "el token no debe viajar: {text}");
}

// ---------------------------------------------------------------------------
// GET /api/opentodo/check — sondeo real de la conexión
// ---------------------------------------------------------------------------

/// Escenario: la conexión funciona. `check` sondea `GET /api/v1/me` de verdad y
/// responde `200 {"ok": true, "detail": "conexión correcta"}`, sin el token.
#[tokio::test]
#[serial]
async fn check_reports_a_working_connection() {
    clear_env();
    let app = TestApp::new_empty().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/me"))
        .and(header("Authorization", "Bearer SECRET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "u1" })))
        .expect(1)
        .mount(&server)
        .await;
    seed(&app, "opentodo_token", "SECRET").await;
    seed(&app, "opentodo_base_url", &server.uri()).await;

    let resp = app.get("/api/opentodo/check").await;
    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let text = resp.text().await;

    assert!(
        text.contains("\"ok\":true"),
        "una conexión sana debe informar `ok: true`: {text}"
    );
    assert!(
        text.contains("conexión correcta"),
        "el detalle debe decir que va bien: {text}"
    );
    assert!(!text.contains("SECRET"), "el token no debe viajar: {text}");
}

/// Escenario: el token no vale (`401` del servidor).
#[tokio::test]
#[serial]
async fn check_reports_an_invalid_token() {
    clear_env();
    let app = TestApp::new_empty().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    seed(&app, "opentodo_token", "SECRET").await;
    seed(&app, "opentodo_base_url", &server.uri()).await;

    let resp = app.get("/api/opentodo/check").await;
    assert_eq!(resp.status(), 200);
    let text = resp.text().await;

    assert!(text.contains("\"ok\":false"), "{text}");
    assert!(
        text.contains("el token no es válido o ha caducado"),
        "el detalle debe distinguir un token inválido: {text}"
    );
    assert!(!text.contains("SECRET"), "el token no debe viajar: {text}");
}

/// Escenario: el servidor no responde.
#[tokio::test]
#[serial]
async fn check_reports_an_unreachable_server() {
    clear_env();
    let app = TestApp::new_empty().await;
    // Puerto cerrado: fallo de red inmediato, sin tocar ninguna instancia real.
    seed(&app, "opentodo_token", "SECRET").await;
    seed(&app, "opentodo_base_url", "http://127.0.0.1:1").await;

    let resp = app.get("/api/opentodo/check").await;
    assert_eq!(resp.status(), 200);
    let text = resp.text().await;

    assert!(text.contains("\"ok\":false"), "{text}");
    assert!(
        text.contains("OpenTodo no responde"),
        "el detalle debe decir que no responde: {text}"
    );
    assert!(!text.contains("SECRET"), "el token no debe viajar: {text}");
}

/// Escenario: no hay token configurado. El sondeo no sale a la red.
#[tokio::test]
#[serial]
async fn check_reports_a_missing_token() {
    clear_env();
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/opentodo/check").await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!("no hay token de OpenTodo configurado")),
        "el detalle debe decir que falta el token: {body}"
    );
}

/// El JSON de `check` cumple su contrato: exactamente `ok` y `detail`, sin el
/// token ni en un campo extra.
#[tokio::test]
#[serial]
async fn check_returns_only_ok_and_detail() {
    clear_env();
    let app = TestApp::new_empty().await;
    seed(&app, "opentodo_token", "SECRET").await;

    let text = app.get("/api/opentodo/check").await.text().await;
    let body = serde_json::from_str::<serde_json::Value>(&text).expect("JSON");

    let keys: std::collections::BTreeSet<&str> = body
        .as_object()
        .expect("un objeto")
        .keys()
        .map(|key| key.as_str())
        .collect();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from(["ok", "detail"]),
        "el veredicto solo lleva `ok` y `detail`: {body}"
    );
    assert!(!text.contains("SECRET"), "el token no debe viajar: {text}");
}
