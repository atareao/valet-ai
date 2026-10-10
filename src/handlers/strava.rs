//! Handlers HTTP de la integración con Strava.
//!
//! Expone el flujo OAuth2 (`/api/strava/authorize` y `/api/strava/callback`), el
//! estado de la conexión (`/api/strava/status`) y la desconexión
//! (`/api/strava/disconnect`). La lógica de OAuth, persistencia y refresco vive
//! en [`crate::services::strava`]; estos handlers solo resuelven la
//! `redirect_uri`, traducen errores y componen la respuesta.
//!
//! Ver `openspec/changes/strava-running/specs/tools/strava/spec.md`.

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::services::strava::{Strava, StravaError};
use crate::AppState;

/// Variable de entorno que fija la `redirect_uri` del callback.
const REDIRECT_URL_ENV: &str = "STRAVA_REDIRECT_URL";
/// Ruta del callback, anexada al origen derivado de las cabeceras.
const CALLBACK_PATH: &str = "/api/strava/callback";
/// Origen por defecto cuando la petición no trae `Host`.
const DEFAULT_ORIGIN: &str = "localhost:3000";

/// Parámetros de query que Strava envía al callback.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Cuerpo de `GET /api/strava/status`. Nunca incluye tokens.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub connected: bool,
    pub athlete_id: Option<String>,
    pub athlete_name: Option<String>,
    pub scope: Option<String>,
}

/// Cuerpo de `POST /api/strava/disconnect`.
#[derive(Debug, Serialize)]
pub struct DisconnectResponse {
    pub connected: bool,
    /// Aviso para el usuario si la revocación remota no se pudo confirmar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Cuerpo de `GET /api/strava/check`. Nunca incluye tokens.
///
/// El sondeo responde **siempre** `200`: el resultado viaja en `ok`, y el motivo
/// del fallo (si lo hay) en `error`.
#[derive(Debug, Serialize)]
pub struct StravaCheckResponse {
    pub ok: bool,
    pub athlete_id: Option<String>,
    pub athlete_name: Option<String>,
    pub error: Option<String>,
}

/// `GET /api/strava/authorize` — redirige a la autorización de Strava.
pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let redirect_url = redirect_uri(&headers);
    let url = Strava::new()
        .authorize_url(&state.db, &redirect_url)
        .await
        .map_err(map_strava_error)?;
    Ok(redirect_to(&url))
}

/// `GET /api/strava/callback` — canjea el `code` y redirige al frontend.
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, AppError> {
    let redirect_url = redirect_uri(&headers);

    // Se llama **siempre** a `handle_callback` para que el `state` se valide y
    // se consuma en ambos caminos. Cuando Strava deniega el acceso, el `code`
    // llega vacío y el error viaja en `error=access_denied`: se pasa ese
    // literal como `code` para que el servicio distinga la denegación del
    // canje normal.
    let code = match query.error.as_deref() {
        Some("access_denied") => "access_denied".to_string(),
        _ => query.code.unwrap_or_default(),
    };
    let state_param = query.state.unwrap_or_default();

    let location = match Strava::new()
        .handle_callback(&state.db, &code, &state_param, &redirect_url)
        .await
    {
        Ok(()) => "/?strava=ok",
        Err(StravaError::Denied) => "/?strava=denied",
        Err(_) => "/?strava=error",
    };

    Ok(redirect_to(location))
}

/// `GET /api/strava/status` — estado de la conexión, sin tokens.
pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponse>, AppError> {
    let status = Strava::new()
        .status(&state.db)
        .await
        .map_err(map_strava_error)?;

    Ok(Json(StatusResponse {
        connected: status.connected,
        athlete_id: status.athlete_id,
        athlete_name: status.athlete_name,
        scope: status.scope,
    }))
}

/// `POST /api/strava/disconnect` — revoca en Strava y limpia los tokens.
pub async fn disconnect(
    State(state): State<AppState>,
) -> Result<Json<DisconnectResponse>, AppError> {
    let outcome = Strava::new()
        .disconnect(&state.db)
        .await
        .map_err(map_strava_error)?;

    Ok(Json(DisconnectResponse {
        connected: false,
        warning: outcome.warning,
    }))
}

/// `GET /api/strava/check` — sondeo activo del estado real de la conexión.
///
/// Responde siempre `200`; el diagnóstico viaja en el cuerpo y nunca se exponen
/// tokens.
pub async fn check(State(state): State<AppState>) -> Json<StravaCheckResponse> {
    match Strava::new().check(&state.db).await {
        Ok(check) => Json(StravaCheckResponse {
            ok: true,
            athlete_id: check.athlete_id,
            athlete_name: check.athlete_name,
            error: None,
        }),
        Err(error) => Json(StravaCheckResponse {
            ok: false,
            athlete_id: None,
            athlete_name: None,
            error: Some(error.to_string()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resuelve la `redirect_uri` del callback.
///
/// Prioriza `STRAVA_REDIRECT_URL`; si no, la compone desde las cabeceras
/// (`x-forwarded-proto`/`x-forwarded-host`, con `host` como respaldo).
fn redirect_uri(headers: &HeaderMap) -> String {
    if let Some(configured) = std::env::var(REDIRECT_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return configured;
    }

    let proto = header_value(headers, "x-forwarded-proto").unwrap_or_else(|| "http".to_string());
    let host = header_value(headers, "x-forwarded-host")
        .or_else(|| header_value(headers, "host"))
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string());

    format!("{proto}://{host}{CALLBACK_PATH}")
}

/// Primer valor no vacío de una cabecera, como `String`.
fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Respuesta `302 Found` con la cabecera `Location`.
fn redirect_to(location: &str) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
}

/// Traduce un [`StravaError`] en la respuesta HTTP correspondiente.
///
/// Los fallos de configuración o de red se tratan como errores internos; los
/// fallos de contrato OAuth (`state` inválido, denegación) como `400`.
fn map_strava_error(error: StravaError) -> AppError {
    match &error {
        StravaError::MissingCredentials
        | StravaError::Http(_)
        | StravaError::RateLimited
        | StravaError::Internal(_)
        | StravaError::ApplicationInactive => AppError::Internal(error.to_string()),
        StravaError::NotConnected | StravaError::InvalidState | StravaError::Denied => {
            AppError::BadRequest(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::SettingsRepo;

    /// Cabecera `Location` de una respuesta de redirección.
    fn location(response: &Response) -> String {
        response
            .headers()
            .get(header::LOCATION)
            .expect("location header")
            .to_str()
            .expect("location is ascii")
            .to_string()
    }

    /// `GET /api/strava/status` sobre una base limpia: no conectada y sin tokens.
    #[tokio::test]
    async fn status_reports_not_connected_by_default() {
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = status(State(state)).await.expect("status ok");

        assert!(!body.connected, "una base limpia no está conectada");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(json.get("access_token").is_none(), "no expone access token");
        assert!(
            json.get("refresh_token").is_none(),
            "no expone refresh token"
        );
        assert!(
            json.get("athlete_name")
                .is_some_and(|value| value.is_null()),
            "sin conexión no hay nombre de atleta"
        );
    }

    /// `GET /api/strava/status` refleja el nombre del atleta persistido.
    #[tokio::test]
    async fn status_reports_the_athlete_name() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "strava_refresh_token", "REF")
            .await
            .expect("seed refresh token");
        SettingsRepo::set(&state.db, "strava_athlete_name", "Jane Doe")
            .await
            .expect("seed athlete name");

        let Json(body) = status(State(state)).await.expect("status ok");

        assert!(body.connected, "con refresh token está conectada");
        assert_eq!(body.athlete_name.as_deref(), Some("Jane Doe"));

        let json = serde_json::to_value(&body).expect("serialize status");
        assert_eq!(json["athlete_name"], "Jane Doe");
    }

    /// `GET /api/strava/authorize` redirige a Strava con scope de solo lectura.
    #[tokio::test]
    async fn authorize_redirects_to_strava_with_read_only_scope() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "strava_client_id", "cid")
            .await
            .expect("seed client_id");
        SettingsRepo::set(&state.db, "strava_client_secret", "csecret")
            .await
            .expect("seed client_secret");

        let response = authorize(State(state), HeaderMap::new())
            .await
            .expect("authorize ok");

        assert_eq!(response.status(), StatusCode::FOUND, "debe ser 302");
        let location = location(&response);
        assert!(
            location.starts_with("https://www.strava.com/oauth/authorize"),
            "location: {location}"
        );
        assert!(
            location.contains("scope=read,activity:read_all"),
            "location: {location}"
        );
    }

    /// Un callback con `state` inválido redirige a `/?strava=error`.
    #[tokio::test]
    async fn callback_with_invalid_state_redirects_to_error() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "strava_client_id", "cid")
            .await
            .expect("seed client_id");
        SettingsRepo::set(&state.db, "strava_client_secret", "csecret")
            .await
            .expect("seed client_secret");

        let query = CallbackQuery {
            code: Some("thecode".to_string()),
            state: Some("bogus-state".to_string()),
            error: None,
        };
        let response = callback(State(state), HeaderMap::new(), Query(query))
            .await
            .expect("callback ok");

        assert_eq!(response.status(), StatusCode::FOUND, "debe ser 302");
        assert!(
            location(&response).contains("strava=error"),
            "location: {}",
            location(&response)
        );
    }

    /// Extrae el `state` de una URL de redirección.
    fn state_from(location: &str) -> String {
        url::Url::parse(location)
            .expect("valid url")
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .expect("state in authorize url")
    }

    /// Una denegación con `state` válido consume el `state`, redirige a
    /// `/?strava=denied` y no deja tokens.
    #[tokio::test]
    async fn callback_with_access_denied_redirects_to_denied_and_consumes_state() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "strava_client_id", "cid")
            .await
            .expect("seed client_id");
        SettingsRepo::set(&state.db, "strava_client_secret", "csecret")
            .await
            .expect("seed client_secret");

        let auth_response = authorize(State(state.clone()), HeaderMap::new())
            .await
            .expect("authorize ok");
        let state_param = state_from(&location(&auth_response));

        let query = CallbackQuery {
            code: None,
            state: Some(state_param.clone()),
            error: Some("access_denied".to_string()),
        };
        let response = callback(State(state.clone()), HeaderMap::new(), Query(query))
            .await
            .expect("callback ok");

        assert_eq!(response.status(), StatusCode::FOUND, "debe ser 302");
        assert!(
            location(&response).contains("strava=denied"),
            "location: {}",
            location(&response)
        );

        // El `state` se consumió: reutilizarlo cae en el camino de error.
        let replay = CallbackQuery {
            code: Some("thecode".to_string()),
            state: Some(state_param),
            error: None,
        };
        let response = callback(State(state.clone()), HeaderMap::new(), Query(replay))
            .await
            .expect("callback ok");
        assert!(
            location(&response).contains("strava=error"),
            "un state reutilizado debe fallar: {}",
            location(&response)
        );

        let refresh = SettingsRepo::get(&state.db, "strava_refresh_token")
            .await
            .expect("read refresh token");
        assert!(
            refresh.unwrap_or_default().is_empty(),
            "una denegación no debe guardar tokens"
        );
    }

    /// `disconnect` deja la conexión como no conectada y vacía los tokens.
    #[tokio::test]
    async fn disconnect_clears_the_tokens() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "strava_refresh_token", "REF")
            .await
            .expect("seed refresh token");
        SettingsRepo::set(&state.db, "strava_access_token", "ACC")
            .await
            .expect("seed access token");

        let Json(body) = disconnect(State(state.clone()))
            .await
            .expect("disconnect ok");

        assert!(!body.connected, "tras desconectar no está conectada");
        let refresh = SettingsRepo::get(&state.db, "strava_refresh_token")
            .await
            .expect("read refresh token");
        assert!(
            refresh.unwrap_or_default().is_empty(),
            "el refresh token debe quedar vacío"
        );
    }
}
