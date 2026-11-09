//! Cliente de la API REST de GitHub (`POST /repos/{owner}/{repo}/issues`).
//!
//! Crea issues contra la URL **fija** `https://api.github.com` (no hay instancia
//! self-hosted que configurar, así que no existe ningún ajuste que la
//! sobreescriba), autenticándose con un **Personal Access Token** clásico
//! mediante `Authorization: Bearer`. La configuración se resuelve de los ajustes
//! (tabla `settings`: `github_token`, `github_default_repo`,
//! `github_default_labels`) en **cada** llamada, **sin** respaldo de variables de
//! entorno, jamás se guarda en el cliente, y **nunca** aparece en un `Debug`, un
//! log o el mensaje de un [`GithubError`].
//!
//! Es el molde de [`crate::services::wordpress`] y
//! [`crate::services::zennotes`]: base inyectable (solo en tests), `User-Agent`
//! propio, timeout y traducción de cada desenlace a un error tipado. **No se
//! reintenta**: cada invocación envía una sola petición, también ante un `429` o
//! un `5xx` (reintentar una escritura sin idempotencia duplicaría issues).

use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;

/// URL base **fija** del API de GitHub. No es un ajuste: no hay instancia
/// self-hosted que configurar.
pub const API_BASE_URL: &str = "https://api.github.com";

/// Repositorio por defecto cuando el ajuste `github_default_repo` no tiene valor.
pub const DEFAULT_REPO: &str = "atareao/valet-ai";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
const REQUEST_TIMEOUT_SECS: u64 = 15;

/// Ruta del endpoint que identifica al usuario autenticado (sonda de conexión).
const USER_PATH: &str = "/user";

/// Errores del cliente de GitHub.
///
/// [`GithubError::NotConfigured`] (falta el token) es una variante **distinta** de
/// [`GithubError::Unauthorized`] (el token no vale). Ningún mensaje incluye el
/// token ni la cabecera `Authorization`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubError {
    /// Falta el token; la llamada no toca la red.
    NotConfigured,
    /// Un argumento (repositorio, título, labels) no es válido. Lleva el detalle.
    InvalidArguments(String),
    /// `401`/`403` — el token no es válido o no tiene permiso.
    Unauthorized,
    /// `404` — el repositorio no existe o el token no lo ve.
    NotFound,
    /// `422` — GitHub rechazó los datos (p. ej. un label inexistente).
    InvalidInput(String),
    /// `429` — se ha superado el límite de peticiones.
    RateLimited,
    /// `5xx` o fallo de red — GitHub no responde.
    Upstream(String),
}

impl std::fmt::Display for GithubError {
    /// Mensajes accionables en español. **Ninguno** incluye el token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GithubError::NotConfigured => write!(
                f,
                "GitHub no está configurado: pega un Personal Access Token en \
                 Ajustes → Integraciones → GitHub"
            ),
            GithubError::InvalidArguments(detail) => write!(f, "{detail}"),
            GithubError::Unauthorized => write!(
                f,
                "GitHub rechazó el token: revisa que el Personal Access Token sea \
                 válido y tenga permiso sobre el repositorio (Ajustes → \
                 Integraciones → GitHub)"
            ),
            GithubError::NotFound => write!(
                f,
                "el repositorio no existe o el token no lo ve: revisa el \
                 repositorio y los permisos del token"
            ),
            GithubError::InvalidInput(detail) => {
                write!(f, "GitHub rechazó los datos del issue: {detail}")
            }
            GithubError::RateLimited => write!(
                f,
                "GitHub ha limitado las peticiones: espera antes de volver a \
                 intentarlo, no reintentes en bucle"
            ),
            GithubError::Upstream(detail) => {
                write!(f, "GitHub no responde: {detail}")
            }
        }
    }
}

impl std::error::Error for GithubError {}

/// Configuración resuelta del cliente de GitHub.
#[derive(Clone, PartialEq, Eq)]
pub struct GithubConfig {
    /// Personal Access Token (ajuste `github_token`).
    pub token: String,
    /// Repositorio por defecto (ajuste `github_default_repo`, def. [`DEFAULT_REPO`]).
    pub default_repo: String,
    /// Labels por defecto (ajuste `github_default_labels`, lista con comas).
    pub default_labels: Vec<String>,
}

impl std::fmt::Debug for GithubConfig {
    /// Redacta el token: nunca debe aparecer en un `Debug`, pero el repositorio y
    /// los labels sí son útiles para diagnosticar.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubConfig")
            .field("token", &"***")
            .field("default_repo", &self.default_repo)
            .field("default_labels", &self.default_labels)
            .finish()
    }
}

/// El resultado de una creación correcta: el número y la URL del issue, si el
/// cuerpo era interpretable (un `201` con cuerpo ilegible los deja en `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedIssue {
    pub number: Option<i64>,
    pub url: Option<String>,
}

/// Cliente de GitHub atado a una URL base.
#[derive(Debug, Clone)]
pub struct Github {
    http: reqwest::Client,
    base_url: String,
    config: GithubConfig,
}

impl Github {
    /// Cliente contra la URL **fija** de GitHub con la configuración dada.
    pub fn new(config: GithubConfig) -> Self {
        Self::build(API_BASE_URL.to_string(), config)
    }

    /// Cliente con una URL base explícita. **No lo usa el camino de producción**
    /// (que siempre pasa por [`Self::new`] con [`API_BASE_URL`]): lo consumen la
    /// sonda inyectable del handler (`check_with_base`) y los tests que apuntan a
    /// un servidor de prueba.
    pub(crate) fn with_base_url(base_url: String, config: GithubConfig) -> Self {
        Self::build(base_url, config)
    }

    fn build(base_url: String, config: GithubConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            config,
        }
    }

    /// Resuelve la configuración de los ajustes en **cada llamada**: `github_token`,
    /// `github_default_repo` (con [`DEFAULT_REPO`] por defecto) y
    /// `github_default_labels` (lista separada por comas). **No** hay respaldo de
    /// variables de entorno. Sin token devuelve [`GithubError::NotConfigured`]
    /// **sin** salir a la red.
    pub async fn resolve_config(pool: &SqlitePool) -> Result<GithubConfig, GithubError> {
        let token = setting_value(pool, "github_token").await;
        let Some(token) = token else {
            return Err(GithubError::NotConfigured);
        };

        let default_repo = setting_value(pool, "github_default_repo")
            .await
            .unwrap_or_else(|| DEFAULT_REPO.to_string());
        let default_labels = setting_value(pool, "github_default_labels")
            .await
            .map(|value| parse_labels(&value))
            .unwrap_or_default();

        Ok(GithubConfig {
            token,
            default_repo,
            default_labels,
        })
    }

    /// Resuelve la configuración y construye el cliente.
    pub async fn from_settings(pool: &SqlitePool) -> Result<Self, GithubError> {
        let config = Self::resolve_config(pool).await?;
        Ok(Self::new(config))
    }

    /// Configuración resuelta (nunca expone el token en un `Debug`).
    pub fn config(&self) -> &GithubConfig {
        &self.config
    }

    /// Sonda la conexión real: `GET {base}/user` con `Authorization: Bearer`. Un
    /// `200` es éxito; un `401`/`403` es un token rechazado
    /// ([`GithubError::Unauthorized`], distinto de una caída) y cualquier otro
    /// desenlace se traduce a un [`GithubError`]. No se reintenta.
    pub async fn check_connection(&self) -> Result<(), GithubError> {
        let response = self
            .http
            .get(format!("{}{USER_PATH}", self.base_url))
            .bearer_auth(&self.config.token)
            .send()
            .await
            .map_err(|_| GithubError::Upstream("fallo de red".to_string()))?;

        let status = response.status().as_u16();
        if status == 200 {
            return Ok(());
        }

        let message = extract_message(response).await;
        Err(error_from_status(status, message))
    }

    /// `POST {base}/repos/{repo}/issues` con el cuerpo `{title, body?, labels?}`
    /// y `Authorization: Bearer <token>`. `body` y `labels` se envían **solo**
    /// cuando están presentes. Solo un `201` es éxito; cualquier otro desenlace
    /// se traduce a un [`GithubError`] y **no** se reintenta.
    pub async fn create_issue(
        &self,
        repo: &str,
        title: &str,
        body: Option<&str>,
        labels: &[String],
    ) -> Result<CreatedIssue, GithubError> {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "title".to_string(),
            serde_json::Value::String(title.to_string()),
        );
        if let Some(body) = body {
            payload.insert(
                "body".to_string(),
                serde_json::Value::String(body.to_string()),
            );
        }
        if !labels.is_empty() {
            payload.insert(
                "labels".to_string(),
                serde_json::Value::Array(
                    labels
                        .iter()
                        .map(|label| serde_json::Value::String(label.clone()))
                        .collect(),
                ),
            );
        }

        let response = self
            .http
            .post(format!("{}/repos/{repo}/issues", self.base_url))
            .bearer_auth(&self.config.token)
            .json(&serde_json::Value::Object(payload))
            .send()
            .await
            .map_err(|_| GithubError::Upstream("fallo de red".to_string()))?;

        let status = response.status().as_u16();
        if status == 201 {
            let value = response.json::<serde_json::Value>().await.ok();
            return Ok(CreatedIssue {
                number: value
                    .as_ref()
                    .and_then(|v| v.get("number"))
                    .and_then(|v| v.as_i64()),
                url: value
                    .as_ref()
                    .and_then(|v| v.get("html_url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }

        let message = extract_message(response).await;
        Err(error_from_status(status, message))
    }
}

/// Lee un ajuste de `settings` y lo `trim`ea; `None` si falta, está en blanco o
/// la lectura falla (un error de la base de datos se trata como ausencia, nunca
/// como pánico).
pub(crate) async fn setting_value(pool: &SqlitePool, key: &str) -> Option<String> {
    SettingsRepo::get(pool, key)
        .await
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Parsea una lista de labels separada por comas, recortando los espacios y
/// descartando las entradas vacías.
fn parse_labels(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// Lee el cuerpo de una respuesta de error y extrae su `message`, si lo hay.
async fn extract_message(response: reqwest::Response) -> Option<String> {
    let body = response.text().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    parsed
        .get("message")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// Traduce un estado HTTP de error de GitHub a su [`GithubError`].
///
/// Nunca incluye el token: solo el estado y, para el `422`, el `message` que
/// devuelve GitHub.
fn error_from_status(status: u16, message: Option<String>) -> GithubError {
    match status {
        401 | 403 => GithubError::Unauthorized,
        404 => GithubError::NotFound,
        422 => GithubError::InvalidInput(
            message.unwrap_or_else(|| "validación rechazada por GitHub".to_string()),
        ),
        429 => GithubError::RateLimited,
        500..=599 => GithubError::Upstream(format!("GitHub respondió {status}")),
        other => GithubError::Upstream(format!("GitHub respondió un estado inesperado {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = "ghp_SECRET";

    /// BD en memoria con todas las migraciones aplicadas (la tabla `settings`
    /// existe y ya trae los ajustes de GitHub sembrados).
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("pool en memoria");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migraciones");
        pool
    }

    async fn seed(pool: &SqlitePool, key: &str, value: &str) {
        SettingsRepo::set(pool, key, value).await.expect("sembrar");
    }

    fn config_for() -> GithubConfig {
        GithubConfig {
            token: TOKEN.to_string(),
            default_repo: "atareao/valet-ai".to_string(),
            default_labels: vec!["valet".to_string()],
        }
    }

    fn client_for(server: &MockServer) -> Github {
        Github::with_base_url(server.uri(), config_for())
    }

    // ─── 1.1.1 Debug redacta el token ────────────────────────────────────────

    #[test]
    fn debug_redacts_the_token() {
        let config = GithubConfig {
            token: TOKEN.to_string(),
            default_repo: "atareao/valet-ai".to_string(),
            default_labels: vec!["valet".to_string()],
        };

        let debug = format!("{config:?}");

        assert!(!debug.contains(TOKEN), "el Debug filtra el token: {debug}");
        assert!(debug.contains("***"), "el Debug debe redactar: {debug}");
    }

    // ─── 1.1.2 sin token → NotConfigured, sin red ────────────────────────────

    #[tokio::test]
    async fn without_token_returns_not_configured() {
        let pool = setup_pool().await;
        seed(&pool, "github_token", "").await;

        let err = Github::resolve_config(&pool)
            .await
            .expect_err("sin token debe fallar");

        assert_eq!(err, GithubError::NotConfigured);
    }

    // ─── 1.1.3 resolución de repo y labels ───────────────────────────────────

    #[tokio::test]
    async fn resolve_config_reads_repo_and_parses_labels() {
        let pool = setup_pool().await;
        seed(&pool, "github_token", TOKEN).await;
        seed(&pool, "github_default_repo", "otro/repo").await;
        seed(&pool, "github_default_labels", "feature, valet ,").await;

        let config = Github::resolve_config(&pool).await.expect("config");

        assert_eq!(config.token, TOKEN);
        assert_eq!(config.default_repo, "otro/repo");
        assert_eq!(
            config.default_labels,
            vec!["feature".to_string(), "valet".to_string()]
        );
    }

    // ─── 1.1.4 default_repo por defecto ──────────────────────────────────────

    #[tokio::test]
    async fn resolve_config_defaults_the_repo() {
        let pool = setup_pool().await;
        seed(&pool, "github_token", TOKEN).await;
        seed(&pool, "github_default_repo", "").await;

        let config = Github::resolve_config(&pool).await.expect("config");

        assert_eq!(config.default_repo, DEFAULT_REPO);
    }

    /// Scenario: La URL del API es fija.
    #[test]
    fn the_api_url_is_fixed_and_not_a_setting() {
        assert_eq!(API_BASE_URL, "https://api.github.com");
        let debug = format!(
            "{:?}",
            Github::new(GithubConfig {
                token: TOKEN.to_string(),
                default_repo: DEFAULT_REPO.to_string(),
                default_labels: Vec::new(),
            })
        );
        assert!(
            debug.contains("https://api.github.com"),
            "el cliente usa la URL fija: {debug}"
        );
    }

    // ─── 1.1.5 la petición lleva Bearer y el cuerpo ──────────────────────────

    #[tokio::test]
    async fn create_issue_sends_bearer_and_the_full_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .and(body_json(serde_json::json!({
                "title": "Título",
                "body": "Cuerpo",
                "labels": ["feature", "valet"],
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "number": 7,
                "html_url": "https://github.com/atareao/valet-ai/issues/7",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = client_for(&server);
        let created = client
            .create_issue(
                "atareao/valet-ai",
                "Título",
                Some("Cuerpo"),
                &["feature".to_string(), "valet".to_string()],
            )
            .await
            .expect("201 debe ser éxito");

        assert_eq!(created.number, Some(7));
        assert_eq!(
            created.url.as_deref(),
            Some("https://github.com/atareao/valet-ai/issues/7")
        );
    }

    /// Scenario: Los campos opcionales se omiten cuando faltan.
    #[tokio::test]
    async fn create_issue_omits_optional_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .and(body_json(serde_json::json!({ "title": "Solo título" })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "number": 1,
                "html_url": "https://github.com/atareao/valet-ai/issues/1",
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        client
            .create_issue("atareao/valet-ai", "Solo título", None, &[])
            .await
            .expect("201 debe ser éxito");
    }

    /// Scenario: Solo `201` es éxito; un `200` no lo es.
    #[tokio::test]
    async fn only_201_is_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "number": 1,
                "html_url": "https://github.com/atareao/valet-ai/issues/1",
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let err = client
            .create_issue("atareao/valet-ai", "Título", None, &[])
            .await
            .expect_err("un 200 no es éxito");

        assert!(
            matches!(err, GithubError::Upstream(_)),
            "un 200 no es 201: {err:?}"
        );
    }

    // ─── 1.1.7 traducción de estados ─────────────────────────────────────────

    #[tokio::test]
    async fn create_issue_translates_error_statuses() {
        for (status, expected) in [
            (401, GithubError::Unauthorized),
            (403, GithubError::Unauthorized),
            (404, GithubError::NotFound),
            (429, GithubError::RateLimited),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/repos/atareao/valet-ai/issues"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let client = client_for(&server);
            let err = client
                .create_issue("atareao/valet-ai", "Título", None, &[])
                .await
                .expect_err("estado de error");

            assert_eq!(err, expected, "status {status} debe traducirse");
        }
    }

    #[tokio::test]
    async fn create_issue_422_carries_the_server_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                "message": "Validation Failed: label `nope` does not exist",
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let err = client
            .create_issue("atareao/valet-ai", "Título", None, &[])
            .await
            .expect_err("422");

        assert_eq!(
            err,
            GithubError::InvalidInput("Validation Failed: label `nope` does not exist".to_string())
        );
    }

    #[tokio::test]
    async fn create_issue_5xx_is_upstream() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let err = client
            .create_issue("atareao/valet-ai", "Título", None, &[])
            .await
            .expect_err("5xx");

        assert!(matches!(err, GithubError::Upstream(_)), "{err:?}");
    }

    #[tokio::test]
    async fn create_issue_network_failure_is_upstream() {
        // Puerto cerrado: fallo de red inmediato.
        let client = Github::with_base_url(
            "http://127.0.0.1:1".to_string(),
            GithubConfig {
                token: TOKEN.to_string(),
                default_repo: DEFAULT_REPO.to_string(),
                default_labels: Vec::new(),
            },
        );

        let err = client
            .create_issue("atareao/valet-ai", "Título", None, &[])
            .await
            .expect_err("fallo de red");

        assert!(matches!(err, GithubError::Upstream(_)), "{err:?}");
    }

    // ─── 1.1.8 no hay reintentos ─────────────────────────────────────────────

    #[tokio::test]
    async fn create_issue_does_not_retry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let _ = client
            .create_issue("atareao/valet-ai", "Título", None, &[])
            .await;

        let requests = server.received_requests().await.expect("requests");
        assert_eq!(
            requests.len(),
            1,
            "una sola petición: no se reintenta la escritura"
        );
    }

    // ─── 1.1.9 ningún error filtra el token ──────────────────────────────────

    #[test]
    fn no_error_display_leaks_the_token() {
        let errors = [
            GithubError::NotConfigured,
            GithubError::InvalidArguments("repo mal".to_string()),
            GithubError::Unauthorized,
            GithubError::NotFound,
            GithubError::InvalidInput("detalle".to_string()),
            GithubError::RateLimited,
            GithubError::Upstream("fallo".to_string()),
        ];

        for error in errors {
            let text = error.to_string();
            assert!(
                !text.contains(TOKEN) && !text.contains("SECRET"),
                "un mensaje filtra el token: {text}"
            );
        }
    }

    // ─── sonda de conexión ───────────────────────────────────────────────────

    #[tokio::test]
    async fn check_connection_ok_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "atareao",
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        client
            .check_connection()
            .await
            .expect("200 debe ser conexión correcta");
    }

    #[tokio::test]
    async fn check_connection_401_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let err = client.check_connection().await.expect_err("401");

        assert_eq!(err, GithubError::Unauthorized);
    }
}
