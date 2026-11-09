//! Integration tests for the GitHub settings surface:
//! `GET /api/github/status`, `GET /api/github/check` and `PUT /api/github/token`,
//! plus the sensitivity of `github_token` in `GET`/`PUT /api/settings`.
//!
//! Ver `openspec/changes/github-integration/specs/github-api/spec.md`: el token
//! **nunca** viaja en la respuesta del estado ni en la lectura de ajustes, un
//! valor vacío o ausente se rechaza sin escribir nada, y la sonda responde
//! siempre `200` (sin token, sin tocar la red).

mod common;
use common::TestApp;
use serde_json::json;
use valet::db::repos::settings::SettingsRepo;

/// URL por defecto del repositorio de trabajo.
const DEFAULT_REPO: &str = "atareao/valet-ai";

/// Escribe una clave de `settings` como lo haría el flujo de configuración.
async fn seed(app: &TestApp, key: &str, value: &str) {
    SettingsRepo::set(&app.db, key, value)
        .await
        .expect("seeding a setting must succeed");
}

// ---------------------------------------------------------------------------
// GET /api/settings — el token no viaja
// ---------------------------------------------------------------------------

/// Scenario: el token no aparece en la lectura de ajustes.
#[tokio::test]
async fn settings_omits_the_token_and_keeps_the_public_keys() {
    let app = TestApp::new_empty().await;
    seed(&app, "github_token", "SECRET").await;
    seed(&app, "github_default_repo", "otro/repo").await;

    let text = app.get("/api/settings").await.text().await;

    assert!(
        text.contains("github_default_repo"),
        "el ajuste público debe viajar: {text}"
    );
    assert!(
        text.contains("github_default_labels"),
        "los labels públicos deben viajar: {text}"
    );
    assert!(
        !text.contains("github_token") && !text.contains("SECRET"),
        "el token no debe aparecer en la lectura de ajustes: {text}"
    );
}

/// Scenario: `PUT /api/settings` persiste los ajustes no sensibles e ignora el
/// token si viaja en el cuerpo.
#[tokio::test]
async fn put_settings_persists_public_keys_and_ignores_the_token() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/settings")
        .json(&json!({
            "github_default_repo": "otro/repo",
            "github_default_labels": "feature, valet",
            "github_token": "SHOULD-BE-IGNORED"
        }))
        .send()
        .await;
    assert!(
        resp.status().is_success(),
        "guardar ajustes debe ir bien: {}",
        resp.status()
    );

    assert_eq!(
        SettingsRepo::get(&app.db, "github_default_repo")
            .await
            .unwrap()
            .as_deref(),
        Some("otro/repo")
    );
    assert_eq!(
        SettingsRepo::get(&app.db, "github_default_labels")
            .await
            .unwrap()
            .as_deref(),
        Some("feature, valet")
    );
    assert!(
        SettingsRepo::get(&app.db, "github_token")
            .await
            .unwrap()
            .unwrap_or_default()
            .is_empty(),
        "`github_token` debe ignorarse en `PUT /api/settings`"
    );
}

// ---------------------------------------------------------------------------
// GET /api/github/status
// ---------------------------------------------------------------------------

/// Scenario: el estado informa sin configurar cuando falta el token.
#[tokio::test]
async fn status_reports_not_configured_without_token() {
    let app = TestApp::new_empty().await;

    let body = app
        .get("/api/github/status")
        .await
        .json::<serde_json::Value>()
        .await;

    assert_eq!(body.get("configured"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("default_repo"),
        Some(&json!(DEFAULT_REPO)),
        "{body}"
    );
    assert!(body.get("token").is_none(), "no debe exponer el token");
}

/// Scenario: el estado informa sin revelar el token.
#[tokio::test]
async fn status_reports_configured_and_hides_the_token() {
    let app = TestApp::new_empty().await;
    seed(&app, "github_token", "SECRET").await;
    seed(&app, "github_default_repo", "otro/repo").await;

    let text = app.get("/api/github/status").await.text().await;

    assert!(text.contains("\"configured\":true"), "{text}");
    assert!(
        text.contains("otro/repo"),
        "el estado debe traer el repo: {text}"
    );
    assert!(
        !text.contains("SECRET"),
        "el estado no debe filtrar el token: {text}"
    );
}

// ---------------------------------------------------------------------------
// PUT /api/github/token
// ---------------------------------------------------------------------------

/// Scenario: un token vacío se rechaza.
#[tokio::test]
async fn put_token_rejects_blank() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/github/token")
        .json(&json!({ "token": "   " }))
        .send()
        .await;

    assert_eq!(resp.status(), 400, "un token en blanco debe ser 400");
    assert!(
        SettingsRepo::get(&app.db, "github_token")
            .await
            .expect("reading the token must not fail")
            .unwrap_or_default()
            .is_empty(),
        "un token en blanco no debe guardarse"
    );
}

/// Scenario: sin el campo `token` también es ausente → 400.
#[tokio::test]
async fn put_token_rejects_absent() {
    let app = TestApp::new_empty().await;

    let resp = app.put("/api/github/token").json(&json!({})).send().await;

    assert_eq!(resp.status(), 400, "sin `token` debe ser 400");
}

/// Scenario: un token válido se guarda.
#[tokio::test]
async fn put_token_stores_it_trimmed_and_returns_204() {
    let app = TestApp::new_empty().await;

    let resp = app
        .put("/api/github/token")
        .json(&json!({ "token": "  ghp_secret  " }))
        .send()
        .await;

    assert_eq!(resp.status(), 204, "guardar un token válido debe ser 204");
    let body = resp.text().await;
    assert!(body.is_empty(), "la respuesta no debe traer cuerpo: {body}");

    assert_eq!(
        SettingsRepo::get(&app.db, "github_token")
            .await
            .expect("reading the token must not fail")
            .as_deref(),
        Some("ghp_secret"),
        "el token debe quedar guardado y recortado"
    );
}

// ---------------------------------------------------------------------------
// GET /api/github/check
// ---------------------------------------------------------------------------

/// Scenario: sin token la sonda lo dice sin tocar la red.
#[tokio::test]
async fn check_reports_incomplete_configuration_without_network() {
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/github/check").await;
    assert_eq!(resp.status(), 200, "el sondeo responde siempre 200");
    let body = resp.json::<serde_json::Value>().await;

    assert_eq!(body.get("ok"), Some(&json!(false)), "{body}");
    assert_eq!(
        body.get("detail"),
        Some(&json!("configuración incompleta")),
        "{body}"
    );
}
