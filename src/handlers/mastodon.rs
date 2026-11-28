//! Handlers HTTP de la integración con Mastodon: estado de configuración,
//! sonda real y alta del token. Nunca devuelven el token.
//!
//! Ver `openspec/changes/mastodon-mentions/specs/mastodon-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::mastodon::{setting_value, Mastodon, MastodonError};
use crate::AppState;

/// Cuerpo de `GET /api/mastodon/status`. Nunca incluye el token.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub configured: bool,
    pub base_url: String,
}

/// Cuerpo de `GET /api/mastodon/check`: veredicto sin el token.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub detail: String,
}

/// Cuerpo de `PUT /api/mastodon/token`.
#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: Option<String>,
}

/// `GET /api/mastodon/status` — estado de la configuración, sin el token.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let token = setting_value(&state.db, "mastodon_token").await;
    let base_url = setting_value(&state.db, "mastodon_base_url").await;
    Json(StatusResponse {
        configured: token.is_some(),
        base_url: base_url.unwrap_or_default(),
    })
}

/// `GET /api/mastodon/check` — sondeo real contra notificaciones, siempre `200`.
pub async fn check(State(state): State<AppState>) -> Json<CheckResponse> {
    let verdict = match Mastodon::from_settings(&state.db).await {
        Ok(client) => match client.check_connection().await {
            Ok(()) => CheckResponse {
                ok: true,
                detail: "conexión correcta".to_string(),
            },
            Err(error) => CheckResponse {
                ok: false,
                detail: check_detail(&error).to_string(),
            },
        },
        Err(error) => CheckResponse {
            ok: false,
            detail: check_detail(&error).to_string(),
        },
    };
    Json(verdict)
}

/// Detalle accionable del sondeo. Ninguna rama incluye el token.
fn check_detail(error: &MastodonError) -> String {
    match error {
        MastodonError::NotConfigured => "configuración incompleta".to_string(),
        MastodonError::Unauthorized => "el token de Mastodon no es válido".to_string(),
        other => other.to_string(),
    }
}

/// `PUT /api/mastodon/token` — guarda el token recortado; vacío o ausente → `400`.
pub async fn update_token(
    State(state): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<StatusCode, AppError> {
    let token = body.token.unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "el token de Mastodon no puede estar vacío".to_string(),
        ));
    }
    SettingsRepo::set(&state.db, "mastodon_token", token).await?;
    Ok(StatusCode::NO_CONTENT)
}
