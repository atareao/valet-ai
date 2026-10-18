//! Rutas HTTP del transporte WebPush (`/api/push`).
//!
//! Tres endpoints: la clave pública VAPID para que el navegador se suscriba,
//! el alta de la suscripción del navegador y su baja idempotente. La
//! criptografía y la persistencia viven en [`crate::services::push`] y
//! [`crate::db::repos::push`]; aquí solo se valida la entrada y se traduce el
//! resultado a HTTP.
//!
//! Ver `openspec/changes/webpush-notifications/specs/push/spec.md`.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use serde::Deserialize;

use crate::db::repos::push::PushRepo;
use crate::errors::AppError;
use crate::services::push::load_or_generate;
use crate::AppState;

/// Cuerpo de `POST /api/push/subscriptions`.
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    /// URL absoluta `https` del Push Service.
    pub endpoint: String,
    /// Claves de la suscripción del navegador.
    pub keys: SubscriptionKeys,
}

/// Claves de cifrado de la suscripción.
#[derive(Debug, Deserialize)]
pub struct SubscriptionKeys {
    /// Clave pública del navegador (65 bytes `uncompressed` en base64url).
    pub p256dh: String,
    /// Secreto de autenticación (16 bytes en base64url).
    pub auth: String,
}

/// Cuerpo de `DELETE /api/push/subscriptions`.
#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    /// Endpoint a dar de baja. Opcional para poder devolver `400` si falta.
    pub endpoint: Option<String>,
}

/// Registra las tres rutas de `/api/push`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/push/vapid-key", get(vapid_key))
        .route(
            "/api/push/subscriptions",
            post(subscribe).delete(unsubscribe),
        )
}

/// `GET /api/push/vapid-key` — devuelve la clave pública VAPID en base64url.
///
/// `200` con `{"public_key": "..."}` si las claves se cargan o generan; `503`
/// con `{"error": "push_unavailable"}` en caso contrario. La respuesta nunca
/// incluye la clave privada.
async fn vapid_key(State(state): State<AppState>) -> Response {
    match load_or_generate(&state.db).await {
        Ok(keys) => (
            StatusCode::OK,
            Json(serde_json::json!({ "public_key": keys.public_key })),
        )
            .into_response(),
        Err(error) => {
            // El mensaje del error nunca contiene material clave (ver
            // `services::push`); aun así no se propaga al cliente.
            tracing::warn!(error = %error, "no se pudieron cargar las claves VAPID");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "push_unavailable" })),
            )
                .into_response()
        }
    }
}

/// `POST /api/push/subscriptions` — valida y registra la suscripción.
///
/// `201` con `{"id": "<uuid>"}`; `400` con `{"error": ...}` si alguna validación
/// falla. El alta es un *upsert* por `endpoint` que conserva el `id` original.
async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubscribeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let endpoint = validate_endpoint(&payload.endpoint)?;
    validate_p256dh(&payload.keys.p256dh)?;
    validate_auth(&payload.keys.auth)?;

    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok());

    let id = PushRepo::upsert(
        &state.db,
        &endpoint,
        &payload.keys.p256dh,
        &payload.keys.auth,
        user_agent,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// `DELETE /api/push/subscriptions` — da de baja la suscripción por `endpoint`.
///
/// `204` siempre (exista o no la fila); `400` si falta `endpoint` en el cuerpo.
async fn unsubscribe(
    State(state): State<AppState>,
    Json(payload): Json<UnsubscribeRequest>,
) -> Result<StatusCode, AppError> {
    let endpoint = payload
        .endpoint
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("endpoint is required".to_string()))?;

    PushRepo::delete_by_endpoint(&state.db, &endpoint).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Comprueba que `endpoint` sea una URL absoluta con esquema `https` y devuelve
/// su forma **normalizada** por [`url::Url`].
///
/// Se guarda la cadena normalizada, no la cruda: `Url` y [`http::Uri`] no
/// coinciden en qué aceptan (un espacio o un carácter UTF-8 en el path, por
/// ejemplo, `Url` los por ciento-codifica y `Uri` los rechaza). Almacenar la
/// forma de `Url` garantiza que lo que pasó el alta con `201` se podrá volver a
/// parsear en `build_request` para cada envío.
fn validate_endpoint(endpoint: &str) -> Result<String, AppError> {
    let url = url::Url::parse(endpoint)
        .map_err(|_| AppError::BadRequest("endpoint must be an absolute https URL".to_string()))?;
    if url.scheme() != "https" {
        return Err(AppError::BadRequest(
            "endpoint must use the https scheme".to_string(),
        ));
    }
    Ok(url.as_str().to_string())
}

/// Comprueba que `p256dh` decodifique (base64url sin padding) a 65 bytes y que
/// esos bytes formen un punto válido de la curva P-256.
///
/// La longitud por sí sola no basta: un blob de 65 bytes que no sea un punto
/// SEC1 válido se guardaría y solo reventaría en cada envío, con un `201`
/// mentiroso. Validando aquí el punto con
/// [`web_push_native::p256::PublicKey::from_sec1_bytes`], un valor inválido se
/// rechaza en el alta con `400`.
fn validate_p256dh(p256dh: &str) -> Result<(), AppError> {
    let raw = engine()
        .decode(p256dh)
        .map_err(|_| AppError::BadRequest("p256dh must be valid base64url".to_string()))?;
    if raw.len() != 65 {
        return Err(AppError::BadRequest(
            "p256dh must decode to 65 bytes".to_string(),
        ));
    }
    web_push_native::p256::PublicKey::from_sec1_bytes(&raw).map_err(|_| {
        AppError::BadRequest("p256dh must be a valid uncompressed P-256 point".to_string())
    })?;
    Ok(())
}

/// Comprueba que `auth` decodifique (base64url sin padding) a 16 bytes.
fn validate_auth(auth: &str) -> Result<(), AppError> {
    let raw = engine()
        .decode(auth)
        .map_err(|_| AppError::BadRequest("auth must be valid base64url".to_string()))?;
    if raw.len() != 16 {
        return Err(AppError::BadRequest(
            "auth must decode to 16 bytes".to_string(),
        ));
    }
    Ok(())
}

/// Motor base64url sin padding, compartido con [`crate::services::push`].
fn engine() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}
