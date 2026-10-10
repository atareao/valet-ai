//! Cliente de la API HTTP de `apimail`.
//!
//! Encapsula la resolución de credenciales (ajustes → entorno → valor por
//! defecto) y las cuatro operaciones que consumen las herramientas del correo.
//! La API key viaja como `Authorization: Bearer <clave>` y nunca se guarda en
//! el cliente ni aparece en un `Debug`, un log o un [`ApimailError`].

use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;

/// URL base por defecto de apimail.
pub const DEFAULT_BASE_URL: &str = "https://apimail.territoriolinux.es";

/// El buzón sobre el que operan todas las herramientas.
pub const MAILBOX: &str = "INBOX";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Errores del cliente de apimail.
#[derive(Debug)]
pub enum ApimailError {
    /// No hay API key ni en settings ni en el entorno.
    MissingCredentials,
    /// 401 unauthorized
    Unauthorized,
    /// 404 message_not_found
    MessageNotFound,
    /// 404 mailbox_not_found
    MailboxNotFound,
    /// 413 message_too_large / payload_too_large
    TooLarge,
    /// 422 message_not_parsable
    NotParsable,
    /// 502 smtp_error
    SmtpError,
    /// 503 imap_unavailable
    Unavailable,
    /// Fallo de red, JSON ilegible o cualquier otro estado.
    Http(String),
}

/// Configuración resuelta de apimail.
#[derive(Clone, PartialEq)]
pub struct ApimailConfig {
    pub base_url: String,
    pub api_key: String,
}

impl std::fmt::Debug for ApimailConfig {
    /// Redacta la API key: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApimailConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"***")
            .finish()
    }
}

/// Cliente de apimail atado a una URL base.
#[derive(Debug, Clone)]
pub struct Apimail {
    http: reqwest::Client,
    base_url: String,
}

impl Apimail {
    /// Cliente con la base dada, `user_agent` propio y timeout de 30 s.
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

    /// Resuelve la configuración: primero `settings` (`apimail_base_url` /
    /// `apimail_api_key`) si el valor no está vacío tras `trim`, después las
    /// variables de entorno `APIMAIL_BASE_URL` / `APIMAIL_API_KEY`, y para la
    /// URL el valor por defecto [`DEFAULT_BASE_URL`]. Sin API key devuelve
    /// [`ApimailError::MissingCredentials`]. Nunca falla por la base de datos:
    /// un error de lectura de settings se trata como ausencia (y se registra con
    /// `tracing::warn!`).
    pub async fn resolve_config(pool: &SqlitePool) -> Result<ApimailConfig, ApimailError> {
        let base_url = setting_or_env(pool, "apimail_base_url", "APIMAIL_BASE_URL")
            .await
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let api_key = setting_or_env(pool, "apimail_api_key", "APIMAIL_API_KEY").await;

        match api_key {
            Some(api_key) => Ok(ApimailConfig { base_url, api_key }),
            None => Err(ApimailError::MissingCredentials),
        }
    }

    /// `GET /api/messages?mailbox=INBOX&unseen=true&limit={limit}`
    pub async fn list_unread(
        &self,
        api_key: &str,
        limit: u32,
    ) -> Result<serde_json::Value, ApimailError> {
        let url = format!(
            "{}/api/messages?mailbox={MAILBOX}&unseen=true&limit={limit}",
            self.base_url
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(network)?;
        decode_json(response).await
    }

    /// `GET /api/messages/{uid}/body?mailbox=INBOX`
    pub async fn get_body(
        &self,
        api_key: &str,
        uid: u32,
    ) -> Result<serde_json::Value, ApimailError> {
        let url = format!(
            "{}/api/messages/{uid}/body?mailbox={MAILBOX}",
            self.base_url
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(network)?;
        decode_json(response).await
    }

    /// `GET /api/messages/{uid}?mailbox=INBOX&format=summary` (para responder)
    pub async fn get_summary(
        &self,
        api_key: &str,
        uid: u32,
    ) -> Result<serde_json::Value, ApimailError> {
        let url = format!(
            "{}/api/messages/{uid}?mailbox={MAILBOX}&format=summary",
            self.base_url
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(network)?;
        decode_json(response).await
    }

    /// `PATCH /api/messages/{uid}/flags?mailbox=INBOX` con `{"add":["\\Seen"]}`
    pub async fn mark_read(
        &self,
        api_key: &str,
        uid: u32,
    ) -> Result<serde_json::Value, ApimailError> {
        let url = format!(
            "{}/api/messages/{uid}/flags?mailbox={MAILBOX}",
            self.base_url
        );
        let response = self
            .http
            .patch(url)
            .bearer_auth(api_key)
            .json(&serde_json::json!({ "add": ["\\Seen"] }))
            .send()
            .await
            .map_err(network)?;
        decode_json(response).await
    }

    /// `POST /api/messages` con `{"to":[...],"subject":...,"text":...}`
    pub async fn send(
        &self,
        api_key: &str,
        to: &[String],
        subject: &str,
        text: &str,
    ) -> Result<(), ApimailError> {
        let url = format!("{}/api/messages", self.base_url);
        let body = serde_json::json!({ "to": to, "subject": subject, "text": text });
        let response = self
            .http
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(network)?;
        decode_json(response).await.map(|_| ())
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
                "no se pudo leer el ajuste de apimail: {error}"
            );
        }
    }

    std::env::var(env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Convierte un error de `reqwest` en [`ApimailError::Http`] (sin credenciales).
fn network(error: reqwest::Error) -> ApimailError {
    ApimailError::Http(error.to_string())
}

/// Parsea el JSON de una respuesta correcta o traduce el error de apimail.
async fn decode_json(response: reqwest::Response) -> Result<serde_json::Value, ApimailError> {
    let status = response.status();
    if status.is_success() {
        return response.json::<Value>().await.map_err(network);
    }

    let body = response.text().await.unwrap_or_default();
    Err(error_from_body(status.as_u16(), &body))
}

/// Traduce un estado y cuerpo de error de apimail a su [`ApimailError`].
///
/// El campo `error` del cuerpo distingue `message_not_found` de
/// `mailbox_not_found` y el `smtp_error` del 502. Los cuerpos que no son JSON,
/// o con un código desconocido, caen al error genérico `Http`.
fn error_from_body(status: u16, body: &str) -> ApimailError {
    let code = error_code(body);
    match status {
        401 => ApimailError::Unauthorized,
        404 => match code.as_deref() {
            Some("message_not_found") => ApimailError::MessageNotFound,
            Some("mailbox_not_found") => ApimailError::MailboxNotFound,
            _ => ApimailError::Http(format!("apimail respondió {status}")),
        },
        413 => ApimailError::TooLarge,
        422 => ApimailError::NotParsable,
        502 => match code.as_deref() {
            Some("smtp_error") => ApimailError::SmtpError,
            _ => ApimailError::Http(format!("apimail respondió {status}")),
        },
        503 => ApimailError::Unavailable,
        _ => ApimailError::Http(format!("apimail respondió {status}")),
    }
}

/// Extrae el campo `error` de un cuerpo `{"error":"<código>","message":"..."}`.
fn error_code(body: &str) -> Option<String> {
    let parsed: ApiErrorBody = serde_json::from_str(body).ok()?;
    parsed.error
}

/// Cuerpo de error de apimail: `{"error":"<código>","message":"..."}`.
#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{body_json, header, method, path, query_param};
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

    /// La API key nunca debe aparecer en un `Debug` de la configuración.
    #[test]
    fn debug_does_not_leak_api_key() {
        let config = ApimailConfig {
            base_url: "https://apimail.test".to_string(),
            api_key: "super-secret-key".to_string(),
        };

        let debug = format!("{config:?}");

        assert!(!debug.contains("super-secret-key"), "{debug}");
        assert!(debug.contains("***"), "{debug}");
        assert!(debug.contains("https://apimail.test"), "{debug}");
    }

    // -----------------------------------------------------------------------
    // resolve_config
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[serial]
    async fn resolve_config_setting_wins_over_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "apimail_base_url", "https://from-settings.test")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "apimail_api_key", "setting-key")
            .await
            .unwrap();
        set_env("APIMAIL_BASE_URL", Some("https://from-env.test"));
        set_env("APIMAIL_API_KEY", Some("env-key"));

        let config = Apimail::resolve_config(&pool).await.unwrap();

        set_env("APIMAIL_BASE_URL", None);
        set_env("APIMAIL_API_KEY", None);

        assert_eq!(
            config,
            ApimailConfig {
                base_url: "https://from-settings.test".to_string(),
                api_key: "setting-key".to_string(),
            }
        );
    }

    #[tokio::test]
    #[serial]
    async fn resolve_config_falls_back_to_env_when_setting_empty() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "apimail_base_url", "   ")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "apimail_api_key", "")
            .await
            .unwrap();
        set_env("APIMAIL_BASE_URL", Some("https://from-env.test"));
        set_env("APIMAIL_API_KEY", Some("env-key"));

        let config = Apimail::resolve_config(&pool).await.unwrap();

        set_env("APIMAIL_BASE_URL", None);
        set_env("APIMAIL_API_KEY", None);

        assert_eq!(
            config,
            ApimailConfig {
                base_url: "https://from-env.test".to_string(),
                api_key: "env-key".to_string(),
            }
        );
    }

    #[tokio::test]
    #[serial]
    async fn resolve_config_defaults_base_url_when_nothing_set() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "apimail_api_key", "only-key")
            .await
            .unwrap();
        set_env("APIMAIL_BASE_URL", None);
        set_env("APIMAIL_API_KEY", None);

        let config = Apimail::resolve_config(&pool).await.unwrap();

        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.api_key, "only-key");
    }

    #[tokio::test]
    #[serial]
    async fn resolve_config_missing_api_key() {
        let pool = setup_pool().await;
        set_env("APIMAIL_BASE_URL", None);
        set_env("APIMAIL_API_KEY", None);

        let err = Apimail::resolve_config(&pool).await.unwrap_err();

        assert!(matches!(err, ApimailError::MissingCredentials));
    }

    // -----------------------------------------------------------------------
    // list_unread
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn list_unread_hits_messages_and_returns_value() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages"))
            .and(query_param("mailbox", "INBOX"))
            .and(query_param("unseen", "true"))
            .and(query_param("limit", "20"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mailbox": "INBOX",
                "total": 2,
                "limit": 20,
                "offset": 0,
                "messages": [
                    { "uid": 1, "seq": 1, "flags": [], "size": 10, "internal_date": "x",
                      "envelope": { "from": [], "to": [], "cc": [],
                                    "subject": "First", "date": "x", "message_id": "a" } },
                    { "uid": 2, "seq": 2, "flags": [], "size": 11, "internal_date": "y",
                      "envelope": { "from": [], "to": [], "cc": [],
                                    "subject": "Second", "date": "y", "message_id": "b" } }
                ]
            })))
            .mount(&server)
            .await;

        let client = Apimail::new(server.uri());
        let value = client.list_unread("test-key", 20).await.unwrap();

        assert_eq!(value["total"], 2);
        assert_eq!(value["messages"].as_array().unwrap().len(), 2);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let query = requests[0].url.query().unwrap();
        assert!(query.contains("mailbox=INBOX"), "query: {query}");
        assert!(query.contains("unseen=true"), "query: {query}");
        assert!(query.contains("limit=20"), "query: {query}");
    }

    // -----------------------------------------------------------------------
    // get_body
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_body_hits_uid_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/42/body"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mailbox": "INBOX",
                "uid": 42,
                "text": "plain body",
                "html": "<p>plain body</p>"
            })))
            .mount(&server)
            .await;

        let client = Apimail::new(server.uri());
        let value = client.get_body("test-key", 42).await.unwrap();

        assert_eq!(value["uid"], 42);
        assert_eq!(value["text"], "plain body");
        assert_eq!(value["html"], "<p>plain body</p>");
    }

    // -----------------------------------------------------------------------
    // get_summary
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_summary_hits_uid_with_format_summary() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/7"))
            .and(query_param("mailbox", "INBOX"))
            .and(query_param("format", "summary"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mailbox": "INBOX",
                "uid": 7,
                "seq": 7,
                "flags": [],
                "size": 1,
                "internal_date": "x",
                "envelope": { "from": [], "to": [], "cc": [],
                              "subject": "Hi", "date": "x", "message_id": "m" },
                "format": "summary"
            })))
            .mount(&server)
            .await;

        let client = Apimail::new(server.uri());
        let value = client.get_summary("test-key", 7).await.unwrap();

        assert_eq!(value["format"], "summary");

        let requests = server.received_requests().await.unwrap();
        let query = requests[0].url.query().unwrap();
        assert!(query.contains("format=summary"), "query: {query}");
    }

    // -----------------------------------------------------------------------
    // mark_read
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn mark_read_patches_flags_with_seen() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/messages/9/flags"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(serde_json::json!({ "add": ["\\Seen"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mailbox": "INBOX",
                "uid": 9,
                "flags": ["\\Seen"]
            })))
            .mount(&server)
            .await;

        let client = Apimail::new(server.uri());
        let value = client.mark_read("test-key", 9).await.unwrap();

        assert_eq!(value["flags"][0], "\\Seen");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].method.as_str(), "PATCH");
    }

    // -----------------------------------------------------------------------
    // send
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn send_posts_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/messages"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(serde_json::json!({
                "to": ["a@b.c"],
                "subject": "Hi",
                "text": "Body"
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "status": "sent" })),
            )
            .mount(&server)
            .await;

        let client = Apimail::new(server.uri());
        client
            .send("test-key", &["a@b.c".to_string()], "Hi", "Body")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].method.as_str(), "POST");
        let content_type = requests[0]
            .headers
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("application/json"),
            "content-type: {content_type}"
        );
    }

    // -----------------------------------------------------------------------
    // error translation
    // -----------------------------------------------------------------------

    async fn error_for(status: u16, code: &str, message: &str) -> ApimailError {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages"))
            .respond_with(
                ResponseTemplate::new(status).set_body_json(serde_json::json!({
                    "error": code,
                    "message": message
                })),
            )
            .mount(&server)
            .await;
        let client = Apimail::new(server.uri());
        client.list_unread("test-key", 20).await.unwrap_err()
    }

    #[tokio::test]
    async fn error_401_is_unauthorized() {
        let err = error_for(401, "unauthorized", "nope").await;
        assert!(matches!(err, ApimailError::Unauthorized), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_404_message_not_found() {
        let err = error_for(404, "message_not_found", "gone").await;
        assert!(matches!(err, ApimailError::MessageNotFound), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_404_mailbox_not_found() {
        let err = error_for(404, "mailbox_not_found", "gone").await;
        assert!(matches!(err, ApimailError::MailboxNotFound), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_413_is_too_large() {
        let err = error_for(413, "message_too_large", "big").await;
        assert!(matches!(err, ApimailError::TooLarge), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_422_is_not_parsable() {
        let err = error_for(422, "message_not_parsable", "bad").await;
        assert!(matches!(err, ApimailError::NotParsable), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_502_smtp_error() {
        let err = error_for(502, "smtp_error", "smtp down").await;
        assert!(matches!(err, ApimailError::SmtpError), "err: {err:?}");
    }

    #[tokio::test]
    async fn error_503_is_unavailable() {
        let err = error_for(503, "imap_unavailable", "imap down").await;
        assert!(matches!(err, ApimailError::Unavailable), "err: {err:?}");
    }
}
