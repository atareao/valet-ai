//! Integration tests for the settings HTTP surface, focused on the security
//! boundary: the OAuth token keys (`strava_access_token`,
//! `strava_refresh_token`) must never travel through `/api/settings`.
//!
//! Ver `openspec/changes/strava-running/specs/tools/strava/spec.md`
//! (requisito del estado de la conexión y escenario «Los tokens no viajan en
//! los ajustes»).

mod common;
use common::TestApp;
use serde_json::json;
use valet::db::repos::settings::SettingsRepo;

/// Escribe una clave de `settings` como lo haría el flujo OAuth.
async fn seed(app: &TestApp, key: &str, value: &str) {
    SettingsRepo::set(&app.db, key, value)
        .await
        .expect("seeding a setting must succeed");
}

/// Tras conectar Strava (tokens sembrados en `settings`), `GET /api/settings`
/// NO debe incluir las claves de tokens; el resto de claves sigue saliendo.
#[tokio::test]
async fn get_settings_hides_strava_token_keys() {
    let app = TestApp::new().await;
    seed(&app, "strava_access_token", "SECRET_ACCESS").await;
    seed(&app, "strava_refresh_token", "SECRET_REFRESH").await;

    let body = app
        .get("/api/settings")
        .await
        .json::<serde_json::Value>()
        .await;

    assert!(
        body.get("strava_access_token").is_none(),
        "el access token de Strava no debe viajar en los ajustes: {body}"
    );
    assert!(
        body.get("strava_refresh_token").is_none(),
        "el refresh token de Strava no debe viajar en los ajustes: {body}"
    );

    // Las claves no sensibles siguen presentes (no rompemos la UI).
    assert!(
        body.get("strava_client_id").is_some(),
        "las claves no sensibles deben seguir exponiéndose: {body}"
    );
    assert!(
        body.get("system_prompt").is_some(),
        "las claves no sensibles deben seguir exponiéndose: {body}"
    );
}

/// `PUT /api/settings` con las claves de tokens en el cuerpo NO las modifica:
/// son de gestión interna y un cliente no puede escribirlas.
#[tokio::test]
async fn put_settings_ignores_strava_token_keys() {
    let app = TestApp::new().await;
    seed(&app, "strava_access_token", "ORIGINAL_ACCESS").await;
    seed(&app, "strava_refresh_token", "ORIGINAL_REFRESH").await;

    let resp = app
        .put("/api/settings")
        .json(&json!({
            "strava_access_token": "INJECTED_ACCESS",
            "strava_refresh_token": "INJECTED_REFRESH",
            "max_window_tokens": "12345",
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 200, "el PUT de ajustes debe responder 200");
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body.get("strava_access_token").is_none() && body.get("strava_refresh_token").is_none(),
        "la respuesta del PUT tampoco debe exponer los tokens: {body}"
    );

    // Los tokens conservan sus valores originales…
    assert_eq!(
        SettingsRepo::get(&app.db, "strava_access_token")
            .await
            .unwrap(),
        Some("ORIGINAL_ACCESS".to_string()),
        "el access token no debe poder escribirse desde los ajustes"
    );
    assert_eq!(
        SettingsRepo::get(&app.db, "strava_refresh_token")
            .await
            .unwrap(),
        Some("ORIGINAL_REFRESH".to_string()),
        "el refresh token no debe poder escribirse desde los ajustes"
    );

    // …mientras que una clave no sensible sí se actualiza.
    assert_eq!(
        SettingsRepo::get(&app.db, "max_window_tokens")
            .await
            .unwrap(),
        Some("12345".to_string()),
        "las claves no sensibles deben seguir siendo escribibles"
    );
}

/// `GET /api/settings` NO debe incluir `zennotes_token` (se gestiona por su
/// endpoint dedicado), pero sí `zennotes_base_url` y `zennotes_folder`, que son
/// ajustes no sensibles y editables.
#[tokio::test]
async fn get_settings_hides_zennotes_token_but_keeps_other_zennotes_keys() {
    let app = TestApp::new_empty().await;
    seed(&app, "zennotes_token", "SECRET_ZENNOTES").await;

    let body = app
        .get("/api/settings")
        .await
        .json::<serde_json::Value>()
        .await;

    assert!(
        body.get("zennotes_token").is_none(),
        "el token de ZenNotes no debe viajar en los ajustes: {body}"
    );
    assert!(
        body.get("zennotes_base_url").is_some(),
        "la URL base de ZenNotes es un ajuste no sensible: {body}"
    );
    assert!(
        body.get("zennotes_folder").is_some(),
        "la carpeta de ZenNotes es un ajuste no sensible: {body}"
    );
}

/// `PUT /api/settings` con `zennotes_token` en el cuerpo NO lo modifica: solo
/// `PUT /api/zennotes/token` puede escribirlo.
#[tokio::test]
async fn put_settings_ignores_zennotes_token() {
    let app = TestApp::new_empty().await;
    seed(&app, "zennotes_token", "ORIGINAL_ZENNOTES").await;

    let resp = app
        .put("/api/settings")
        .json(&json!({
            "zennotes_token": "INJECTED_ZENNOTES",
            "max_window_tokens": "12345",
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 200, "el PUT de ajustes debe responder 200");
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body.get("zennotes_token").is_none(),
        "la respuesta del PUT tampoco debe exponer el token: {body}"
    );

    assert_eq!(
        SettingsRepo::get(&app.db, "zennotes_token").await.unwrap(),
        Some("ORIGINAL_ZENNOTES".to_string()),
        "el token de ZenNotes no debe poder escribirse desde los ajustes"
    );
}

/// Scenario: el token no aparece en la lectura de ajustes. `GET /api/settings`
/// NO debe incluir `opentodo_token` (se gestiona por `PUT /api/opentodo/token`),
/// pero sí `opentodo_base_url`, que es un ajuste normal y editable.
#[tokio::test]
async fn get_settings_hides_opentodo_token_but_keeps_base_url() {
    let app = TestApp::new_empty().await;
    seed(&app, "opentodo_token", "SECRET_OPENTODO").await;

    let resp = app.get("/api/settings").await;
    assert_eq!(resp.status(), 200);
    let text = resp.text().await;

    assert!(
        !text.contains("SECRET_OPENTODO"),
        "el token de OpenTodo no debe viajar en los ajustes: {text}"
    );

    let body = serde_json::from_str::<serde_json::Value>(&text).expect("los ajustes son JSON");
    assert!(
        body.get("opentodo_token").is_none(),
        "el token de OpenTodo no debe viajar en los ajustes: {body}"
    );
    assert!(
        body.get("opentodo_base_url").is_some(),
        "la URL base de OpenTodo es un ajuste no sensible: {body}"
    );
}

/// Scenario: el token no se puede escribir por la API de ajustes.
/// `PUT /api/settings` con `opentodo_token` en el cuerpo NO lo modifica, pero
/// `opentodo_base_url` sí se puede editar por esa vía.
#[tokio::test]
async fn put_settings_ignores_opentodo_token() {
    let app = TestApp::new_empty().await;
    seed(&app, "opentodo_token", "ORIGINAL_OPENTODO").await;

    let resp = app
        .put("/api/settings")
        .json(&json!({
            "opentodo_token": "INJECTED_OPENTODO",
            "opentodo_base_url": "https://todo.midominio.es",
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 200, "el PUT de ajustes debe responder 200");
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body.get("opentodo_token").is_none(),
        "la respuesta del PUT tampoco debe exponer el token: {body}"
    );

    assert_eq!(
        SettingsRepo::get(&app.db, "opentodo_token").await.unwrap(),
        Some("ORIGINAL_OPENTODO".to_string()),
        "el token de OpenTodo no debe poder escribirse desde los ajustes"
    );
    assert_eq!(
        SettingsRepo::get(&app.db, "opentodo_base_url")
            .await
            .unwrap(),
        Some("https://todo.midominio.es".to_string()),
        "la URL base de OpenTodo sí se edita desde los ajustes"
    );
}
