//! Handlers HTTP de la integración con el blog de WordPress.
//!
//! Expone el estado de configuración (`/api/blog/status`), el sondeo real de la
//! conexión (`/api/blog/check`) y el alta de la Application Password
//! (`/api/blog/password`). `PUT /api/settings` ignora las claves de
//! [`crate::db::repos::settings::SENSITIVE_KEYS`], así que `blog_app_password`
//! —que sí figura ahí— no se puede guardar por esa vía: este endpoint dedicado
//! es su única puerta, y **nunca** devuelve la contraseña.
//!
//! Es el mismo contrato de [`crate::handlers::opentodo`].
//!
//! Ver `openspec/changes/wordpress-credentials-settings/specs/blog-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::wordpress::{Wordpress, WordpressError, DEFAULT_BASE_URL};
use crate::AppState;

/// Cuerpo de `GET /api/blog/status`. Nunca incluye la contraseña.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub configured: bool,
    pub base_url: String,
    pub user: String,
}

/// Cuerpo de `GET /api/blog/check`: veredicto del sondeo, sin la contraseña.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub detail: String,
}

/// Cuerpo de `PUT /api/blog/password`.
#[derive(Debug, Deserialize)]
pub struct PasswordBody {
    pub password: Option<String>,
}

/// Lee un ajuste de texto y lo `trim`ea; cadena vacía si falta o está en blanco.
async fn setting(pool: &SqlitePool, key: &str) -> String {
    SettingsRepo::get(pool, key)
        .await
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

/// `GET /api/blog/status` — estado de la configuración, sin la contraseña.
///
/// `configured` es `true` solo cuando el usuario y la contraseña tienen valor no
/// vacío; `base_url` es la resuelta, con su valor por defecto, y `user` el
/// usuario de servicio (nunca la contraseña).
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let base_url = {
        let value = setting(&state.db, "blog_base_url").await;
        if value.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            value
        }
    };
    let user = setting(&state.db, "blog_service_user").await;
    let password = setting(&state.db, "blog_app_password").await;

    Json(StatusResponse {
        configured: !user.is_empty() && !password.is_empty(),
        base_url,
        user,
    })
}

/// `GET /api/blog/check` — sondea la conexión **real** con `GET
/// /wp-json/wp/v2/users/me`.
///
/// Responde siempre `200` con un veredicto legible: que el blog no responda no
/// es un error del cliente, es precisamente lo que se está averiguando. El
/// detalle nunca contiene la contraseña.
pub async fn check(State(state): State<AppState>) -> Json<CheckResponse> {
    let verdict = match Wordpress::resolve_config(&state.db).await {
        Ok(config) => {
            let client = Wordpress::new(config.base_url.clone());
            match client.check_connection(&config).await {
                Ok(()) => CheckResponse {
                    ok: true,
                    detail: "conexión correcta".to_string(),
                },
                Err(error) => CheckResponse {
                    ok: false,
                    detail: check_detail(&error).to_string(),
                },
            }
        }
        Err(error) => CheckResponse {
            ok: false,
            detail: check_detail(&error).to_string(),
        },
    };

    Json(verdict)
}

/// Traduce el error del sondeo a un detalle en español, sin la contraseña.
///
/// Los dos casos que el usuario tiene que poder distinguir son «faltan las
/// credenciales» y «las credenciales no valen»; el resto se resume en que el blog
/// no responde.
fn check_detail(error: &WordpressError) -> &'static str {
    match error {
        WordpressError::MissingCredentials { .. } => {
            "faltan el usuario o la contraseña de aplicación del blog"
        }
        WordpressError::Unauthorized | WordpressError::Forbidden => {
            "el usuario o la contraseña de aplicación no son válidos"
        }
        _ => "el blog no responde",
    }
}

/// `PUT /api/blog/password` — guarda la contraseña recortada; una contraseña
/// vacía o ausente es `400`. Responde `204` sin devolverla.
pub async fn update_password(
    State(state): State<AppState>,
    Json(body): Json<PasswordBody>,
) -> Result<StatusCode, AppError> {
    let password = body.password.unwrap_or_default();
    let password = password.trim();
    if password.is_empty() {
        return Err(AppError::BadRequest(
            "la contraseña de aplicación del blog no puede estar vacía".to_string(),
        ));
    }

    SettingsRepo::set(&state.db, "blog_app_password", password).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// La cabecera `Authorization: Basic base64(usuario:contraseña)` esperada.
    fn basic(user: &str, password: &str) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
        )
    }

    async fn seed(state: &AppState, key: &str, value: &str) {
        SettingsRepo::set(&state.db, key, value)
            .await
            .expect("sembrar ajuste");
    }

    /// `GET /api/blog/status` sin credenciales: `configured=false`, con la URL
    /// por defecto y **sin** la contraseña.
    #[tokio::test]
    async fn status_reports_not_configured_without_credentials() {
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = status(State(state)).await;

        assert!(!body.configured, "sin credenciales no está configurado");
        assert_eq!(body.base_url, "https://atareao.es");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            json.get("app_password").is_none() && json.get("password").is_none(),
            "el estado no expone la contraseña: {json}"
        );
    }

    /// `GET /api/blog/status` con credenciales: `configured=true` y sin la
    /// contraseña.
    #[tokio::test]
    async fn status_reports_configured_and_hides_the_password() {
        let state = AppState::new_in_memory_empty().await;
        seed(&state, "blog_service_user", "alice").await;
        seed(&state, "blog_app_password", "SECRET").await;

        let Json(body) = status(State(state)).await;

        assert!(body.configured, "con credenciales está configurado");
        assert_eq!(body.user, "alice");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            !json.to_string().contains("SECRET"),
            "el estado no debe filtrar la contraseña: {json}"
        );
    }

    /// `PUT /api/blog/password` en blanco → `400` y nada guardado.
    #[tokio::test]
    async fn update_password_rejects_blank() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_password(
            State(state.clone()),
            Json(PasswordBody {
                password: Some("   ".to_string()),
            }),
        )
        .await
        .expect_err("una contraseña en blanco debe rechazarse");

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
        assert!(
            SettingsRepo::get(&state.db, "blog_app_password")
                .await
                .unwrap()
                .unwrap_or_default()
                .is_empty(),
            "una contraseña en blanco no debe guardarse"
        );
    }

    /// `PUT /api/blog/password` válida → guardada recortada y `204`.
    #[tokio::test]
    async fn update_password_stores_it_trimmed() {
        let state = AppState::new_in_memory_empty().await;

        let code = update_password(
            State(state.clone()),
            Json(PasswordBody {
                password: Some("  app-pass  ".to_string()),
            }),
        )
        .await
        .expect("una contraseña válida debe aceptarse");

        assert_eq!(code, StatusCode::NO_CONTENT, "debe responder 204");
        assert_eq!(
            SettingsRepo::get(&state.db, "blog_app_password")
                .await
                .unwrap(),
            Some("app-pass".to_string()),
            "la contraseña debe quedar guardada recortada"
        );
    }

    /// `GET /api/blog/check` con credenciales válidas → `200` con `ok: true`.
    #[tokio::test]
    async fn check_reports_a_working_connection() {
        let state = AppState::new_in_memory_empty().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/users/me"))
            .and(header("authorization", basic("alice", "SECRET").as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1
            })))
            .mount(&server)
            .await;
        seed(&state, "blog_base_url", &server.uri()).await;
        seed(&state, "blog_service_user", "alice").await;
        seed(&state, "blog_app_password", "SECRET").await;

        let Json(body) = check(State(state)).await;

        assert!(body.ok, "una conexión sana debe informar ok: true");
        assert_eq!(body.detail, "conexión correcta");
    }

    /// `GET /api/blog/check` sin credenciales → `200` con `ok: false` y sin tocar
    /// la red.
    #[tokio::test]
    async fn check_reports_missing_credentials_without_network() {
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = check(State(state)).await;

        assert!(!body.ok);
        assert_eq!(
            body.detail,
            "faltan el usuario o la contraseña de aplicación del blog"
        );
    }

    /// `GET /api/blog/check` con `401` → credenciales no válidas, sin filtrar la
    /// contraseña.
    #[tokio::test]
    async fn check_reports_invalid_credentials() {
        let state = AppState::new_in_memory_empty().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/users/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        seed(&state, "blog_base_url", &server.uri()).await;
        seed(&state, "blog_service_user", "alice").await;
        seed(&state, "blog_app_password", "SECRET").await;

        let Json(body) = check(State(state)).await;

        assert!(!body.ok);
        assert_eq!(
            body.detail,
            "el usuario o la contraseña de aplicación no son válidos"
        );
    }

    /// `GET /api/blog/check` con el sitio caído → `200` con `ok: false`.
    #[tokio::test]
    async fn check_reports_an_unreachable_server() {
        let state = AppState::new_in_memory_empty().await;
        seed(&state, "blog_base_url", "http://127.0.0.1:1").await;
        seed(&state, "blog_service_user", "alice").await;
        seed(&state, "blog_app_password", "SECRET").await;

        let Json(body) = check(State(state)).await;

        assert!(!body.ok);
        assert_eq!(body.detail, "el blog no responde");
    }
}
