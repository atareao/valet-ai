//! Handlers HTTP de la integración con GitHub.
//!
//! Expone el estado de configuración (`/api/github/status`), el sondeo real de
//! la conexión (`/api/github/check`) y el alta del Personal Access Token
//! (`/api/github/token`). `PUT /api/settings` ignora las claves de
//! [`crate::db::repos::settings::SENSITIVE_KEYS`], así que `github_token` —que sí
//! figura ahí— no se puede guardar por esa vía: este endpoint dedicado es su
//! única puerta, y **nunca** devuelve el token.
//!
//! Es el mismo contrato de [`crate::handlers::blog`] y
//! [`crate::handlers::oximeal`].
//!
//! Ver `openspec/changes/github-integration/specs/github-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::github::{setting_value, Github, GithubError, API_BASE_URL, DEFAULT_REPO};
use crate::AppState;

/// Cuerpo de `GET /api/github/status`. Nunca incluye el token.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// `true` cuando `github_token` tiene valor no vacío.
    pub configured: bool,
    /// El repositorio por defecto resuelto (`github_default_repo`, def.
    /// [`DEFAULT_REPO`]).
    pub default_repo: String,
}

/// Cuerpo de `GET /api/github/check`: veredicto del sondeo, sin el token.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub detail: String,
}

/// Cuerpo de `PUT /api/github/token`.
#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: Option<String>,
}

/// `GET /api/github/status` — estado de la configuración, sin el token.
///
/// `configured` es `true` solo cuando `github_token` tiene valor no vacío;
/// `default_repo` es el resuelto, con su valor por defecto.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let token = setting_value(&state.db, "github_token").await;
    let default_repo = setting_value(&state.db, "github_default_repo")
        .await
        .unwrap_or_else(|| DEFAULT_REPO.to_string());

    Json(StatusResponse {
        configured: token.is_some(),
        default_repo,
    })
}

/// `GET /api/github/check` — sondea la conexión **real** con `GET /user`.
///
/// Responde siempre `200` con un veredicto legible: que GitHub no responda no es
/// un error del cliente, es precisamente lo que se está averiguando. El detalle
/// nunca contiene el token.
pub async fn check(State(state): State<AppState>) -> Json<CheckResponse> {
    check_with_base(&state.db, API_BASE_URL).await
}

/// Sonda con la URL base inyectable: el camino de producción pasa
/// [`API_BASE_URL`]; los tests pasan un servidor `wiremock`. El veredicto nunca
/// contiene el token.
async fn check_with_base(db: &SqlitePool, base_url: &str) -> Json<CheckResponse> {
    let verdict = match Github::resolve_config(db).await {
        Ok(config) => {
            let client = Github::with_base_url(base_url.to_string(), config);
            match client.check_connection().await {
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

/// Traduce el error del sondeo a un detalle en español, sin el token.
///
/// Los tres casos que el usuario tiene que poder distinguir son «falta el
/// token», «el token no vale» y «GitHub no responde».
fn check_detail(error: &GithubError) -> &'static str {
    match error {
        GithubError::NotConfigured => "configuración incompleta",
        GithubError::Unauthorized => "el token fue rechazado",
        _ => "GitHub no responde",
    }
}

/// `PUT /api/github/token` — guarda el token recortado; un token vacío o ausente
/// es `400`. Responde `204` sin devolverlo.
pub async fn update_token(
    State(state): State<AppState>,
    Json(body): Json<TokenBody>,
) -> Result<StatusCode, AppError> {
    let token = body.token.unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "el Personal Access Token de GitHub no puede estar vacío".to_string(),
        ));
    }

    SettingsRepo::set(&state.db, "github_token", token).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed(state: &AppState, key: &str, value: &str) {
        SettingsRepo::set(&state.db, key, value)
            .await
            .expect("sembrar ajuste");
    }

    /// `GET /api/github/status` sin token: `configured=false`, con el repositorio
    /// por defecto y **sin** el token.
    #[tokio::test]
    async fn status_reports_not_configured_without_token() {
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = status(State(state)).await;

        assert!(!body.configured, "sin token no está configurado");
        assert_eq!(body.default_repo, DEFAULT_REPO);

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            json.get("token").is_none(),
            "el estado no expone el token: {json}"
        );
    }

    /// `GET /api/github/status` con token: `configured=true` y sin el token.
    #[tokio::test]
    async fn status_reports_configured_and_hides_the_token() {
        let state = AppState::new_in_memory_empty().await;
        seed(&state, "github_token", "SECRET").await;
        seed(&state, "github_default_repo", "otro/repo").await;

        let Json(body) = status(State(state)).await;

        assert!(body.configured, "con token está configurado");
        assert_eq!(body.default_repo, "otro/repo");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            !json.to_string().contains("SECRET"),
            "el estado no debe filtrar el token: {json}"
        );
    }

    /// `PUT /api/github/token` en blanco → `400` y nada guardado.
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

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
        assert!(
            SettingsRepo::get(&state.db, "github_token")
                .await
                .unwrap()
                .unwrap_or_default()
                .is_empty(),
            "un token en blanco no debe guardarse"
        );
    }

    /// `PUT /api/github/token` sin `token` → `400`.
    #[tokio::test]
    async fn update_token_rejects_absent() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_token(State(state), Json(TokenBody { token: None }))
            .await
            .expect_err("sin `token` debe rechazarse");

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
    }

    /// `PUT /api/github/token` válido → guardado recortado y `204`.
    #[tokio::test]
    async fn update_token_stores_it_trimmed() {
        let state = AppState::new_in_memory_empty().await;

        let code = update_token(
            State(state.clone()),
            Json(TokenBody {
                token: Some("  ghp_secret  ".to_string()),
            }),
        )
        .await
        .expect("un token válido debe aceptarse");

        assert_eq!(code, StatusCode::NO_CONTENT, "debe responder 204");
        assert_eq!(
            SettingsRepo::get(&state.db, "github_token").await.unwrap(),
            Some("ghp_secret".to_string()),
            "el token debe quedar guardado recortado"
        );
    }

    /// `GET /api/github/check` sin token → `200` con `ok: false` y sin tocar la
    /// red.
    #[tokio::test]
    async fn check_reports_missing_configuration_without_network() {
        let state = AppState::new_in_memory_empty().await;

        let Json(body) = check(State(state)).await;

        assert!(!body.ok);
        assert_eq!(body.detail, "configuración incompleta");
    }

    /// La traducción de errores del sondeo: tres mensajes distinguibles y sin el
    /// token.
    #[test]
    fn check_detail_distinguishes_the_outcomes() {
        assert_eq!(
            check_detail(&GithubError::NotConfigured),
            "configuración incompleta"
        );
        assert_eq!(
            check_detail(&GithubError::Unauthorized),
            "el token fue rechazado"
        );
        assert_eq!(
            check_detail(&GithubError::Upstream("caída".to_string())),
            "GitHub no responde"
        );
        assert_eq!(
            check_detail(&GithubError::NotFound),
            "GitHub no responde",
            "un 404 no es un token rechazado ni una configuración ausente"
        );
    }

    // ─── sonda e2e (base inyectable) ─────────────────────────────────────────

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = "ghp_SECRET";

    /// Serializa el veredicto y comprueba que no filtra el token.
    fn assert_no_token_leak(body: &CheckResponse) {
        let json = serde_json::to_string(body).expect("serialize check response");
        assert!(
            !json.contains(TOKEN) && !json.contains("SECRET"),
            "la sonda no debe filtrar el token: {json}"
        );
    }

    /// Scenario: Sonda correcta con un token válido.
    #[tokio::test]
    async fn check_with_valid_token_reports_ok() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "atareao",
            })))
            .mount(&server)
            .await;

        let state = AppState::new_in_memory_empty().await;
        seed(&state, "github_token", TOKEN).await;

        let Json(body) = check_with_base(&state.db, &server.uri()).await;

        assert!(body.ok, "un token válido debe dar ok");
        assert_eq!(body.detail, "conexión correcta");
        assert_no_token_leak(&body);
    }

    /// Scenario: Un token rechazado se distingue de una caída.
    #[tokio::test]
    async fn check_with_rejected_token_says_it_was_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let state = AppState::new_in_memory_empty().await;
        seed(&state, "github_token", TOKEN).await;

        let Json(body) = check_with_base(&state.db, &server.uri()).await;

        assert!(!body.ok, "un token rechazado no es ok");
        assert_eq!(body.detail, "el token fue rechazado");
        assert_no_token_leak(&body);
    }

    /// Scenario: Un GitHub caído no es un error de Valet — la sonda responde
    /// igualmente `ok: false` con «GitHub no responde», sin filtrar el token.
    #[tokio::test]
    async fn check_with_a_down_site_reports_it_does_not_respond() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let state = AppState::new_in_memory_empty().await;
        seed(&state, "github_token", TOKEN).await;

        let Json(body) = check_with_base(&state.db, &server.uri()).await;

        assert!(!body.ok, "un 5xx no es ok");
        assert_eq!(body.detail, "GitHub no responde");
        assert_no_token_leak(&body);
    }
}
