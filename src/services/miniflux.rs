//! Cliente de la API HTTP v1 de `Miniflux`.
//!
//! Encapsula la resolución de la configuración (ajustes → entorno → valor por
//! defecto solo para la URL) y las cuatro operaciones que consumirán las
//! herramientas del feed (`rss_list_unread`, `rss_get_entry`,
//! `rss_search_entries` y `rss_mark_read`). El token viaja en la cabecera
//! `X-Auth-Token: <token>` (no `Bearer`) y **nunca** debe aparecer ni en un
//! `Debug`, ni en un log, ni en el mensaje de un [`MinifluxError`].
//!
//! Es el molde de [`crate::services::apimail`]: misma resolución de credenciales
//! en cada llamada y misma traducción de errores.
//!
//! # Estado (fase RED — tarea 2.1)
//!
//! Este módulo fija la **superficie pública** y los tests que describen el
//! contrato. Los cuerpos de [`Miniflux::resolve_config`] y de los cuatro métodos
//! HTTP son todavía `unimplemented!()`: se implementan en la fase GREEN (tarea
//! 2.2). Por eso `cargo test miniflux` está en rojo a propósito.

use sqlx::SqlitePool;

/// URL base por defecto de Miniflux cuando ni el ajuste ni la variable de
/// entorno la definen.
pub const DEFAULT_BASE_URL: &str = "https://miniflux.territoriolinux.es";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Errores del cliente de Miniflux.
///
/// [`MinifluxError::MissingCredentials`] (no hay token configurado) es una
/// variante **distinta** de [`MinifluxError::Unauthorized`] (el token no vale):
/// la primera indica dónde configurarlo y la segunda que ha caducado o es
/// incorrecto.
#[derive(Debug)]
pub enum MinifluxError {
    /// No hay token ni en `settings` ni en el entorno.
    MissingCredentials,
    /// `401` — el token no es válido o ha caducado.
    Unauthorized,
    /// `404` — la entrada (o el recurso) no existe.
    NotFound,
    /// Fallo de red o `5xx` — Miniflux no está disponible.
    Unavailable,
    /// Cualquier otro estado o una respuesta que no sea JSON interpretable.
    /// El mensaje **nunca** contiene el token.
    Http(String),
}

/// Configuración resuelta de Miniflux.
#[derive(Clone, PartialEq)]
pub struct MinifluxConfig {
    pub base_url: String,
    pub api_key: String,
}

impl std::fmt::Debug for MinifluxConfig {
    /// Redacta la API key: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinifluxConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"***")
            .finish()
    }
}

/// Cliente de Miniflux atado a una URL base.
///
/// En la fase RED los campos aún no se leen (los métodos son stubs); se silencia
/// `dead_code` hasta que la fase GREEN los use.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Miniflux {
    http: reqwest::Client,
    base_url: String,
}

impl Miniflux {
    /// Cliente con la base dada, `user_agent` propio y timeout de 30 s.
    ///
    /// La base es **inyectable** para poder apuntar los tests a un servidor de
    /// prueba con `wiremock`.
    pub fn new(base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build reqwest Client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Resuelve la configuración: primero `settings` (`miniflux_base_url` /
    /// `miniflux_api_key`) si el valor no está vacío tras `trim`, después las
    /// variables de entorno `MINIFLUX_BASE_URL` / `MINIFLUX_API_KEY` y, solo
    /// para la URL, el valor por defecto [`DEFAULT_BASE_URL`]. Sin token
    /// devuelve [`MinifluxError::MissingCredentials`]. Las claves se leen en
    /// **cada llamada**, de modo que editarlas surte efecto sin reiniciar. Un
    /// error de lectura de settings se trata como ausencia (nunca revienta).
    ///
    /// RED stub: se implementa en la tarea 2.2.
    pub async fn resolve_config(pool: &SqlitePool) -> Result<MinifluxConfig, MinifluxError> {
        let _ = pool;
        unimplemented!("resolve_config: fase RED — se implementa en la tarea 2.2")
    }

    /// `GET /v1/entries?status=unread&limit={limit}[&category_id={category}]`
    ///
    /// Devuelve el JSON `{total, entries:[…]}` sin el contenido de cada entrada.
    /// El token viaja en la cabecera `X-Auth-Token`. Sin token (vacío o en
    /// blanco) devuelve [`MinifluxError::MissingCredentials`] sin tocar la red.
    ///
    /// RED stub: se implementa en la tarea 2.2.
    pub async fn list_unread(
        &self,
        api_key: &str,
        limit: u32,
        category: Option<i64>,
    ) -> Result<serde_json::Value, MinifluxError> {
        let _ = (api_key, limit, category);
        unimplemented!("list_unread: fase RED — se implementa en la tarea 2.2")
    }

    /// `GET /v1/entries/{entry_id}` — una sola entrada, con su `content`. **No**
    /// cambia su estado. El token viaja en `X-Auth-Token`.
    ///
    /// RED stub: se implementa en la tarea 2.2.
    pub async fn get_entry(
        &self,
        api_key: &str,
        entry_id: i64,
    ) -> Result<serde_json::Value, MinifluxError> {
        let _ = (api_key, entry_id);
        unimplemented!("get_entry: fase RED — se implementa en la tarea 2.2")
    }

    /// `GET /v1/entries?search={query}&limit={limit}` — **sin** `status`, para
    /// encontrar también lo ya leído. El token viaja en `X-Auth-Token`.
    ///
    /// RED stub: se implementa en la tarea 2.2.
    pub async fn search_entries(
        &self,
        api_key: &str,
        query: &str,
        limit: u32,
    ) -> Result<serde_json::Value, MinifluxError> {
        let _ = (api_key, query, limit);
        unimplemented!("search_entries: fase RED — se implementa en la tarea 2.2")
    }

    /// `PUT /v1/entries` con `{"entry_ids":[…],"status":"read"}` — marca las
    /// entradas como leídas en una sola petición (idempotente, `204`). El token
    /// viaja en `X-Auth-Token`.
    ///
    /// RED stub: se implementa en la tarea 2.2.
    pub async fn mark_read(&self, api_key: &str, ids: &[i64]) -> Result<(), MinifluxError> {
        let _ = (api_key, ids);
        unimplemented!("mark_read: fase RED — se implementa en la tarea 2.2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::SettingsRepo;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory sqlite");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        pool
    }

    fn set_env(key: &str, value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    // -----------------------------------------------------------------------
    // resolve_config — precedencia settings > entorno > valor por defecto
    // -----------------------------------------------------------------------

    /// Scenario: El ajuste manda sobre el entorno (URL y token).
    #[tokio::test]
    #[serial]
    async fn resolve_config_setting_wins_over_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "miniflux_base_url", "https://from-settings.test")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "miniflux_api_key", "setting-key")
            .await
            .unwrap();
        set_env("MINIFLUX_BASE_URL", Some("https://from-env.test"));
        set_env("MINIFLUX_API_KEY", Some("env-key"));

        let config = Miniflux::resolve_config(&pool).await.unwrap();

        set_env("MINIFLUX_BASE_URL", None);
        set_env("MINIFLUX_API_KEY", None);

        assert_eq!(
            config,
            MinifluxConfig {
                base_url: "https://from-settings.test".to_string(),
                api_key: "setting-key".to_string(),
            }
        );
    }

    /// Scenario: Sin ajuste (vacío o en blanco) se usa el entorno.
    #[tokio::test]
    #[serial]
    async fn resolve_config_falls_back_to_env_when_setting_empty() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "miniflux_base_url", "   ")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "miniflux_api_key", "")
            .await
            .unwrap();
        set_env("MINIFLUX_BASE_URL", Some("https://from-env.test"));
        set_env("MINIFLUX_API_KEY", Some("env-key"));

        let config = Miniflux::resolve_config(&pool).await.unwrap();

        set_env("MINIFLUX_BASE_URL", None);
        set_env("MINIFLUX_API_KEY", None);

        assert_eq!(
            config,
            MinifluxConfig {
                base_url: "https://from-env.test".to_string(),
                api_key: "env-key".to_string(),
            }
        );
    }

    /// Scenario: Sin nada configurado la URL cae al valor por defecto.
    #[tokio::test]
    #[serial]
    async fn resolve_config_defaults_base_url_when_nothing_set() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "miniflux_base_url", "").await.unwrap();
        SettingsRepo::set(&pool, "miniflux_api_key", "only-key")
            .await
            .unwrap();
        set_env("MINIFLUX_BASE_URL", None);
        set_env("MINIFLUX_API_KEY", None);

        let config = Miniflux::resolve_config(&pool).await.unwrap();

        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.api_key, "only-key");
    }

    /// Sin token ni en settings ni en el entorno → `MissingCredentials`.
    #[tokio::test]
    #[serial]
    async fn resolve_config_missing_api_key() {
        let pool = setup_pool().await;
        set_env("MINIFLUX_BASE_URL", None);
        set_env("MINIFLUX_API_KEY", None);

        let err = Miniflux::resolve_config(&pool).await.unwrap_err();

        assert!(matches!(err, MinifluxError::MissingCredentials), "err: {err:?}");
    }

    // -----------------------------------------------------------------------
    // Sin token no se sale a la red
    // -----------------------------------------------------------------------

    /// Scenario: Sin token no se envía ninguna petición. Se levanta un
    /// `MockServer` para vigilar que no llega nada y se llama al cliente con un
    /// token vacío: debe devolver `MissingCredentials` sin tocar la red.
    #[tokio::test]
    async fn missing_token_sends_no_request() {
        let server = MockServer::start().await;
        let client = Miniflux::new(server.uri());

        let err = client.list_unread("", 20, None).await.unwrap_err();

        assert!(matches!(err, MinifluxError::MissingCredentials), "err: {err:?}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin token no debe enviarse ninguna petición"
        );
    }

    // -----------------------------------------------------------------------
    // El token viaja en la cabecera X-Auth-Token
    // -----------------------------------------------------------------------

    /// Scenario: El token se envía como `X-Auth-Token` (no `Bearer`).
    #[tokio::test]
    async fn token_travels_in_x_auth_token_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .and(query_param("status", "unread"))
            .and(query_param("limit", "20"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "total": 0, "entries": [] })),
            )
            .mount(&server)
            .await;

        let client = Miniflux::new(server.uri());
        let value = client.list_unread("test-key", 20, None).await.unwrap();

        assert_eq!(value["total"], 0);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]
                .headers
                .get("x-auth-token")
                .expect("la petición debe llevar la cabecera X-Auth-Token")
                .to_str()
                .unwrap(),
            "test-key"
        );
    }

    // -----------------------------------------------------------------------
    // Traducción de errores — el token nunca aparece
    // -----------------------------------------------------------------------

    /// Scenario: Un `401` se traduce a `Unauthorized` y el token no aparece en
    /// el mensaje de error.
    #[tokio::test]
    async fn error_401_is_unauthorized_and_hides_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let client = Miniflux::new(server.uri());
        let err = client
            .list_unread("super-secret-token", 20, None)
            .await
            .unwrap_err();

        assert!(matches!(err, MinifluxError::Unauthorized), "err: {err:?}");
        let debug = format!("{err:?}");
        assert!(
            !debug.contains("super-secret-token"),
            "el token no debe aparecer en el error: {debug}"
        );
    }
}
