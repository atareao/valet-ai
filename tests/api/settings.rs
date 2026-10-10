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
