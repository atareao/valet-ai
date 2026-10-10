use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::db::repos::settings::{is_sensitive_key, SettingsRepo};
use crate::AppState;

/// `GET /api/settings` — todos los ajustes **salvo** las claves sensibles.
///
/// Las claves de tokens OAuth (`strava_access_token`, `strava_refresh_token`)
/// se omiten siempre: son material sensible de gestión interna y no deben
/// viajar al navegador.
pub async fn get_settings(State(state): State<AppState>) -> Json<HashMap<String, String>> {
    let mut settings = SettingsRepo::get_all(&state.db).await.unwrap_or_default();
    settings.retain(|key, _| !is_sensitive_key(key));

    Json(settings)
}

/// `PUT /api/settings` — actualiza ajustes ignorando las claves sensibles.
///
/// Una clave sensible presente en el cuerpo se descarta: un cliente no puede
/// escribir los tokens OAuth, que solo gestiona el flujo de Strava.
pub async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<HashMap<String, String>>, (StatusCode, Json<JsonValue>)> {
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No settings provided"})),
        ));
    }

    for (key, value) in &body {
        if is_sensitive_key(key) {
            continue;
        }
        SettingsRepo::set(&state.db, key, value)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
    }

    let mut settings = SettingsRepo::get_all(&state.db).await.unwrap_or_default();
    settings.retain(|key, _| !is_sensitive_key(key));
    Ok(Json(settings))
}
