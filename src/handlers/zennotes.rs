//! Handlers HTTP de la integración con ZenNotes.
//!
//! Expone el estado de configuración (`/api/zennotes/status`) y el alta del
//! token (`/api/zennotes/token`). `PUT /api/settings` ignora las claves de
//! [`crate::db::repos::settings::SENSITIVE_KEYS`], así que un token oculto no se
//! puede guardar por esa vía: estos endpoints dedicados son su única puerta, y
//! **nunca** devuelven el token.
//!
//! Ver `openspec/changes/zennotes-notes/specs/zennotes-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::zennotes::Zennotes;
use crate::AppState;

/// Cuerpo de `GET /api/zennotes/status`. Nunca incluye el token.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub configured: bool,
    pub base_url: String,
    pub folder: String,
}

/// Cuerpo de `PUT /api/zennotes/token`.
#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: Option<String>,
}

/// `GET /api/zennotes/status` — estado de la configuración, sin el token.
///
/// `configured` es `true` cuando hay un token resuelto (en `settings` o en el
/// entorno); `base_url` y `folder` son los valores resueltos con sus defaults.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let configured = Zennotes::resolve_token(&state.db).await.is_some();
    let base_url = Zennotes::resolve_base_url(&state.db).await;
    let folder = Zennotes::resolve_folder(&state.db).await;

    Json(StatusResponse {
        configured,
        base_url,
        folder,
    })
}

/// `PUT /api/zennotes/token` — guarda el token; un token vacío o ausente es
/// `400`. Responde `204` sin devolverlo.
pub async fn update_token(
    State(state): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<StatusCode, AppError> {
    let token = body.token.unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "el token de ZenNotes no puede estar vacío".to_string(),
        ));
    }

    SettingsRepo::set(&state.db, "zennotes_token", token).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// `GET /api/zennotes/status` sin token: `configured=false`, con `base_url` y
    /// `folder` por defecto y **sin** el token.
    #[tokio::test]
    #[serial]
    async fn status_reports_not_configured_without_token() {
        // Determinista frente a los tests del servicio, que manipulan el entorno.
        std::env::remove_var("ZENNOTES_TOKEN");
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = status(State(state)).await;

        assert!(!body.configured, "sin token no está configurada");
        assert_eq!(body.base_url, "https://zennotes.territoriolinux.es");
        assert_eq!(body.folder, "Valet");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(json.get("token").is_none(), "el estado no expone el token");
    }

    /// `GET /api/zennotes/status` con token: `configured=true` y sin el token.
    #[tokio::test]
    #[serial]
    async fn status_reports_configured_and_hides_the_token() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "zennotes_token", "SECRET")
            .await
            .expect("seed token");

        let Json(body) = status(State(state.clone())).await;

        assert!(body.configured, "con token está configurada");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(json.get("token").is_none(), "el estado no expone el token");
        assert!(
            !json.to_string().contains("SECRET"),
            "el estado no debe filtrar el token: {json}"
        );
    }

    /// `PUT /api/zennotes/token` con token vacío → `400`.
    #[tokio::test]
    async fn update_token_rejects_empty() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_token(
            State(state.clone()),
            Json(TokenBody {
                token: Some("   ".to_string()),
            }),
        )
        .await
        .expect_err("un token vacío debe rechazarse");

        assert!(
            matches!(err, AppError::BadRequest(_)),
            "un token vacío debe ser 400: {err:?}"
        );

        // Y no debe haber guardado nada.
        let stored = SettingsRepo::get(&state.db, "zennotes_token")
            .await
            .unwrap();
        assert!(
            stored.unwrap_or_default().is_empty(),
            "un token vacío no debe guardarse"
        );
    }

    /// `PUT /api/zennotes/token` con token ausente → `400`.
    #[tokio::test]
    async fn update_token_rejects_absent() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_token(State(state), Json(TokenBody { token: None }))
            .await
            .expect_err("un token ausente debe rechazarse");

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
    }

    /// `PUT /api/zennotes/token` con token válido → guardado y `204`.
    #[tokio::test]
    async fn update_token_stores_a_valid_token() {
        let state = AppState::new_in_memory_empty().await;

        let status_code = update_token(
            State(state.clone()),
            Json(TokenBody {
                token: Some("  new-token  ".to_string()),
            }),
        )
        .await
        .expect("un token válido debe aceptarse");

        assert_eq!(status_code, StatusCode::NO_CONTENT, "debe responder 204");
        assert_eq!(
            SettingsRepo::get(&state.db, "zennotes_token")
                .await
                .unwrap(),
            Some("new-token".to_string()),
            "el token debe quedar guardado (recortado)"
        );
    }
}
