//! Cliente de la API REST del núcleo de WordPress (`/wp-json/wp/v2/comments`).
//!
//! Publica respuestas a comentarios del blog autenticándose con una
//! **Application Password** mediante `Authorization: Basic`. La credencial se
//! resuelve de las variables de entorno `BLOG_BASE_URL`, `BLOG_SERVICE_USER` y
//! `BLOG_APP_PASSWORD` en **cada llamada**, jamás se guarda en el cliente, y
//! **nunca** aparece en un `Debug`, un log o el mensaje de un
//! [`WordpressError`]. El `Authorization` se construye en el momento de la
//! petición.
//!
//! Es el molde de [`crate::services::miniflux`] y [`crate::services::apimail`]:
//! base inyectable, `User-Agent` propio, timeout y traducción de cada desenlace
//! a un error tipado. **No se reintenta**: cada invocación envía una sola
//! petición, también ante un `429` o un `5xx`.

/// URL base por defecto cuando `BLOG_BASE_URL` no está definida.
pub const DEFAULT_BASE_URL: &str = "https://atareao.es";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
const REQUEST_TIMEOUT_SECS: u64 = 15;

/// Ruta del endpoint de comentarios de la API REST del núcleo.
const COMMENTS_PATH: &str = "/wp-json/wp/v2/comments";

/// Longitud máxima del extracto del comentario que viaja como **evidencia** al
/// modelo, en caracteres.
const MAX_EXCERPT_CHARS: usize = 200;

/// Errores del cliente de WordPress.
///
/// [`WordpressError::MissingCredentials`] (falta configuración) es una variante
/// **distinta** de [`WordpressError::Unauthorized`] (la credencial no vale) y de
/// [`WordpressError::Forbidden`] (la credencial vale pero no puede publicar).
/// Ningún mensaje incluye la contraseña de aplicación ni la cabecera
/// `Authorization`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordpressError {
    /// Faltan una o varias variables de entorno; se nombran las que faltan.
    MissingCredentials { missing: Vec<&'static str> },
    /// `401` — la credencial no es válida o ha caducado.
    Unauthorized,
    /// `403` — la credencial es válida pero el usuario de servicio no puede
    /// completar la operación (rol insuficiente). El mensaje es **neutral
    /// respecto a la operación**: el mismo `403` sirve a publicar y a borrar.
    Forbidden,
    /// `404` con `code` `rest_no_route` — la ruta solicitada **no existe en el
    /// sitio** (política REST del plugin).
    RouteNotFound,
    /// `404` con otro `code` (`rest_comment_invalid_id`) — el comentario no
    /// existe en el sitio.
    CommentNotFound,
    /// `501` con `code` `rest_trash_not_supported` — el sitio no tiene papelera
    /// (`EMPTY_TRASH_DAYS = 0`); el comentario no se ha borrado.
    TrashUnsupported,
    /// `429` — la cota de escritura está agotada; la respuesta **no** se publicó.
    RateLimited,
    /// `400` — la petición no es válida.
    BadRequest,
    /// Fallo de red, timeout o `5xx` — el sitio no está disponible.
    Unavailable,
    /// Cualquier otro estado. Lleva el código HTTP y el `code` de WordPress,
    /// **sin** su `message` libre (texto de terceros).
    Unexpected { status: u16, code: Option<String> },
}

impl std::fmt::Display for WordpressError {
    /// Mensajes accionables en español. **Ninguno** incluye la contraseña.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WordpressError::MissingCredentials { missing } => write!(
                f,
                "faltan variables de entorno del blog: {}; define el usuario de \
                 servicio y su Application Password",
                missing.join(", ")
            ),
            WordpressError::Unauthorized => write!(
                f,
                "la credencial de servicio del blog no es válida o ha caducado \
                 (revisa BLOG_SERVICE_USER y BLOG_APP_PASSWORD)"
            ),
            WordpressError::Forbidden => write!(
                f,
                "la credencial del blog es válida pero no puede completar esta \
                 operación: revisa el rol del usuario de servicio (necesita \
                 `moderate_comments`)"
            ),
            WordpressError::RouteNotFound => write!(
                f,
                "la ruta solicitada no existe en el sitio del blog: revisa la \
                 política REST del plugin (la ruta de la API REST no está \
                 disponible)"
            ),
            WordpressError::CommentNotFound => {
                write!(f, "el comentario no existe en el sitio del blog")
            }
            WordpressError::TrashUnsupported => write!(
                f,
                "el sitio del blog no admite la papelera (EMPTY_TRASH_DAYS = 0): \
                 el comentario NO se ha borrado"
            ),
            WordpressError::RateLimited => write!(
                f,
                "la cota de escritura del blog está agotada: la respuesta NO se \
                 publicó; no reintentes en bucle"
            ),
            WordpressError::BadRequest => write!(
                f,
                "la petición a WordPress no es válida: revisa los identificadores \
                 del comentario y de la entrada"
            ),
            WordpressError::Unavailable => write!(
                f,
                "el sitio del blog no está disponible (fallo de red, timeout o \
                 error del servidor)"
            ),
            WordpressError::Unexpected { status, code } => match code {
                Some(code) => write!(
                    f,
                    "WordPress respondió un estado inesperado {status} (código: {code})"
                ),
                None => write!(f, "WordPress respondió un estado inesperado {status}"),
            },
        }
    }
}

impl std::error::Error for WordpressError {}

/// Configuración resuelta del cliente de WordPress.
#[derive(Clone, PartialEq, Eq)]
pub struct WordpressConfig {
    /// URL base del sitio (sin barra final).
    pub base_url: String,
    /// Usuario de servicio (`BLOG_SERVICE_USER`).
    pub service_user: String,
    /// Application Password (`BLOG_APP_PASSWORD`).
    pub app_password: String,
    /// Timeout de cada petición. Es **inyectable** para que los tests no
    /// dependan de la carrera entre un reloj pausado y un servidor lento; en
    /// producción vale siempre [`WordpressConfig::default_timeout`].
    pub timeout: std::time::Duration,
}

impl WordpressConfig {
    /// Timeout de producción de cada petición (15 s). Es el valor que
    /// [`Wordpress::resolve_config`] asigna siempre, de modo que un timeout
    /// inyectado en un test **nunca** altera el valor de producción.
    pub fn default_timeout() -> std::time::Duration {
        std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS)
    }
}

impl std::fmt::Debug for WordpressConfig {
    /// Redacta la contraseña: nunca debe aparecer en un `Debug`, pero la URL y
    /// el usuario sí son útiles para diagnosticar.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WordpressConfig")
            .field("base_url", &self.base_url)
            .field("service_user", &self.service_user)
            .field("app_password", &"***")
            .finish()
    }
}

/// El resultado de una creación correcta: el id que devolvió WordPress, si el
/// cuerpo era interpretable (un `201` con cuerpo ilegible lo deja en `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedReply {
    pub wp_comment_id: Option<String>,
}

/// Evidencia de un comentario leído antes de borrarlo. El extracto viene ya
/// **acotado** a [`MAX_EXCERPT_CHARS`] caracteres; el contenido íntegro **no** se
/// conserva en ninguna parte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentSnapshot {
    /// Id del comentario, en forma decimal.
    pub comment_id: String,
    /// Id de la entrada a la que pertenece (el **real**, según WordPress).
    pub post_id: String,
    /// Nombre del autor tal cual lo devuelve WordPress (`""` si no viene).
    pub author: String,
    /// Extracto del contenido, acotado a [`MAX_EXCERPT_CHARS`] caracteres.
    pub excerpt: String,
    /// Estado del comentario **antes** de la operación (p. ej. `approved`).
    pub previous_state: String,
}

/// Desenlace correcto de un borrado a la papelera. Ningún otro desenlace es
/// éxito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashOutcome {
    /// `200` — el comentario ha quedado en la papelera.
    Trashed,
    /// `410 rest_already_trashed` — el comentario **ya** estaba en la papelera.
    AlreadyTrashed,
}

/// Cliente de WordPress atado a una URL base.
#[derive(Debug, Clone)]
pub struct Wordpress {
    http: reqwest::Client,
    base_url: String,
}

impl Wordpress {
    /// Cliente con la base dada, `User-Agent` propio y el **timeout de
    /// producción** ([`WordpressConfig::default_timeout`]). La base es
    /// **inyectable** para poder apuntar los tests a un servidor de prueba.
    pub fn new(base_url: String) -> Self {
        Self::with_timeout(base_url, WordpressConfig::default_timeout())
    }

    /// Cliente con un timeout explícito. Solo lo usan los tests para acortar la
    /// espera sin depender del reloj real; en producción se usa [`Self::new`].
    pub fn with_timeout(base_url: String, timeout: std::time::Duration) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            .build()
            .expect("Failed to build reqwest Client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Resuelve la configuración de las variables de entorno en **cada
    /// llamada**: `BLOG_BASE_URL` (con [`DEFAULT_BASE_URL`] por defecto),
    /// `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD`. Sin usuario o sin contraseña
    /// devuelve [`WordpressError::MissingCredentials`] nombrando las variables
    /// que faltan, **sin** salir a la red.
    pub fn resolve_config() -> Result<WordpressConfig, WordpressError> {
        let base_url =
            env_nonempty("BLOG_BASE_URL").unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let service_user = env_nonempty("BLOG_SERVICE_USER");
        let app_password = env_nonempty("BLOG_APP_PASSWORD");

        match (service_user, app_password) {
            (Some(service_user), Some(app_password)) => Ok(WordpressConfig {
                base_url,
                service_user,
                app_password,
                timeout: WordpressConfig::default_timeout(),
            }),
            (None, None) => Err(WordpressError::MissingCredentials {
                missing: vec!["BLOG_SERVICE_USER", "BLOG_APP_PASSWORD"],
            }),
            (None, Some(_)) => Err(WordpressError::MissingCredentials {
                missing: vec!["BLOG_SERVICE_USER"],
            }),
            (Some(_), None) => Err(WordpressError::MissingCredentials {
                missing: vec!["BLOG_APP_PASSWORD"],
            }),
        }
    }

    /// `POST {base}/wp-json/wp/v2/comments` con el cuerpo `{post, parent,
    /// content, status: "approved"}` y `Authorization: Basic
    /// base64(usuario:contraseña)` **construida en el momento de la petición**
    /// (nunca guardada ni registrada). El cuerpo **no** lleva `author_name` ni
    /// `author_email`: con ellos el comentario nacería pendiente.
    ///
    /// Solo un `201` es éxito; un `201` con cuerpo ilegible sigue siendo éxito
    /// (el comentario ya está publicado) con el id ausente. Cualquier otro
    /// desenlace se traduce a un [`WordpressError`] y **no** se reintenta.
    pub async fn publish_comment_reply(
        &self,
        config: &WordpressConfig,
        post_id: &str,
        comment_id: &str,
        content: &str,
    ) -> Result<PublishedReply, WordpressError> {
        let body = serde_json::json!({
            "post": id_json(post_id),
            "parent": id_json(comment_id),
            "content": content,
            "status": "approved",
        });

        let response = self
            .http
            .post(format!("{}{COMMENTS_PATH}", self.base_url))
            .basic_auth(&config.service_user, Some(&config.app_password))
            .json(&body)
            .send()
            .await
            .map_err(|_| WordpressError::Unavailable)?;

        let status = response.status().as_u16();
        if status == 201 {
            let wp_comment_id = response
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|value| value.get("id").and_then(json_id));
            return Ok(PublishedReply { wp_comment_id });
        }

        let code = extract_code(response).await;
        Err(error_from_status(status, code))
    }

    /// `GET {base}/wp-json/wp/v2/comments/{comment_id}` con `Authorization:
    /// Basic` (construida en la petición) y **sin** cuerpo. Es la **lectura
    /// previa** del borrado: comprueba que el comentario existe y es la fuente de
    /// la evidencia (autor, extracto acotado, estado y `post_id` real).
    ///
    /// Solo un `200` con cuerpo interpretable es éxito; cualquier otro estado se
    /// traduce a un [`WordpressError`] y un cuerpo ilegible es una lectura
    /// fallida ([`WordpressError::Unavailable`]), **sin** pánico y **sin**
    /// reintento.
    pub async fn fetch_comment(
        &self,
        config: &WordpressConfig,
        comment_id: &str,
    ) -> Result<CommentSnapshot, WordpressError> {
        let response = self
            .http
            .get(format!("{}{COMMENTS_PATH}/{comment_id}", self.base_url))
            .basic_auth(&config.service_user, Some(&config.app_password))
            .send()
            .await
            .map_err(|_| WordpressError::Unavailable)?;

        let status = response.status().as_u16();
        if status != 200 {
            let code = extract_code(response).await;
            return Err(error_from_status(status, code));
        }

        let body = response
            .text()
            .await
            .map_err(|_| WordpressError::Unavailable)?;
        parse_comment_snapshot(&body, comment_id).ok_or(WordpressError::Unavailable)
    }

    /// `DELETE {base}/wp-json/wp/v2/comments/{comment_id}?force=false` con
    /// `Authorization: Basic` y **sin** cuerpo. Manda el comentario a la
    /// **papelera**: `force=true` **nunca** se envía.
    ///
    /// Un `200` es [`TrashOutcome::Trashed`] y un `410 rest_already_trashed` es
    /// [`TrashOutcome::AlreadyTrashed`] (idempotente). Cualquier otro desenlace
    /// se traduce a un [`WordpressError`] y no se reintenta.
    pub async fn trash_comment(
        &self,
        config: &WordpressConfig,
        comment_id: &str,
    ) -> Result<TrashOutcome, WordpressError> {
        let response = self
            .http
            .delete(format!(
                "{}{COMMENTS_PATH}/{comment_id}?force=false",
                self.base_url
            ))
            .basic_auth(&config.service_user, Some(&config.app_password))
            .send()
            .await
            .map_err(|_| WordpressError::Unavailable)?;

        let status = response.status().as_u16();
        if status == 200 {
            return Ok(TrashOutcome::Trashed);
        }

        let code = extract_code(response).await;
        if status == 410 && code.as_deref() == Some("rest_already_trashed") {
            return Ok(TrashOutcome::AlreadyTrashed);
        }
        Err(error_from_status(status, code))
    }
}

/// Lee una variable de entorno y la `trim`ea; `None` si falta o está en blanco.
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// El identificador como número JSON si es decimal, o como cadena si no lo es
/// (nunca falla; la validación de la forma ocurre antes, en la herramienta).
fn id_json(raw: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(raw)
        .unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

/// Extrae el `id` de un cuerpo de WordPress (número o cadena) como texto.
fn json_id(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::String(text) => Some(text.clone()),
        _ => None,
    }
}

/// Lee el cuerpo de una respuesta de error y extrae su `code`, si lo hay.
async fn extract_code(response: reqwest::Response) -> Option<String> {
    let body = response.text().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    parsed
        .get("code")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// Traduce un estado HTTP de error de WordPress a su [`WordpressError`].
///
/// El `404` se **desambigua por el `code`**: `rest_no_route` es una ruta ausente
/// en el sitio y cualquier otro `code` (`rest_comment_invalid_id`) es un
/// comentario inexistente. El `501 rest_trash_not_supported` es un sitio sin
/// papelera. El `code` solo se conserva, además, para la variante inesperada.
fn error_from_status(status: u16, code: Option<String>) -> WordpressError {
    match status {
        400 => WordpressError::BadRequest,
        401 => WordpressError::Unauthorized,
        403 => WordpressError::Forbidden,
        404 => match code.as_deref() {
            Some("rest_no_route") => WordpressError::RouteNotFound,
            _ => WordpressError::CommentNotFound,
        },
        429 => WordpressError::RateLimited,
        501 if code.as_deref() == Some("rest_trash_not_supported") => {
            WordpressError::TrashUnsupported
        }
        500..=599 => WordpressError::Unavailable,
        _ => WordpressError::Unexpected { status, code },
    }
}

/// Interpreta el cuerpo de un `GET` de comentario y construye la evidencia. El
/// `id` cae al solicitado si WordPress no lo devuelve; el `post` es obligatorio
/// (sin él no hay evidencia utilizable y se trata como lectura fallida).
fn parse_comment_snapshot(body: &str, requested_id: &str) -> Option<CommentSnapshot> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let object = value.as_object()?;

    let comment_id = object
        .get("id")
        .and_then(json_id)
        .unwrap_or_else(|| requested_id.to_string());
    let post_id = object.get("post").and_then(json_id)?;
    let author = object
        .get("author_name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let content = object
        .get("content")
        .and_then(|content| content.get("rendered"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let previous_state = object
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    Some(CommentSnapshot {
        comment_id,
        post_id,
        author,
        excerpt: truncate_chars(content, MAX_EXCERPT_CHARS),
        previous_state,
    })
}

/// Recorta un texto a `max` caracteres (por límites de carácter, no de byte).
fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serial_test::serial;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn set_env(key: &str, value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    /// Configuración apuntando al servidor de prueba con credenciales conocidas.
    fn config_for(server: &MockServer) -> WordpressConfig {
        WordpressConfig {
            base_url: server.uri(),
            service_user: "alice".to_string(),
            app_password: "s3cret-app-password".to_string(),
            timeout: WordpressConfig::default_timeout(),
        }
    }

    /// La cabecera `Authorization: Basic base64(usuario:contraseña)` esperada.
    fn expected_basic(user: &str, password: &str) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
        )
    }

    // -----------------------------------------------------------------------
    // 2.1 — resolución de la configuración en cada llamada
    // -----------------------------------------------------------------------

    /// Scenario: la configuración se resuelve de nuevo en cada llamada, de modo
    /// que un cambio en el entorno surte efecto sin reiniciar.
    #[test]
    #[serial]
    fn resolve_config_reads_env_each_call() {
        set_env("BLOG_SERVICE_USER", Some("alice"));
        set_env("BLOG_APP_PASSWORD", Some("uno"));
        set_env("BLOG_BASE_URL", Some("https://blog.test"));

        let first = Wordpress::resolve_config().unwrap();
        assert_eq!(first.service_user, "alice");
        assert_eq!(first.app_password, "uno");
        assert_eq!(first.base_url, "https://blog.test");

        set_env("BLOG_SERVICE_USER", Some("bob"));
        set_env("BLOG_APP_PASSWORD", Some("dos"));
        let second = Wordpress::resolve_config().unwrap();
        assert_eq!(second.service_user, "bob");
        assert_eq!(second.app_password, "dos");

        set_env("BLOG_SERVICE_USER", None);
        set_env("BLOG_APP_PASSWORD", None);
        set_env("BLOG_BASE_URL", None);
    }

    /// Scenario: sin usuario ni contraseña el error nombra **las dos** variables
    /// que faltan y no hay ninguna petición.
    #[tokio::test]
    #[serial]
    async fn missing_credentials_names_both_variables() {
        let server = MockServer::start().await;
        set_env("BLOG_SERVICE_USER", None);
        set_env("BLOG_APP_PASSWORD", None);

        let err = Wordpress::resolve_config().unwrap_err();
        let message = err.to_string();
        assert!(
            matches!(&err, WordpressError::MissingCredentials { .. }),
            "err: {err:?}"
        );
        assert!(message.contains("BLOG_SERVICE_USER"), "{message}");
        assert!(message.contains("BLOG_APP_PASSWORD"), "{message}");

        // Sin credenciales no se envía nada.
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin credenciales no debe haber peticiones"
        );
    }

    /// Scenario: sin contraseña (pero con usuario) el error nombra la variable
    /// que falta.
    #[test]
    #[serial]
    fn missing_password_names_it() {
        set_env("BLOG_SERVICE_USER", Some("alice"));
        set_env("BLOG_APP_PASSWORD", None);

        let err = Wordpress::resolve_config().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("BLOG_APP_PASSWORD"), "{message}");
        assert!(
            !message.contains("BLOG_SERVICE_USER"),
            "no debe nombrar la variable presente: {message}"
        );

        set_env("BLOG_SERVICE_USER", None);
    }

    /// Scenario: sin `BLOG_BASE_URL` la base es `https://atareao.es`.
    #[test]
    #[serial]
    fn resolve_config_defaults_base_url() {
        set_env("BLOG_BASE_URL", None);
        set_env("BLOG_SERVICE_USER", Some("alice"));
        set_env("BLOG_APP_PASSWORD", Some("uno"));

        let config = Wordpress::resolve_config().unwrap();
        assert_eq!(config.base_url, "https://atareao.es");
        assert_eq!(config.base_url, DEFAULT_BASE_URL);

        set_env("BLOG_SERVICE_USER", None);
        set_env("BLOG_APP_PASSWORD", None);
    }

    /// Scenario: la petición va a `/wp-json/wp/v2/comments` con `Authorization:
    /// Basic` y el cuerpo `post`, `parent`, `content` y `status: approved`.
    #[tokio::test]
    async fn publishes_to_comments_with_basic_auth_and_body() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .and(header(
                "authorization",
                expected_basic("alice", "s3cret-app-password").as_str(),
            ))
            .and(body_json(serde_json::json!({
                "post": 42,
                "parent": 901,
                "content": "Gracias por el aviso",
                "status": "approved"
            })))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let reply = client
            .publish_comment_reply(&config, "42", "901", "Gracias por el aviso")
            .await
            .unwrap();

        assert_eq!(reply.wp_comment_id.as_deref(), Some("7701"));

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let auth = requests[0]
            .headers
            .get("authorization")
            .expect("la petición debe llevar Authorization")
            .to_str()
            .unwrap();
        assert_eq!(auth, expected_basic("alice", "s3cret-app-password"));
    }

    /// Scenario: un `401` se traduce a credencial inválida y la contraseña
    /// **no** aparece ni en el mensaje ni en el `Debug`, tampoco la cabecera.
    #[tokio::test]
    async fn error_401_is_unauthorized_and_hides_password() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(401).set_body_string(format!(
                "unauthorized: {}\nAuthorization: Basic xyz",
                config.app_password
            )))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::Unauthorized), "err: {err:?}");
        let message = err.to_string();
        let debug = format!("{err:?}");
        assert!(
            !message.contains(&config.app_password),
            "el mensaje filtra la contraseña: {message}"
        );
        assert!(
            !message.contains("Authorization"),
            "el mensaje filtra la cabecera: {message}"
        );
        assert!(
            !debug.contains(&config.app_password),
            "el Debug filtra la contraseña: {debug}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// Scenario: un `403` habla del rol del usuario de servicio y es un error
    /// **distinto** del de credencial inválida.
    #[tokio::test]
    async fn error_403_is_forbidden_and_distinct() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({ "code": "rest_cannot_create" })),
            )
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::Forbidden), "err: {err:?}");
        assert_ne!(
            format!("{err:?}"),
            format!("{:?}", WordpressError::Unauthorized),
            "un 403 y un 401 deben ser errores distintos"
        );
        let message = err.to_string().to_lowercase();
        assert!(
            message.contains("rol"),
            "el mensaje debe hablar del rol: {message}"
        );
    }

    // -----------------------------------------------------------------------
    // 2.2 — resto del mapa de errores y el Debug
    // -----------------------------------------------------------------------

    /// Scenario: un `429` informa de la cota agotada, dice que la respuesta
    /// **no** se publicó y el servidor recibe **exactamente una** petición.
    #[tokio::test]
    async fn error_429_is_rate_limited_without_retry() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "code": "atareao_valet_reply_rate_limited",
                "message": "too many requests"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::RateLimited), "err: {err:?}");
        let message = err.to_string().to_lowercase();
        assert!(
            message.contains("no se public"),
            "el mensaje debe decir que no se publicó: {message}"
        );
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "un 429 no se reintenta"
        );
    }

    /// Scenario: un `400` es petición inválida y **no** indisponibilidad.
    #[tokio::test]
    async fn error_400_is_bad_request_not_unavailable() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "code": "rest_invalid_param",
                "message": "invalid"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::BadRequest), "err: {err:?}");
        assert!(
            !matches!(&err, WordpressError::Unavailable),
            "un 400 no es indisponibilidad"
        );
    }

    /// Scenario: un `503` se traduce a sitio no disponible.
    #[tokio::test]
    async fn error_503_is_unavailable() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(503).set_body_string("down"))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::Unavailable), "err: {err:?}");
        let message = err.to_string().to_lowercase();
        assert!(
            message.contains("no está disponible"),
            "el mensaje debe hablar de disponibilidad: {message}"
        );
    }

    /// Scenario: un fallo de red (servidor inalcanzable) se traduce a sitio no
    /// disponible, sin pánico.
    #[tokio::test]
    async fn network_failure_is_unavailable() {
        let config = WordpressConfig {
            base_url: "http://127.0.0.1:1".to_string(),
            service_user: "alice".to_string(),
            app_password: "s3cret".to_string(),
            timeout: WordpressConfig::default_timeout(),
        };
        let client = Wordpress::new(config.base_url.clone());

        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        assert!(matches!(&err, WordpressError::Unavailable), "err: {err:?}");
    }

    /// Scenario: un `418` con `code` y `message` produce un error que lleva el
    /// código HTTP y el `code`, pero **no** el `message` libre.
    #[tokio::test]
    async fn error_418_carries_status_and_code_not_message() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(418).set_body_json(serde_json::json!({
                "code": "teapot",
                "message": "soy una tetera y no deberías leer esto"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();

        match &err {
            WordpressError::Unexpected { status, code } => {
                assert_eq!(*status, 418);
                assert_eq!(code.as_deref(), Some("teapot"));
            }
            other => panic!("esperaba Unexpected, obtuve {other:?}"),
        }
        let message = err.to_string();
        assert!(message.contains("418"), "{message}");
        assert!(message.contains("teapot"), "{message}");
        assert!(
            !message.contains("tetera"),
            "el mensaje libre de WordPress no debe filtrarse: {message}"
        );
    }

    /// Scenario: un `201` con cuerpo ilegible se trata como **éxito** con el id
    /// ausente y no se repite la petición.
    #[tokio::test]
    async fn created_with_unreadable_body_is_success_without_id() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(201).set_body_string("no soy json"))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let reply = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .expect("un 201 con cuerpo ilegible es un éxito");

        assert_eq!(reply.wp_comment_id, None);
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "no debe repetirse la petición"
        );
    }

    /// Scenario: el `Debug` de la configuración redacta la contraseña y muestra
    /// la URL base y el usuario.
    #[test]
    fn debug_redacts_password_and_shows_url_and_user() {
        let config = WordpressConfig {
            base_url: "https://blog.test".to_string(),
            service_user: "alice".to_string(),
            app_password: "super-secret-app-pass".to_string(),
            timeout: WordpressConfig::default_timeout(),
        };

        let debug = format!("{config:?}");

        assert!(
            !debug.contains("super-secret-app-pass"),
            "el Debug filtra la contraseña: {debug}"
        );
        assert!(debug.contains("***"), "{debug}");
        assert!(debug.contains("https://blog.test"), "{debug}");
        assert!(debug.contains("alice"), "{debug}");
    }

    /// Aserción de compilación: el futuro de publicación debe ser `Send` (el
    /// trait `Tool` lo exige).
    #[test]
    fn publish_future_is_send() {
        fn assert_send<T: Send>(_: T) {}
        let config = WordpressConfig {
            base_url: "http://localhost:1".to_string(),
            service_user: "alice".to_string(),
            app_password: "s3cret".to_string(),
            timeout: WordpressConfig::default_timeout(),
        };
        let client = Wordpress::new(config.base_url.clone());
        assert_send(client.publish_comment_reply(&config, "42", "901", "hola"));
    }

    /// Scenario: el timeout inyectado **no** cambia el valor de producción: la
    /// configuración resuelta del entorno lleva siempre el timeout por defecto
    /// de 15 s, y el atajo de test no lo altera.
    #[test]
    #[serial]
    fn production_timeout_is_fifteen_seconds() {
        assert_eq!(
            WordpressConfig::default_timeout(),
            std::time::Duration::from_secs(15),
            "el timeout de producción debe ser 15 s"
        );

        set_env("BLOG_SERVICE_USER", Some("alice"));
        set_env("BLOG_APP_PASSWORD", Some("uno"));
        let config = Wordpress::resolve_config().unwrap();
        assert_eq!(
            config.timeout,
            WordpressConfig::default_timeout(),
            "resolve_config debe usar el timeout de producción"
        );

        set_env("BLOG_SERVICE_USER", None);
        set_env("BLOG_APP_PASSWORD", None);
    }

    // -----------------------------------------------------------------------
    // 3.1 — lectura previa del ítem (`fetch_comment`)
    // -----------------------------------------------------------------------

    /// Scenario: la lectura envía un `GET` al ítem con `Authorization: Basic` y
    /// **sin** cuerpo, y devuelve la evidencia normalizada.
    #[tokio::test]
    async fn fetch_comment_sends_get_with_basic_auth_and_no_body() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .and(header(
                "authorization",
                expected_basic("alice", "s3cret-app-password").as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 901,
                "post": 42,
                "author_name": "Ana",
                "content": { "rendered": "Un comentario de prueba" },
                "status": "approved"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let snapshot = client.fetch_comment(&config, "901").await.unwrap();

        assert_eq!(snapshot.comment_id, "901");
        assert_eq!(snapshot.post_id, "42");
        assert_eq!(snapshot.author, "Ana");
        assert_eq!(snapshot.excerpt, "Un comentario de prueba");
        assert_eq!(snapshot.previous_state, "approved");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method.as_str(), "GET");
        assert!(
            requests[0].body.is_empty(),
            "la lectura no debe llevar cuerpo: {:?}",
            requests[0].body
        );
    }

    /// Scenario: un cuerpo ilegible no provoca pánico: es una lectura fallida.
    #[tokio::test]
    async fn fetch_comment_unreadable_body_is_read_failure_without_panic() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(200).set_body_string("no soy json"))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.fetch_comment(&config, "901").await.unwrap_err();
        assert!(matches!(&err, WordpressError::Unavailable), "err: {err:?}");
    }

    /// Scenario: leer un comentario inexistente se traduce a `CommentNotFound`
    /// (nunca a un estado inesperado).
    #[tokio::test]
    async fn fetch_comment_404_invalid_id_is_comment_not_found() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": "rest_comment_invalid_id",
                "message": "Invalid comment ID."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.fetch_comment(&config, "901").await.unwrap_err();
        assert!(
            matches!(&err, WordpressError::CommentNotFound),
            "err: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // 3.2 — borrado del ítem (`trash_comment`)
    // -----------------------------------------------------------------------

    /// Scenario: el borrado envía un `DELETE` con `force=false` como parámetro de
    /// consulta, **sin** cuerpo y con `Authorization: Basic`; un `200` es éxito.
    #[tokio::test]
    async fn trash_comment_sends_delete_with_force_false_and_no_body() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .and(query_param("force", "false"))
            .and(header(
                "authorization",
                expected_basic("alice", "s3cret-app-password").as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 901,
                "status": "trash"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let outcome = client.trash_comment(&config, "901").await.unwrap();
        assert_eq!(outcome, TrashOutcome::Trashed);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method.as_str(), "DELETE");
        assert!(
            requests[0].body.is_empty(),
            "el borrado nunca debe llevar cuerpo: {:?}",
            requests[0].body
        );
        let query = requests[0].url.query().unwrap_or("");
        assert!(query.contains("force=false"), "falta force=false: {query}");
        assert!(
            !query.contains("force=true"),
            "el borrado nunca debe enviar force=true: {query}"
        );
    }

    // -----------------------------------------------------------------------
    // 4.1 — mapa completo de desenlaces del borrado
    // -----------------------------------------------------------------------

    /// Scenario: un `410 rest_already_trashed` es «ya estaba hecho», distinto del
    /// éxito de un `200` y del comentario inexistente.
    #[tokio::test]
    async fn trash_comment_410_already_trashed_is_idempotent_success() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(410).set_body_json(serde_json::json!({
                "code": "rest_already_trashed",
                "message": "El comentario ya ha sido movido a la papelera."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let outcome = client.trash_comment(&config, "901").await.unwrap();
        assert_eq!(outcome, TrashOutcome::AlreadyTrashed);
        assert_ne!(outcome, TrashOutcome::Trashed);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// Scenario: un `404 rest_comment_invalid_id` es «el comentario no existe».
    #[tokio::test]
    async fn trash_comment_404_invalid_id_is_comment_not_found() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": "rest_comment_invalid_id",
                "message": "Invalid comment ID."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(
            matches!(&err, WordpressError::CommentNotFound),
            "err: {err:?}"
        );
    }

    /// Scenario: un `404 rest_no_route` es la ruta ausente, distinto del
    /// comentario inexistente, y el mensaje no filtra la credencial.
    #[tokio::test]
    async fn trash_comment_404_no_route_is_route_not_found_and_distinct() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": "rest_no_route",
                "message": "No route was found matching the URL and request method."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(
            matches!(&err, WordpressError::RouteNotFound),
            "err: {err:?}"
        );
        assert_ne!(
            format!("{err:?}"),
            format!("{:?}", WordpressError::CommentNotFound),
            "la ruta ausente y el comentario inexistente deben ser errores distintos"
        );
        let message = err.to_string().to_lowercase();
        assert!(message.contains("ruta"), "{message}");
        assert!(
            message.contains("política") || message.contains("politica"),
            "{message}"
        );
        assert!(!message.contains(&config.app_password), "{message}");
        assert!(!message.contains("authorization"), "{message}");
    }

    /// Scenario: un `403` reutiliza `Forbidden`, habla del rol y es distinto de
    /// un `401`.
    #[tokio::test]
    async fn trash_comment_403_forbidden_mentions_role_distinct_from_401() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({ "code": "rest_cannot_delete" })),
            )
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(matches!(&err, WordpressError::Forbidden), "err: {err:?}");
        assert_ne!(
            format!("{err:?}"),
            format!("{:?}", WordpressError::Unauthorized),
            "un 403 y un 401 deben ser errores distintos"
        );
        let message = err.to_string().to_lowercase();
        assert!(message.contains("rol"), "{message}");
    }

    /// Scenario: un `501 rest_trash_not_supported` dice que el sitio no tiene
    /// papelera y que el comentario no se ha borrado.
    #[tokio::test]
    async fn trash_comment_501_is_trash_unsupported() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(501).set_body_json(serde_json::json!({
                "code": "rest_trash_not_supported",
                "message": "Trashing is not supported."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(
            matches!(&err, WordpressError::TrashUnsupported),
            "err: {err:?}"
        );
        let message = err.to_string().to_lowercase();
        assert!(message.contains("papelera"), "{message}");
        assert!(
            message.contains("no se ha borrado") || message.contains("no borra"),
            "{message}"
        );
    }

    /// Scenario: un `503` es sitio no disponible con **exactamente una** petición.
    #[tokio::test]
    async fn trash_comment_503_is_unavailable_with_single_request() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(503).set_body_string("down"))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(matches!(&err, WordpressError::Unavailable), "err: {err:?}");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// Scenario: un `429` no se reintenta.
    #[tokio::test]
    async fn trash_comment_429_no_retry() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({
                "code": "atareao_valet_reply_rate_limited",
                "message": "too many requests"
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client.trash_comment(&config, "901").await.unwrap_err();
        assert!(matches!(&err, WordpressError::RateLimited), "err: {err:?}");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    // -----------------------------------------------------------------------
    // 4.2 — extensión al publicador: también desambigua la ruta ausente
    // -----------------------------------------------------------------------

    /// Scenario: el publicador de respuestas ante un `404 rest_no_route` produce
    /// el error propio de ruta ausente (no un estado inesperado) y no reintenta.
    #[tokio::test]
    async fn publish_comment_reply_404_no_route_is_route_not_found() {
        let server = MockServer::start().await;
        let config = config_for(&server);
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "code": "rest_no_route",
                "message": "No route was found matching the URL and request method."
            })))
            .mount(&server)
            .await;

        let client = Wordpress::new(config.base_url.clone());
        let err = client
            .publish_comment_reply(&config, "42", "901", "hola")
            .await
            .unwrap_err();
        assert!(
            matches!(&err, WordpressError::RouteNotFound),
            "err: {err:?}"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    // -----------------------------------------------------------------------
    // B1 — la frontera multibyte del extracto
    // -----------------------------------------------------------------------

    /// Scenario: el extracto se recorta a 200 **caracteres**, no a 200 bytes: con
    /// contenido multibyte y un emoji corta en 200 sin romper el UTF-8 ni crecer.
    #[test]
    fn truncate_chars_cuts_at_character_boundary_with_multibyte() {
        // 199 ASCII + un emoji (4 bytes) + 1 ASCII = 201 caracteres, 204 bytes.
        let mut text = "a".repeat(199);
        text.push('😀');
        text.push('b');
        assert_eq!(text.chars().count(), 201);

        let out = truncate_chars(&text, 200);
        assert_eq!(
            out.chars().count(),
            200,
            "debe cortar en 200 caracteres, no en 200 bytes"
        );
        assert_eq!(out.len(), 199 + 4, "199 ASCII + el emoji de 4 bytes");
        assert!(out.ends_with('😀'), "la frontera no debe romper el UTF-8");

        // 200 caracteres multibyte se conservan íntegros (no decrece).
        let exact = "á".repeat(200);
        assert_eq!(truncate_chars(&exact, 200), exact);

        // 201 caracteres multibyte de 2 bytes → 200 caracteres, 400 bytes.
        let over = "ñ".repeat(201);
        let cut = truncate_chars(&over, 200);
        assert_eq!(cut.chars().count(), 200);
        assert_eq!(cut.len(), 400);
    }
}
