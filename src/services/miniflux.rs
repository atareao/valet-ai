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

use crate::db::repos::settings::SettingsRepo;
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

impl std::fmt::Display for MinifluxError {
    /// Mensajes accionables en español. **Ninguno** incluye el token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MinifluxError::MissingCredentials => write!(
                f,
                "no hay token de Miniflux configurado: defínelo en Ajustes → API Keys o en MINIFLUX_API_KEY"
            ),
            MinifluxError::Unauthorized => {
                write!(f, "el token de Miniflux no es válido o ha caducado")
            }
            MinifluxError::NotFound => write!(f, "la entrada solicitada no existe en Miniflux"),
            MinifluxError::Unavailable => {
                write!(f, "Miniflux no está disponible (fallo de red o error del servidor)")
            }
            MinifluxError::Http(detail) => {
                write!(f, "Miniflux devolvió un error inesperado: {detail}")
            }
        }
    }
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
    pub async fn resolve_config(pool: &SqlitePool) -> Result<MinifluxConfig, MinifluxError> {
        let base_url = setting_or_env(pool, "miniflux_base_url", "MINIFLUX_BASE_URL")
            .await
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let api_key = setting_or_env(pool, "miniflux_api_key", "MINIFLUX_API_KEY").await;

        match api_key {
            Some(api_key) => Ok(MinifluxConfig { base_url, api_key }),
            None => Err(MinifluxError::MissingCredentials),
        }
    }

    /// `GET /v1/entries?status=unread&limit={limit}[&category_id={category}]`
    ///
    /// Devuelve el JSON `{total, entries:[…]}` sin el contenido de cada entrada.
    /// El token viaja en la cabecera `X-Auth-Token`. Sin token (vacío o en
    /// blanco) devuelve [`MinifluxError::MissingCredentials`] sin tocar la red.
    pub async fn list_unread(
        &self,
        api_key: &str,
        limit: u32,
        category: Option<i64>,
    ) -> Result<serde_json::Value, MinifluxError> {
        require_token(api_key)?;

        // El `Serializer` se libera dentro de este bloque: si sobreviviera al
        // `.await` de la petición, el futuro dejaría de ser `Send` (el trait
        // `Tool` exige `Send`).
        let url = {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            query.append_pair("status", "unread");
            query.append_pair("limit", &limit.to_string());
            if let Some(category_id) = category {
                query.append_pair("category_id", &category_id.to_string());
            }
            format!("{}/v1/entries?{}", self.base_url, query.finish())
        };

        let response = self
            .http
            .get(url)
            .header("X-Auth-Token", api_key)
            .send()
            .await
            .map_err(|_| MinifluxError::Unavailable)?;
        decode_json(response).await
    }

    /// `GET /v1/entries/{entry_id}` — una sola entrada, con su `content`. **No**
    /// cambia su estado. El token viaja en `X-Auth-Token`.
    pub async fn get_entry(
        &self,
        api_key: &str,
        entry_id: i64,
    ) -> Result<serde_json::Value, MinifluxError> {
        require_token(api_key)?;

        let response = self
            .http
            .get(format!("{}/v1/entries/{entry_id}", self.base_url))
            .header("X-Auth-Token", api_key)
            .send()
            .await
            .map_err(|_| MinifluxError::Unavailable)?;
        decode_json(response).await
    }

    /// `GET /v1/entries?search={query}&limit={limit}` — **sin** `status`, para
    /// encontrar también lo ya leído. El token viaja en `X-Auth-Token`.
    pub async fn search_entries(
        &self,
        api_key: &str,
        query: &str,
        limit: u32,
    ) -> Result<serde_json::Value, MinifluxError> {
        require_token(api_key)?;

        // Mismo cuidado que en `list_unread`: el `Serializer` se construye y se
        // consume dentro del bloque, de modo que ningún valor no-`Send` cruza el
        // `.await` y el futuro sigue siendo `Send`.
        let url = {
            let query_string = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("search", query)
                .append_pair("limit", &limit.to_string())
                .finish();
            format!("{}/v1/entries?{query_string}", self.base_url)
        };

        let response = self
            .http
            .get(url)
            .header("X-Auth-Token", api_key)
            .send()
            .await
            .map_err(|_| MinifluxError::Unavailable)?;
        decode_json(response).await
    }

    /// `PUT /v1/entries` con `{"entry_ids":[…],"status":"read"}` — marca las
    /// entradas como leídas en una sola petición (idempotente, `204`). El token
    /// viaja en `X-Auth-Token`.
    pub async fn mark_read(&self, api_key: &str, ids: &[i64]) -> Result<(), MinifluxError> {
        require_token(api_key)?;

        let body = serde_json::json!({ "entry_ids": ids, "status": "read" });
        let response = self
            .http
            .put(format!("{}/v1/entries", self.base_url))
            .header("X-Auth-Token", api_key)
            .json(&body)
            .send()
            .await
            .map_err(|_| MinifluxError::Unavailable)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(error_from_status(status.as_u16()))
        }
    }

    /// `GET /v1/categories` — devuelve el array de categorías (`[{id, title,
    /// …}]`) tal cual, para resolver el **nombre** de una categoría a su `id`.
    /// El token viaja en la cabecera `X-Auth-Token`. Sin token (vacío o en
    /// blanco) devuelve [`MinifluxError::MissingCredentials`] sin tocar la red;
    /// un `401` se traduce a [`MinifluxError::Unauthorized`], un `5xx` o un
    /// fallo de transporte a [`MinifluxError::Unavailable`] y un cuerpo que no
    /// sea JSON interpretable a [`MinifluxError::Http`].
    pub async fn categories(&self, api_key: &str) -> Result<serde_json::Value, MinifluxError> {
        require_token(api_key)?;

        let response = self
            .http
            .get(format!("{}/v1/categories", self.base_url))
            .header("X-Auth-Token", api_key)
            .send()
            .await
            .map_err(|_| MinifluxError::Unavailable)?;
        decode_json(response).await
    }
}

/// Rechaza un token vacío o en blanco **antes** de tocar la red.
fn require_token(api_key: &str) -> Result<(), MinifluxError> {
    if api_key.trim().is_empty() {
        Err(MinifluxError::MissingCredentials)
    } else {
        Ok(())
    }
}

/// Lee una clave de `settings`; si está vacía o ilegible, cae a la variable de
/// entorno. Un error de la base de datos se trata como ausencia y se registra
/// con `tracing::warn!`, nunca como fallo.
async fn setting_or_env(pool: &SqlitePool, key: &str, env: &str) -> Option<String> {
    match SettingsRepo::get(pool, key).await {
        Ok(Some(value)) => {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                setting = key,
                "no se pudo leer el ajuste de miniflux: {error}"
            );
        }
    }

    std::env::var(env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Parsea el JSON de una respuesta correcta o traduce el error de Miniflux.
///
/// Una respuesta correcta que no sea JSON interpretable se traduce a
/// [`MinifluxError::Http`]; el texto del error de `reqwest` no incluye el token
/// (viaja en una cabecera, no en la URL).
async fn decode_json(response: reqwest::Response) -> Result<serde_json::Value, MinifluxError> {
    let status = response.status();
    if status.is_success() {
        return response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| MinifluxError::Http(error.to_string()));
    }

    Err(error_from_status(status.as_u16()))
}

/// Traduce un estado HTTP de error de Miniflux a su [`MinifluxError`].
fn error_from_status(status: u16) -> MinifluxError {
    match status {
        401 => MinifluxError::Unauthorized,
        404 => MinifluxError::NotFound,
        500..=599 => MinifluxError::Unavailable,
        _ => MinifluxError::Http(format!("miniflux respondió {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        SettingsRepo::set(&pool, "miniflux_base_url", "")
            .await
            .unwrap();
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

        assert!(
            matches!(err, MinifluxError::MissingCredentials),
            "err: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Sin token no se sale a la red
    // -----------------------------------------------------------------------

    /// Scenario: Sin token no se envía ninguna petición. Se levanta un
    /// `MockServer` para vigilar que no llega nada y se llama con un token vacío:
    /// `MissingCredentials` y cero peticiones. Se cubren **las cuatro**
    /// operaciones, porque la comprobación del token está repetida en cada una.
    #[tokio::test]
    async fn missing_token_sends_no_request_list_unread() {
        let server = MockServer::start().await;
        let client = Miniflux::new(server.uri());

        let err = client.list_unread("", 20, None).await.unwrap_err();

        assert!(
            matches!(err, MinifluxError::MissingCredentials),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "list_unread sin token no debe enviar ninguna petición"
        );
    }

    #[tokio::test]
    async fn missing_token_sends_no_request_get_entry() {
        let server = MockServer::start().await;
        let client = Miniflux::new(server.uri());

        let err = client.get_entry("", 1).await.unwrap_err();

        assert!(
            matches!(err, MinifluxError::MissingCredentials),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "get_entry sin token no debe enviar ninguna petición"
        );
    }

    #[tokio::test]
    async fn missing_token_sends_no_request_search_entries() {
        let server = MockServer::start().await;
        let client = Miniflux::new(server.uri());

        let err = client.search_entries("", "rust", 20).await.unwrap_err();

        assert!(
            matches!(err, MinifluxError::MissingCredentials),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "search_entries sin token no debe enviar ninguna petición"
        );
    }

    #[tokio::test]
    async fn missing_token_sends_no_request_mark_read() {
        let server = MockServer::start().await;
        let client = Miniflux::new(server.uri());

        let err = client.mark_read("", &[1, 2]).await.unwrap_err();

        assert!(
            matches!(err, MinifluxError::MissingCredentials),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "mark_read sin token no debe enviar ninguna petición"
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
    // GET /v1/categories — cabecera X-Auth-Token y cuerpo
    // -----------------------------------------------------------------------

    /// Scenario: `categories` pide `GET /v1/categories`, envía el token en
    /// `X-Auth-Token` y devuelve el array de categorías tal cual.
    #[tokio::test]
    async fn categories_sends_token_and_returns_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/categories"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": 1, "title": "Tech" },
                { "id": 2, "title": "News" }
            ])))
            .mount(&server)
            .await;

        let client = Miniflux::new(server.uri());
        let value = client.categories("test-key").await.unwrap();

        assert_eq!(
            value,
            serde_json::json!([
                { "id": 1, "title": "Tech" },
                { "id": 2, "title": "News" }
            ])
        );

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

    /// Scenario: Un `401` se traduce a `Unauthorized` y el token no aparece ni
    /// en el `Debug` ni en el `Display` del error.
    ///
    /// El mock responde con un cuerpo que **sí contiene** el token, de modo que
    /// la aserción fallaría si el cliente reenviara el cuerpo del error: así el
    /// test prueba que el token no se filtra por ese camino.
    #[tokio::test]
    async fn error_401_is_unauthorized_and_hides_token() {
        const TOKEN: &str = "super-secret-token";
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(format!("unauthorized: {TOKEN}")),
            )
            .mount(&server)
            .await;

        let client = Miniflux::new(server.uri());
        let err = client.list_unread(TOKEN, 20, None).await.unwrap_err();

        assert!(matches!(err, MinifluxError::Unauthorized), "err: {err:?}");
        let debug = format!("{err:?}");
        assert!(!debug.contains(TOKEN), "el Debug filtra el token: {debug}");
        let display = err.to_string();
        assert!(
            !display.contains(TOKEN),
            "el Display filtra el token: {display}"
        );
    }

    /// Scenario: Un `5xx` (aquí `503`) se traduce a `Unavailable`.
    #[tokio::test]
    async fn error_503_is_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(ResponseTemplate::new(503).set_body_string("miniflux down"))
            .mount(&server)
            .await;

        let client = Miniflux::new(server.uri());
        let err = client.list_unread("test-key", 20, None).await.unwrap_err();

        assert!(matches!(err, MinifluxError::Unavailable), "err: {err:?}");
    }

    /// Scenario: Un error de lectura de `settings` se trata como **ausencia** y
    /// cae al entorno, en vez de reventar. Se rompe la tabla `settings` para
    /// forzar el error de la consulta.
    #[tokio::test]
    #[serial]
    async fn resolve_config_treats_settings_error_as_absent() {
        let pool = setup_pool().await;
        sqlx::query("DROP TABLE settings")
            .execute(&pool)
            .await
            .expect("drop settings para forzar el error de lectura");

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

    // -----------------------------------------------------------------------
    // Los futuros públicos deben ser `Send` (el trait `Tool` lo exige)
    // -----------------------------------------------------------------------

    /// Aserción de compilación: los cinco futuros públicos tienen que ser
    /// `Send`, porque el trait `Tool` los ejecuta en un runtime multihilo. Un
    /// `url::form_urlencoded::Serializer` vivo a través de un `.await` rompe
    /// esta propiedad sin que un `#[tokio::test]` de un solo hilo lo note.
    #[test]
    fn public_futures_are_send() {
        fn assert_send<T: Send>(_: T) {}
        let client = Miniflux::new("http://localhost:1".to_string());
        assert_send(client.list_unread("k", 1, None));
        assert_send(client.search_entries("k", "q", 1));
        assert_send(client.get_entry("k", 1));
        assert_send(client.mark_read("k", &[1]));
        assert_send(client.categories("k"));
    }
}
