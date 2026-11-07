//! Handlers HTTP de la integración con OxiMeal.
//!
//! Expone el estado de configuración (`/api/oximeal/status`), el sondeo real de
//! la conexión (`/api/oximeal/check`) y el alta del secreto M2M
//! (`/api/oximeal/secret`). `PUT /api/settings` ignora las claves de
//! [`crate::db::repos::settings::SENSITIVE_KEYS`], así que el `client_secret`
//! —que figura ahí— no se puede guardar por esa vía: este endpoint dedicado es su
//! única puerta, y **nunca** devuelve el secreto.
//!
//! Es el mismo contrato de [`crate::handlers::opentodo`] y
//! [`crate::handlers::zennotes`].
//!
//! Ver `openspec/changes/oximeal-integration/specs/oximeal-api/spec.md`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::repos::settings::SettingsRepo;
use crate::errors::AppError;
use crate::services::oximeal::{Oximeal, OximealError};
use crate::AppState;

/// Cuerpo de `GET /api/oximeal/status`. Nunca incluye el secreto.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// `true` cuando hay `client_id` y `client_secret` resueltos.
    pub configured: bool,
    /// URL base resuelta, con su valor por defecto.
    pub base_url: String,
    /// Emisor OIDC resuelto (o `AUTH_ISSUER_URL` como respaldo).
    pub issuer_url: String,
    /// `true` cuando hay `client_id` resuelto (aunque falte el secreto).
    pub has_client_id: bool,
}

/// Cuerpo de `GET /api/oximeal/check`: veredicto del sondeo, sin credenciales.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub detail: String,
}

/// Cuerpo de `PUT /api/oximeal/secret`.
#[derive(Deserialize)]
pub struct SecretBody {
    pub client_secret: Option<String>,
}

impl std::fmt::Debug for SecretBody {
    /// Redacta el secreto: nunca debe aparecer en un `Debug` (simétrico con
    /// [`crate::services::oximeal::OximealConfig`]).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretBody")
            .field("client_secret", &"***")
            .finish()
    }
}

/// `GET /api/oximeal/status` — estado de la configuración, sin el secreto.
///
/// `configured` es `true` cuando [`Oximeal::resolve_config`] ve credenciales
/// completas; `has_client_id` distingue «falta el `client_id`» de «solo falta el
/// secreto». `base_url` e `issuer_url` son las resueltas.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let configured = Oximeal::resolve_config(&state.db).await.is_ok();
    let base_url = Oximeal::resolve_base_url(&state.db).await;
    let issuer_url = Oximeal::resolve_issuer_url(&state.db).await;
    let has_client_id = !Oximeal::resolve_client_id(&state.db).await.is_empty();

    Json(StatusResponse {
        configured,
        base_url,
        issuer_url,
        has_client_id,
    })
}

/// `GET /api/oximeal/check` — sondea la conexión **real** con `probe()` (obtiene
/// un token M2M y consulta `recent-recipes`).
///
/// Responde siempre `200` con un veredicto legible: que OxiMeal —o su IdP— no
/// responda no es un error del cliente, es precisamente lo que se averigua. El
/// detalle nunca contiene el secreto ni el token.
pub async fn check(State(state): State<AppState>) -> Json<CheckResponse> {
    let verdict = match Oximeal::from_settings(&state.db).await {
        Ok(client) => match client.probe().await {
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

/// Traduce el error del sondeo a un detalle en español, sin credenciales.
///
/// Distingue las cuatro averías que el usuario tiene que poder separar: falta
/// configuración, el IdP no entregó token, el token fue rechazado y OxiMeal no
/// responde.
///
/// La separación es **estructural** (por variante), no por el texto del error:
/// un `5xx` de OxiMeal que mencione «token» es un fallo suyo, no del IdP.
fn check_detail(error: &OximealError) -> &'static str {
    match error {
        OximealError::NotConfigured => "configuración incompleta",
        OximealError::Unauthorized => "el token fue rechazado",
        // El servicio etiqueta con `TokenEndpoint` los fallos del descubrimiento
        // OIDC y de la petición `client_credentials`.
        OximealError::TokenEndpoint(_) => "el IdP no entregó token",
        _ => "OxiMeal no responde",
    }
}

/// `PUT /api/oximeal/secret` — guarda el secreto recortado; un secreto vacío o
/// ausente es `400`. Responde `204` sin devolverlo.
pub async fn update_secret(
    State(state): State<AppState>,
    Json(body): Json<SecretBody>,
) -> Result<StatusCode, AppError> {
    let secret = body.client_secret.unwrap_or_default();
    let secret = secret.trim();
    if secret.is_empty() {
        return Err(AppError::BadRequest(
            "el client_secret de OxiMeal no puede estar vacío".to_string(),
        ));
    }

    SettingsRepo::set(&state.db, "oximeal_client_secret", secret).await?;

    // El token cacheado se obtuvo con el secreto viejo: se invalida para que la
    // siguiente llamada pida uno nuevo con las credenciales rotadas.
    Oximeal::invalidate_cache();

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::is_sensitive_key;
    use crate::services::oximeal::DEFAULT_BASE_URL;
    use axum::body::Body;
    use axum::http::Request;
    use serial_test::serial;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SECRET: &str = "SECRET";

    /// Segmento de ruta único por test.
    ///
    /// La caché del token es **de proceso** y no se puede limpiar desde aquí
    /// (vive en el módulo del servicio): dar a cada test un emisor OIDC distinto
    /// evita que reutilice un token cacheado por otro servidor que haya ocupado
    /// el mismo puerto efímero.
    fn unique_segment() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQ: AtomicU32 = AtomicU32::new(0);
        format!("/issuer-{}", SEQ.fetch_add(1, Ordering::Relaxed))
    }

    /// Emisor OIDC propio del test: el URI del servidor más su segmento único.
    fn issuer_for(server: &MockServer, segment: &str) -> String {
        format!("{}{segment}", server.uri())
    }

    /// Deja el entorno que influye en la resolución de configuración limpio, para
    /// que los ajustes sembrados sean la única fuente.
    fn clear_env() {
        for key in [
            "OXIMEAL_BASE_URL",
            "OXIMEAL_ISSUER_URL",
            "OXIMEAL_CLIENT_ID",
            "OXIMEAL_CLIENT_SECRET",
            "AUTH_ISSUER_URL",
        ] {
            std::env::remove_var(key);
        }
    }

    /// Siembra una configuración completa apuntando al servidor de prueba.
    async fn seed_config(
        state: &AppState,
        server: &MockServer,
        segment: &str,
        client_id: &str,
        secret: &str,
    ) {
        for (key, value) in [
            ("oximeal_base_url", server.uri()),
            ("oximeal_issuer_url", issuer_for(server, segment)),
            ("oximeal_client_id", client_id.to_string()),
            ("oximeal_client_secret", secret.to_string()),
        ] {
            SettingsRepo::set(&state.db, key, &value)
                .await
                .expect("seed oximeal setting");
        }
    }

    /// Ruta del descubrimiento OIDC dentro del segmento único del test.
    fn discovery_path(segment: &str) -> String {
        format!("{segment}/.well-known/openid-configuration")
    }

    /// Cuántas peticiones ha recibido una ruta concreta.
    async fn count_path(server: &MockServer, target: &str) -> usize {
        server
            .received_requests()
            .await
            .expect("requests")
            .iter()
            .filter(|request| request.url.path() == target)
            .count()
    }

    /// Monta la Agent API con una respuesta vacía y correcta.
    async fn mount_recent_recipes(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .mount(server)
            .await;
    }

    /// Monta el descubrimiento OIDC y el endpoint de token de `client_credentials`.
    async fn mount_auth(server: &MockServer, segment: &str) {
        Mock::given(method("GET"))
            .and(path(discovery_path(segment)))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .mount(server)
            .await;

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok-1",
                "expires_in": 3600,
            })))
            .mount(server)
            .await;
    }

    /// `GET /api/oximeal/status` sin credenciales: `configured=false`,
    /// `has_client_id=false`, `base_url` por defecto y **sin** el secreto.
    #[tokio::test]
    #[serial]
    async fn status_reports_not_configured_without_credentials() {
        clear_env();
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "oximeal_client_id", "")
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_client_secret", "")
            .await
            .unwrap();

        let Json(body) = status(State(state)).await;

        assert!(!body.configured, "sin credenciales no está configurado");
        assert!(!body.has_client_id, "sin client_id no hay credencial");
        assert_eq!(body.base_url, DEFAULT_BASE_URL);

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            json.get("client_secret").is_none() && json.get("secret").is_none(),
            "el estado no debe exponer el secreto"
        );
    }

    /// `GET /api/oximeal/status` con credenciales: `configured=true`,
    /// `has_client_id=true` y **nunca** el secreto.
    #[tokio::test]
    #[serial]
    async fn status_reports_configured_and_hides_the_secret() {
        clear_env();
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "oximeal_base_url", "https://oxi.example.test")
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_issuer_url", "https://idp.example.test")
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_client_id", "client-id")
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_client_secret", SECRET)
            .await
            .unwrap();

        let Json(body) = status(State(state)).await;

        assert!(body.configured, "con credenciales está configurado");
        assert!(body.has_client_id, "hay client_id");
        assert_eq!(body.base_url, "https://oxi.example.test");
        assert_eq!(body.issuer_url, "https://idp.example.test");

        let json = serde_json::to_value(&body).expect("serialize status");
        assert!(
            !json.to_string().contains(SECRET),
            "el estado no debe filtrar el secreto: {json}"
        );
        assert!(
            json.get("client_secret").is_none() && json.get("secret").is_none(),
            "el estado no debe exponer el secreto"
        );
    }

    /// Scenario: sonda correcta con credenciales válidas → `ok: true`.
    #[tokio::test]
    #[serial]
    async fn check_is_ok_with_valid_credentials() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;

        assert!(body.ok, "detail: {}", body.detail);
        assert_eq!(body.detail, "conexión correcta");
    }

    /// Scenario: sin credenciales la sonda lo dice sin tocar la red.
    #[tokio::test]
    #[serial]
    async fn check_reports_incomplete_configuration_without_touching_the_network() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "oximeal_base_url", &server.uri())
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_issuer_url", &server.uri())
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_client_id", "")
            .await
            .unwrap();
        SettingsRepo::set(&state.db, "oximeal_client_secret", "")
            .await
            .unwrap();

        let Json(body) = check(State(state)).await;

        assert_eq!(body.detail, "configuración incompleta");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin credenciales no debe enviar ninguna petición"
        );
    }

    /// Scenario: el IdP que no entrega token se distingue de un OxiMeal caído.
    #[tokio::test]
    #[serial]
    async fn check_distinguishes_a_token_the_idp_did_not_deliver() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        Mock::given(method("GET"))
            .and(path(discovery_path(&segment)))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_client",
            })))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;

        assert_eq!(body.detail, "el IdP no entregó token");
    }

    /// Scenario: un token rechazado (`401`) se distingue de una caída.
    #[tokio::test]
    #[serial]
    async fn check_distinguishes_a_rejected_token() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;

        assert_eq!(body.detail, "el token fue rechazado");
    }

    /// Scenario: un `5xx` de OxiMeal cuyo cuerpo menciona «token» **no** debe
    /// confundirse con un fallo del IdP: el fallo está en la Agent API, no en la
    /// fase de obtención del token.
    #[tokio::test]
    #[serial]
    async fn check_does_not_misclassify_oximeal_5xx_as_idp_failure() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "error": "token bucket saturated",
            })))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;

        assert_eq!(
            body.detail, "OxiMeal no responde",
            "un 5xx de OxiMeal es un fallo suyo, no del IdP"
        );
    }

    /// Scenario: un OxiMeal caído (`5xx`) no es un error de Valet.
    #[tokio::test]
    #[serial]
    async fn check_reports_oximeal_down() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;

        assert_eq!(body.detail, "OxiMeal no responde");
    }

    /// Scenario: la sonda no filtra ni el secreto ni el token.
    #[tokio::test]
    #[serial]
    async fn check_never_leaks_the_secret() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .mount(&server)
            .await;

        let Json(body) = check(State(state)).await;
        let json = serde_json::to_string(&body).expect("serialize check");

        assert!(!json.contains(SECRET), "la sonda filtra el secreto: {json}");
        assert!(!json.contains("tok-1"), "la sonda filtra el token: {json}");
    }

    /// El `Debug` de [`SecretBody`] no debe filtrar el secreto (simétrico con
    /// [`crate::services::oximeal::OximealConfig`]).
    #[test]
    fn secret_body_debug_redacts_the_secret() {
        let body = SecretBody {
            client_secret: Some(SECRET.to_string()),
        };

        let debug = format!("{body:?}");

        assert!(
            !debug.contains(SECRET),
            "el Debug filtra el secreto: {debug}"
        );
        assert!(debug.contains("***"), "el Debug debe redactar: {debug}");
    }

    /// `PUT /api/oximeal/secret` con valor en blanco → `400` y nada guardado.
    #[tokio::test]
    async fn update_secret_rejects_blank() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_secret(
            State(state.clone()),
            Json(SecretBody {
                client_secret: Some("   ".to_string()),
            }),
        )
        .await
        .expect_err("un secreto en blanco debe rechazarse");

        assert!(
            matches!(err, AppError::BadRequest(_)),
            "un secreto en blanco debe ser 400: {err:?}"
        );

        let stored = SettingsRepo::get(&state.db, "oximeal_client_secret")
            .await
            .unwrap();
        assert!(
            stored.unwrap_or_default().is_empty(),
            "un secreto en blanco no debe guardarse"
        );
    }

    /// `PUT /api/oximeal/secret` con el campo ausente → `400`.
    #[tokio::test]
    async fn update_secret_rejects_absent() {
        let state = AppState::new_in_memory_empty().await;

        let err = update_secret(
            State(state),
            Json(SecretBody {
                client_secret: None,
            }),
        )
        .await
        .expect_err("un secreto ausente debe rechazarse");

        assert!(matches!(err, AppError::BadRequest(_)), "err: {err:?}");
    }

    /// `PUT /api/oximeal/secret` con valor válido → guardado recortado y `204`.
    #[tokio::test]
    async fn update_secret_stores_a_valid_secret_trimmed() {
        let state = AppState::new_in_memory_empty().await;

        let status_code = update_secret(
            State(state.clone()),
            Json(SecretBody {
                client_secret: Some("  new-secret  ".to_string()),
            }),
        )
        .await
        .expect("un secreto válido debe aceptarse");

        assert_eq!(status_code, StatusCode::NO_CONTENT, "debe responder 204");
        assert_eq!(
            SettingsRepo::get(&state.db, "oximeal_client_secret")
                .await
                .unwrap(),
            Some("new-secret".to_string()),
            "el secreto debe quedar guardado (recortado)"
        );
    }

    /// Scenario: rotar el secreto invalida la caché del token; la siguiente
    /// llamada vuelve a pedirle uno al IdP (no sirve el obtenido con el secreto
    /// viejo).
    #[tokio::test]
    #[serial]
    async fn update_secret_invalidates_the_token_cache() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        mount_recent_recipes(&server).await;

        let Json(first) = check(State(state.clone())).await;
        assert!(first.ok, "detail: {}", first.detail);
        assert_eq!(count_path(&server, "/token").await, 1, "primer token");

        update_secret(
            State(state.clone()),
            Json(SecretBody {
                client_secret: Some("new-secret".to_string()),
            }),
        )
        .await
        .expect("guardar el secreto nuevo");

        let Json(second) = check(State(state.clone())).await;
        assert!(second.ok, "detail: {}", second.detail);
        assert_eq!(
            count_path(&server, "/token").await,
            2,
            "rotar el secreto debe invalidar la caché y forzar un token nuevo"
        );
    }

    /// Scenario: cambiar el `client_id` (mismo emisor) también invalida la
    /// caché, porque el token cacheado pertenece a otro `client_id`.
    #[tokio::test]
    #[serial]
    async fn changing_client_id_invalidates_the_token_cache() {
        clear_env();
        let server = MockServer::start().await;
        let state = AppState::new_in_memory_empty().await;
        let segment = unique_segment();
        seed_config(&state, &server, &segment, "client-id", SECRET).await;
        mount_auth(&server, &segment).await;
        mount_recent_recipes(&server).await;

        let Json(first) = check(State(state.clone())).await;
        assert!(first.ok, "detail: {}", first.detail);
        assert_eq!(count_path(&server, "/token").await, 1, "primer token");

        SettingsRepo::set(&state.db, "oximeal_client_id", "client-id-2")
            .await
            .unwrap();

        let Json(second) = check(State(state.clone())).await;
        assert!(second.ok, "detail: {}", second.detail);
        assert_eq!(
            count_path(&server, "/token").await,
            2,
            "cambiar el client_id debe forzar un token nuevo"
        );
    }

    /// `oximeal_client_secret` es una clave sensible.
    #[test]
    fn oximeal_client_secret_is_sensitive() {
        assert!(is_sensitive_key("oximeal_client_secret"));
        assert!(!is_sensitive_key("oximeal_base_url"));
        assert!(!is_sensitive_key("oximeal_issuer_url"));
        assert!(!is_sensitive_key("oximeal_client_id"));
    }

    /// Scenario: el secreto no aparece en la lectura de ajustes (router real).
    #[tokio::test]
    async fn settings_endpoint_omits_the_secret() {
        let state = AppState::new_in_memory_empty().await;
        SettingsRepo::set(&state.db, "oximeal_client_secret", SECRET)
            .await
            .unwrap();

        let app = crate::app_with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("router");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(
            !text.contains(SECRET),
            "los ajustes filtran el secreto: {text}"
        );

        let json: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert!(
            json.get("oximeal_client_secret").is_none(),
            "el secreto no debe viajar en los ajustes: {json}"
        );
        assert!(
            json.get("oximeal_base_url").is_some(),
            "las claves no sensibles deben seguir exponiéndose: {json}"
        );
    }
}
