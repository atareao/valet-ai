//! Handlers HTTP de la integración con OpenTodo.
//!
//! Expone el estado de configuración (`/api/opentodo/status`), el sondeo real de
//! la conexión (`/api/opentodo/check`) y el alta del token
//! (`/api/opentodo/token`). `PUT /api/settings` ignora las claves de
//! [`crate::db::repos::settings::SENSITIVE_KEYS`], así que el token —que sí
//! figura ahí— no se puede guardar por esa vía: estos endpoints dedicados son su
//! única puerta, y **nunca** devuelven el token.
//!
//! Es el mismo contrato de [`crate::handlers::zennotes`].
//!
//! Ver `openspec/changes/opentodo-integration/specs/opentodo-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::opentodo::{Opentodo, OpentodoError};
use crate::AppState;

/// Cuerpo de `GET /api/opentodo/status`. Nunca incluye el token.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub configured: bool,
    pub base_url: String,
}

/// Cuerpo de `GET /api/opentodo/check`: veredicto del sondeo, sin el token.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub detail: String,
}

/// Cuerpo de `PUT /api/opentodo/token`.
#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: Option<String>,
}

/// `GET /api/opentodo/status` — estado de la configuración, sin el token.
///
/// `configured` es `true` cuando hay un token resuelto (en `settings` o en el
/// entorno); `base_url` es la resuelta, con su valor por defecto.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let configured = Opentodo::resolve_token(&state.db).await.is_some();
    let base_url = Opentodo::resolve_base_url(&state.db).await;

    Json(StatusResponse {
        configured,
        base_url,
    })
}

/// `GET /api/opentodo/check` — sondea la conexión **real** con `GET /api/v1/me`.
///
/// Responde siempre `200` con un veredicto legible: que la instancia no
/// responda no es un error del cliente, es precisamente lo que se está
/// averiguando. El detalle nunca contiene el token.
pub async fn check(State(state): State<AppState>) -> Json<CheckResponse> {
    let verdict = match Opentodo::from_settings(&state.db).await {
        Ok(client) => match client.me().await {
            Ok(_) => CheckResponse {
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

/// Traduce el error del sondeo a un detalle en español, sin el token.
///
/// Los dos casos que el usuario tiene que poder distinguir son «falta el token»
/// y «el token no vale»; el resto se resume en que OpenTodo no responde.
fn check_detail(error: &OpentodoError) -> &'static str {
    match error {
        OpentodoError::NotConfigured => "no hay token de OpenTodo configurado",
        OpentodoError::Unauthorized => "el token no es válido o ha caducado",
        _ => "OpenTodo no responde",
    }
}

/// `PUT /api/opentodo/token` — guarda el token recortado; un token vacío o
/// ausente es `400`. Responde `204` sin devolverlo.
pub async fn update_token(
    State(state): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<StatusCode, AppError> {
    let token = body.token.unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "el token de OpenTodo no puede estar vacío".to_string(),
        ));
    }

    SettingsRepo::set(&state.db, "opentodo_token", token).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// `GET /api/opentodo/status` sin token: `configured=false`, `base_url` por
    /// defecto y **sin** el token.
    #[tokio::test]
    #[serial]
    async fn status_reports_not_configured_without_token() {
        // Determinista frente a los tests del servicio, que manipulan el entorno.
        std::env::remove_var("OPENTODO_TOKEN");
        std::env::remove_var("OPENTODO_BASE_URL");
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = status(State(state)).await;

        assert!(!body.configured, "sin token no está configurado");
        assert_eq!(body.base_url, "https://todo.territoriolinux.es");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(json.get("token").is_none(), "el estado no expone el token");
    }

    /// `GET /api/opentodo/status` con token: `configured=true` y sin el token.
    #[tokio::test]
    #[serial]
    async fn status_reports_configured_and_hides_the_token() {
        std::env::remove_var("OPENTODO_TOKEN");
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "opentodo_token", "SECRET")
            .await
            .expect("seed token");

        let Json(body) = status(State(state)).await;

        assert!(body.configured, "con token está configurado");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(json.get("token").is_none(), "el estado no expone el token");
        assert!(
            !json.to_string().contains("SECRET"),
            "el estado no debe filtrar el token: {json}"
        );
    }

    /// `PUT /api/opentodo/token` con token en blanco → `400` y nada guardado.
    #[tokio::test]
    async fn update_token_rejects_blank() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_token(
            State(state.clone()),
            Json(TokenBody {
                token: Some("   ".to_string()),
            }),
        )
        .await
        .expect_err("un token en blanco debe rechazarse");

        assert!(
            matches!(err, AppError::BadRequest(_)),
            "un token en blanco debe ser 400: {err:?}"
        );

        let stored = SettingsRepo::get(&state.db, "opentodo_token")
            .await
            .unwrap();
        assert!(
            stored.unwrap_or_default().is_empty(),
            "un token en blanco no debe guardarse"
        );
    }

    /// `PUT /api/opentodo/token` con token ausente → `400`.
    #[tokio::test]
    async fn update_token_rejects_absent() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_token(State(state), Json(TokenBody { token: None }))
            .await
            .expect_err("un token ausente debe rechazarse");

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
    }

    /// `PUT /api/opentodo/token` con token válido → guardado recortado y `204`.
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
            SettingsRepo::get(&state.db, "opentodo_token")
                .await
                .unwrap(),
            Some("new-token".to_string()),
            "el token debe quedar guardado (recortado)"
        );
    }
}
