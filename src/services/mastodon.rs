//! Cliente de la API REST de Mastodon para menciones y respuestas.
//!
//! La configuración (`mastodon_base_url` y `mastodon_token`) se resuelve de
//! `settings` en cada llamada; el token nunca aparece en un `Debug`, un log ni
//! un mensaje de error.

use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;

/// Errores tipados del cliente de Mastodon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MastodonError {
    /// Falta el token; no se toca la red.
    NotConfigured,
    /// Falta la URL base (no hay valor por defecto); no se toca la red.
    MissingBaseUrl,
    /// `401`/`403`: token rechazado o sin permisos.
    Unauthorized,
    /// `404`: el status o el endpoint no existe.
    NotFound,
    /// `422`: Mastodon rechazó los datos enviados.
    InvalidInput(String),
    /// `429`: límite de peticiones superado.
    RateLimited,
    /// `5xx` o fallo de red.
    Upstream(String),
}

impl std::fmt::Display for MastodonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MastodonError::NotConfigured => write!(
                f,
                "Mastodon no está configurado: pega el token en Ajustes → Integraciones → Mastodon"
            ),
            MastodonError::MissingBaseUrl => write!(
                f,
                "Mastodon no tiene URL base: configura mastodon_base_url con la URL de tu instancia en Ajustes → Integraciones → Mastodon (no hay valor por defecto)"
            ),
            MastodonError::Unauthorized => write!(
                f,
                "Mastodon rechazó el token: revisa que sea válido y tenga los scopes read:notifications y write:statuses"
            ),
            MastodonError::NotFound => write!(f, "Mastodon devolvió 404: el status no existe"),
            MastodonError::InvalidInput(detail) => {
                write!(f, "Mastodon rechazó los datos: {detail}")
            }
            MastodonError::RateLimited => write!(
                f,
                "Mastodon ha limitado las peticiones: espera antes de volver a intentarlo"
            ),
            MastodonError::Upstream(detail) => write!(f, "Mastodon no responde: {detail}"),
        }
    }
}

impl std::error::Error for MastodonError {}

/// Configuración resuelta de los ajustes.
#[derive(Clone, PartialEq, Eq)]
pub struct MastodonConfig {
    /// Token de acceso personal (nunca se muestra en `Debug`).
    pub token: String,
    /// URL base de la instancia, sin barra final.
    pub base_url: String,
}

impl std::fmt::Debug for MastodonConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MastodonConfig")
            .field("token", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// Una mención ya convertida a texto plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub notification_id: String,
    pub status_id: String,
    pub author: String,
    pub text: String,
    pub visibility: String,
    pub url: String,
}

/// Cliente atado a una instancia y unos credenciales.
#[derive(Debug, Clone)]
pub struct Mastodon {
    http: reqwest::Client,
    config: MastodonConfig,
}

impl Mastodon {
    /// Construye el cliente con la configuración dada.
    pub fn new(config: MastodonConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("valet/1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            config: MastodonConfig {
                token: config.token,
                base_url: config.base_url.trim_end_matches('/').to_string(),
            },
        }
    }

    /// Resuelve la configuración de `settings` en cada llamada.
    pub async fn resolve_config(pool: &SqlitePool) -> Result<MastodonConfig, MastodonError> {
        let token = setting_value(pool, "mastodon_token")
            .await
            .ok_or(MastodonError::NotConfigured)?;
        let base_url = setting_value(pool, "mastodon_base_url")
            .await
            .ok_or(MastodonError::MissingBaseUrl)?;
        Ok(MastodonConfig { token, base_url })
    }

    /// Resuelve la configuración y construye el cliente.
    pub async fn from_settings(pool: &SqlitePool) -> Result<Self, MastodonError> {
        let config = Self::resolve_config(pool).await?;
        Ok(Self::new(config))
    }

    /// Configuración resuelta (con el token redactado en `Debug`).
    pub fn config(&self) -> &MastodonConfig {
        &self.config
    }

    /// Sonda real: `GET /api/v1/notifications?limit=1&types[]=mention`.
    pub async fn check_connection(&self) -> Result<(), MastodonError> {
        let response = self
            .http
            .get(notifications_url(&self.config.base_url, 1, None))
            .bearer_auth(&self.config.token)
            .send()
            .await
            .map_err(|_| MastodonError::Upstream("fallo de red".to_string()))?;
        let status = response.status().as_u16();
        if status == 200 {
            return Ok(());
        }
        Err(error_from_status(
            status,
            extract_message(response, &self.config.token).await,
        ))
    }

    /// Lista menciones con `limit`, `max_id` opcional y conversión HTML→texto.
    pub async fn list_mentions(
        &self,
        limit: u32,
        max_id: Option<&str>,
    ) -> Result<Vec<Mention>, MastodonError> {
        let url = notifications_url(&self.config.base_url, limit.clamp(1, 40), max_id);

        let response = self
            .http
            .get(url)
            .bearer_auth(&self.config.token)
            .send()
            .await
            .map_err(|_| MastodonError::Upstream("fallo de red".to_string()))?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(error_from_status(
                status,
                extract_message(response, &self.config.token).await,
            ));
        }
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| MastodonError::Upstream("respuesta no válida".to_string()))?;
        let entries = body
            .as_array()
            .ok_or_else(|| MastodonError::Upstream("respuesta no válida".to_string()))?;

        let mut mentions = Vec::new();
        for entry in entries {
            if entry.get("type").and_then(|value| value.as_str()) != Some("mention") {
                continue;
            }
            let Some(status) = entry.get("status") else {
                continue;
            };
            let field = |key: &str| {
                status
                    .get(key)
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            mentions.push(Mention {
                notification_id: entry
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status_id: field("id"),
                author: entry
                    .get("account")
                    .and_then(|account| account.get("acct"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                text: html_to_text(&field("content")),
                visibility: field("visibility"),
                url: field("url"),
            });
        }
        Ok(mentions)
    }

    /// Publica una respuesta a un status con `Idempotency-Key` fresco.
    pub async fn reply(
        &self,
        status_id: &str,
        text: &str,
        visibility: &str,
        spoiler_text: Option<&str>,
    ) -> Result<String, MastodonError> {
        let mut body = serde_json::Map::new();
        body.insert(
            "status".to_string(),
            serde_json::Value::String(text.to_string()),
        );
        body.insert(
            "in_reply_to_id".to_string(),
            serde_json::Value::String(status_id.to_string()),
        );
        body.insert(
            "visibility".to_string(),
            serde_json::Value::String(visibility.to_string()),
        );
        if let Some(spoiler) = spoiler_text {
            body.insert(
                "spoiler_text".to_string(),
                serde_json::Value::String(spoiler.to_string()),
            );
        }

        let response = self
            .http
            .post(format!("{}/api/v1/statuses", self.config.base_url))
            .bearer_auth(&self.config.token)
            .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .json(&serde_json::Value::Object(body))
            .send()
            .await
            .map_err(|_| MastodonError::Upstream("fallo de red".to_string()))?;
        let status = response.status().as_u16();
        if !(200..=299).contains(&status) {
            return Err(error_from_status(
                status,
                extract_message(response, &self.config.token).await,
            ));
        }
        let created: serde_json::Value = response
            .json()
            .await
            .map_err(|_| MastodonError::Upstream("respuesta no válida".to_string()))?;
        let url = created
            .get("url")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                MastodonError::Upstream(
                    "respuesta no válida: falta la URL del status creado".to_string(),
                )
            })?;
        Ok(url)
    }
}

/// Convierte el HTML de un status a texto plano sin añadir dependencias.
pub fn html_to_text(html: &str) -> String {
    const MAX_CHARS: usize = 20_000;
    let bytes = html.as_bytes();
    let mut raw = String::with_capacity(html.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'<' => {
                if let Some(end) = html[index + 1..].find('>') {
                    let tag = &html[index + 1..index + 1 + end];
                    let closing = tag.starts_with('/');
                    let name = tag
                        .trim_start_matches('/')
                        .split(|character: char| {
                            character.is_ascii_whitespace() || character == '/'
                        })
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    let block = matches!(
                        name.as_str(),
                        "p" | "div"
                            | "blockquote"
                            | "li"
                            | "ul"
                            | "ol"
                            | "pre"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                    );
                    if name == "br" || (closing && block) {
                        raw.push('\n');
                    }
                    index += end + 2;
                } else {
                    raw.push('<');
                    index += 1;
                }
            }
            b'&' => {
                let rest = &html[index + 1..];
                let end = rest.find(';').filter(|end| *end <= 12);
                let mut decoded_entity = None;
                if let Some(end) = end {
                    decoded_entity = decode_entity(&rest[..end]).map(|decoded| (decoded, end));
                }
                if let Some((decoded, end)) = decoded_entity {
                    raw.push_str(&decoded);
                    index += end + 2;
                } else {
                    raw.push('&');
                    index += 1;
                }
            }
            _ => {
                let character = html[index..].chars().next().expect("índice válido");
                raw.push(character);
                index += character.len_utf8();
            }
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            if lines.last().is_some_and(|last| !last.is_empty()) {
                lines.push(String::new());
            }
        } else {
            lines.push(collapsed);
        }
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let mut text = lines.join("\n");
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect();
    }
    text
}

/// Decodifica una entidad HTML habitual, incluidas las entidades numéricas.
fn decode_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => return Some("&".to_string()),
        "lt" => return Some("<".to_string()),
        "gt" => return Some(">".to_string()),
        "quot" => return Some("\"".to_string()),
        "apos" | "#39" => return Some("'".to_string()),
        "nbsp" => return Some(" ".to_string()),
        _ => {}
    }
    let digits = entity.strip_prefix('#')?;
    let code = if let Some(hex) = digits
        .strip_prefix('x')
        .or_else(|| digits.strip_prefix('X'))
    {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        digits.parse().ok()?
    };
    char::from_u32(code).map(|character| character.to_string())
}

/// Construye la URL de notificaciones. `reqwest` no tiene el feature `query`
/// habilitado en este proyecto, así que se serializa con `form_urlencoded`.
fn notifications_url(base_url: &str, limit: u32, max_id: Option<&str>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("limit", &limit.to_string());
    serializer.append_pair("types[]", "mention");
    if let Some(max_id) = max_id {
        serializer.append_pair("max_id", max_id);
    }
    format!("{base_url}/api/v1/notifications?{}", serializer.finish())
}

/// Lee un ajuste de `settings`, recortado; `None` si falta o está en blanco.
pub(crate) async fn setting_value(pool: &SqlitePool, key: &str) -> Option<String> {
    SettingsRepo::get(pool, key)
        .await
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Extrae el mensaje de error de la respuesta y redacta cualquier eco del token
/// o de la cabecera `Authorization: Bearer`. Una instancia hostil no puede
/// convertir el detalle en un canal de filtración.
async fn extract_message(response: reqwest::Response, token: &str) -> Option<String> {
    let body = response.text().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    let raw = ["error", "message", "error_description"]
        .iter()
        .filter_map(|key| parsed.get(*key).and_then(|value| value.as_str()))
        .next()?;
    Some(sanitize_detail(raw, token))
}

/// Descarta las líneas que contienen el token configurado o una referencia a la
/// autorización. Si no queda nada útil, devuelve un marcador accionable.
fn sanitize_detail(raw: &str, token: &str) -> String {
    const REDACTED: &str = "[detalle redactado por seguridad]";
    let token_lower = token.to_ascii_lowercase();
    let mut kept = Vec::new();
    for line in raw.lines() {
        let lower = line.to_ascii_lowercase();
        let echoes_token = !token_lower.is_empty() && lower.contains(&token_lower);
        let echoes_auth = lower.contains("authorization") || lower.contains("bearer");
        if !echoes_token && !echoes_auth {
            kept.push(line);
        }
    }
    let sanitized = kept.join("\n").trim().to_string();
    if sanitized.is_empty() {
        REDACTED.to_string()
    } else if token_lower.is_empty() {
        sanitized
    } else {
        sanitized.replace(token, "[redactado]").trim().to_string()
    }
}

/// Traduce un estado HTTP de error a [`MastodonError`]. Nunca incluye el token.
fn error_from_status(status: u16, message: Option<String>) -> MastodonError {
    match status {
        401 | 403 => MastodonError::Unauthorized,
        404 => MastodonError::NotFound,
        422 => MastodonError::InvalidInput(
            message.unwrap_or_else(|| "validación rechazada por Mastodon".to_string()),
        ),
        429 => MastodonError::RateLimited,
        500..=599 => MastodonError::Upstream(format!("Mastodon respondió {status}")),
        other => {
            MastodonError::Upstream(format!("Mastodon respondió un estado inesperado {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = "mastodon_SECRET_TOKEN";

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

    fn client_for(server: &MockServer) -> Mastodon {
        Mastodon::new(MastodonConfig {
            token: TOKEN.to_string(),
            base_url: server.uri(),
        })
    }

    #[test]
    fn html_to_text_keeps_links_entities_and_line_breaks() {
        let html = "<p>Hola <a href=\"https://ejemplo.es\">mundo</a> &amp; amigos<br/>fin</p>";

        let text = html_to_text(html);

        assert_eq!(text, "Hola mundo & amigos\nfin");
    }

    #[test]
    fn html_to_text_decodes_common_entities_and_block_tags() {
        let html = "<div>&quot;hola&quot; &lt;mundo&gt; &#39;aso&#39;&nbsp;fin</div><p>otro</p>";

        let text = html_to_text(html);

        assert!(text.contains("\"hola\" <mundo> 'aso'"), "texto: {text:?}");
        assert!(text.contains("fin"), "texto: {text:?}");
        assert!(text.contains("otro"), "texto: {text:?}");
        assert!(
            !text.contains("<p") && !text.contains("<div"),
            "sin etiquetas: {text:?}"
        );
    }

    #[test]
    fn error_from_status_maps_every_documented_code() {
        assert_eq!(error_from_status(401, None), MastodonError::Unauthorized);
        assert_eq!(error_from_status(403, None), MastodonError::Unauthorized);
        assert_eq!(error_from_status(404, None), MastodonError::NotFound);
        assert_eq!(
            error_from_status(422, Some(".error".to_string())),
            MastodonError::InvalidInput(".error".to_string())
        );
        assert_eq!(error_from_status(429, None), MastodonError::RateLimited);
        assert!(matches!(
            error_from_status(503, None),
            MastodonError::Upstream(_)
        ));
    }

    #[test]
    fn debug_never_exposes_the_token() {
        let config = MastodonConfig {
            token: TOKEN.to_string(),
            base_url: "https://mastodon.example".to_string(),
        };

        let debug = format!("{config:?}");

        assert!(!debug.contains(TOKEN), "Debug filtra el token: {debug}");
        assert!(debug.contains("***"), "Debug debe redactar: {debug}");
    }

    #[tokio::test]
    async fn without_token_returns_not_configured_without_network() {
        let server = MockServer::start().await;
        let pool = setup_pool().await;
        seed(&pool, "mastodon_base_url", &server.uri()).await;

        let error = Mastodon::from_settings(&pool)
            .await
            .expect_err("sin token debe fallar");

        assert_eq!(error, MastodonError::NotConfigured);
        assert!(
            server
                .received_requests()
                .await
                .expect("requests")
                .is_empty(),
            "sin token no se toca la red"
        );
    }

    #[tokio::test]
    async fn without_base_url_asks_for_configuration_without_network() {
        let pool = setup_pool().await;
        seed(&pool, "mastodon_token", TOKEN).await;

        let error = Mastodon::from_settings(&pool)
            .await
            .expect_err("sin URL base debe fallar");

        assert_eq!(error, MastodonError::MissingBaseUrl);
        let detail = error.to_string();
        assert!(
            detail.contains("mastodon_base_url") && detail.contains("Ajustes"),
            "detalle accionable: {detail}"
        );
    }

    #[tokio::test]
    async fn check_connection_reports_success_and_rejected_token() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("GET"))
            .and(path("/api/v1/notifications"))
            .and(query_param("limit", "1"))
            .and(query_param("types[]", "mention"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/notifications"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        assert_eq!(client.check_connection().await, Ok(()));

        let bad = Mastodon::new(MastodonConfig {
            token: "bad-token".to_string(),
            base_url: server.uri(),
        });
        assert_eq!(
            bad.check_connection().await,
            Err(MastodonError::Unauthorized)
        );
    }

    #[tokio::test]
    async fn list_mentions_parses_notifications_and_queries_only_mentions() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("GET"))
            .and(path("/api/v1/notifications"))
            .and(query_param("types[]", "mention"))
            .and(query_param("limit", "2"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "n1",
                    "type": "mention",
                    "account": { "acct": "alice@example.social" },
                    "status": {
                        "id": "s1",
                        "content": "<p>Hola <a href=\"https://ejemplo.es\">mundo</a> &amp; amigos<br/>fin</p>",
                        "visibility": "public",
                        "url": "https://example.social/@alice/1"
                    }
                },
                { "id": "n2", "type": "follow", "account": { "acct": "bob" } }
            ])))
            .mount(&server)
            .await;

        let mentions = client
            .list_mentions(2, None)
            .await
            .expect("listar menciones");

        assert_eq!(mentions.len(), 1, "solo menciones: {mentions:?}");
        assert_eq!(mentions[0].notification_id, "n1");
        assert_eq!(mentions[0].status_id, "s1");
        assert_eq!(mentions[0].author, "alice@example.social");
        assert_eq!(mentions[0].text, "Hola mundo & amigos\nfin");
        assert_eq!(mentions[0].visibility, "public");
        assert_eq!(mentions[0].url, "https://example.social/@alice/1");
    }

    #[tokio::test]
    async fn list_mentions_sends_max_id() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("GET"))
            .and(path("/api/v1/notifications"))
            .and(query_param("max_id", "n41"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let mentions = client
            .list_mentions(20, Some("n41"))
            .await
            .expect("listar menciones");

        assert!(mentions.is_empty());
    }

    /// Un `2xx` sin `url` no es una respuesta usable: debe fallar, no devolver
    /// una cadena vacía que el agente presentaría como enlace.
    #[tokio::test]
    async fn reply_without_url_in_the_body_is_an_upstream_error() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "s9" })),
            )
            .mount(&server)
            .await;

        let error = client
            .reply("s1", "Gracias", "unlisted", None)
            .await
            .expect_err("un 200 sin url debe fallar");

        assert!(
            matches!(&error, MastodonError::Upstream(detail) if detail.contains("respuesta no válida")),
            "error accionable esperado, llegó {error:?}"
        );
    }

    /// Una instancia hostil que hace eco del Bearer en un `422` no debe filtrar
    /// el token ni la cabecera en el detalle del error.
    #[tokio::test]
    async fn reply_sanitizes_token_echoed_by_a_hostile_instance() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                "error": format!("Authorization: Bearer {TOKEN} rechazado por la instancia")
            })))
            .mount(&server)
            .await;

        let error = client
            .reply("s1", "Gracias", "public", None)
            .await
            .expect_err("un 422 debe fallar");

        assert!(
            matches!(error, MastodonError::InvalidInput(_)),
            "era un 422, llegó {error:?}"
        );
        let detail = error.to_string();
        assert!(
            !detail.contains(TOKEN),
            "el detalle filtra el token: {detail}"
        );
        let lower = detail.to_lowercase();
        assert!(
            !lower.contains("authorization") && !lower.contains("bearer"),
            "el detalle filtra la cabecera: {detail}"
        );
        assert!(
            lower.contains("redactado"),
            "el detalle debe explicar el redactado: {detail}"
        );
    }

    #[tokio::test]
    async fn reply_posts_status_with_fresh_idempotency_key() {
        let server = MockServer::start().await;
        let client = client_for(&server);
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "s9", "url": "https://example.social/@alice/9"
            })))
            .expect(2)
            .mount(&server)
            .await;

        let first = client
            .reply("s1", "Gracias", "unlisted", None)
            .await
            .expect("responder");
        let second = client
            .reply("s1", "Gracias", "unlisted", None)
            .await
            .expect("responder");

        assert_eq!(first, "https://example.social/@alice/9");
        assert_eq!(second, first);
        let requests = server.received_requests().await.expect("requests");
        let keys: Vec<String> = requests
            .iter()
            .map(|req| {
                req.headers
                    .get("idempotency-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| !key.is_empty()), "claves: {keys:?}");
        assert_ne!(keys[0], keys[1], "cada petición lleva una clave nueva");
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("cuerpo JSON");
        assert_eq!(body["status"], "Gracias");
        assert_eq!(body["in_reply_to_id"], "s1");
        assert_eq!(body["visibility"], "unlisted");
    }

    #[tokio::test]
    async fn reply_maps_upstream_error_codes() {
        for (status, expected) in [
            (401, MastodonError::Unauthorized),
            (403, MastodonError::Unauthorized),
            (404, MastodonError::NotFound),
            (429, MastodonError::RateLimited),
        ] {
            let server = MockServer::start().await;
            let client = client_for(&server);
            Mock::given(method("POST"))
                .and(path("/api/v1/statuses"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let error = client
                .reply("s1", "texto", "public", None)
                .await
                .expect_err("debe fallar");
            assert_eq!(error, expected, "estado {status}");
            assert!(
                !error.to_string().contains(TOKEN),
                "el error no filtra el token: {error}"
            );
        }
    }

    #[tokio::test]
    async fn network_failure_maps_to_upstream() {
        let client = Mastodon::new(MastodonConfig {
            token: TOKEN.to_string(),
            base_url: "http://127.0.0.1:1".to_string(),
        });

        let error = client
            .check_connection()
            .await
            .expect_err("sin servidor debe fallar");

        assert!(matches!(error, MastodonError::Upstream(_)), "{error:?}");
    }
}
