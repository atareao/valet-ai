//! Herramienta `mastodon`: lee menciones y contesta con aprobación explícita.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::db::repos::notifications::NotificationsRepo;
#[cfg(test)]
use crate::services::mastodon::MastodonConfig;
use crate::services::mastodon::{Mastodon, MastodonError, Mention};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Operaciones admitidas.
const OPERATIONS: [&str; 2] = ["list_mentions", "reply"];

/// Acceso compartido a la configuración y, en tests, a un cliente inyectado.
struct MastodonAccess {
    db: SqlitePool,
    #[cfg(test)]
    client_override: Option<Mastodon>,
    #[cfg(test)]
    base_url_override: Option<String>,
}

impl MastodonAccess {
    fn new(db: SqlitePool) -> Self {
        Self {
            db,
            #[cfg(test)]
            client_override: None,
            #[cfg(test)]
            base_url_override: None,
        }
    }

    #[cfg(test)]
    fn with_client(db: SqlitePool, client: Mastodon) -> Self {
        Self {
            db,
            client_override: Some(client),
            base_url_override: None,
        }
    }

    #[cfg(test)]
    fn with_base_url(db: SqlitePool, base_url: String) -> Self {
        Self {
            db,
            client_override: None,
            base_url_override: Some(base_url),
        }
    }

    async fn client(&self) -> Result<Mastodon, ToolError> {
        #[cfg(test)]
        if let Some(client) = &self.client_override {
            return Ok(client.clone());
        }
        let config = Mastodon::resolve_config(&self.db)
            .await
            .map_err(mastodon_error)?;
        #[cfg(test)]
        if let Some(base_url) = &self.base_url_override {
            return Ok(Mastodon::new(MastodonConfig {
                token: config.token,
                base_url: base_url.clone(),
            }));
        }
        Ok(Mastodon::new(config))
    }
}

/// Traduce un error del servicio a un error de herramienta accionable.
fn mastodon_error(error: MastodonError) -> ToolError {
    match error {
        MastodonError::NotFound => ToolError::NotFound(error.to_string()),
        other => ToolError::ExecutionError(other.to_string()),
    }
}

/// Cadena obligatoria no vacía; un tipo distinto de cadena se rechaza.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    match args.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim()),
        _ => Err(ToolError::InvalidArguments(format!(
            "{key} es obligatorio y no puede estar en blanco"
        ))),
    }
}

/// Cadena opcional; en blanco o ausente equivale a `None`.
fn optional_str(args: &Value, key: &str) -> Result<Option<String>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            Ok((!value.is_empty()).then(|| value.to_string()))
        }
        Some(_) => Err(ToolError::InvalidArguments(format!(
            "{key} debe ser una cadena de texto"
        ))),
    }
}

/// `limit` entre 1 y 40, con 20 por defecto; nunca toca la red.
fn parse_limit(args: &Value) -> Result<u32, ToolError> {
    match args.get("limit") {
        None | Some(Value::Null) => Ok(20),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(value) if (1..=40).contains(&value) => Ok(value as u32),
            _ => Err(ToolError::InvalidArguments(
                "limit debe ser un entero entre 1 y 40".to_string(),
            )),
        },
        Some(_) => Err(ToolError::InvalidArguments(
            "limit debe ser un entero entre 1 y 40".to_string(),
        )),
    }
}

/// Visibilidad admitida, con `unlisted` por defecto; nunca toca la red.
fn parse_visibility(args: &Value) -> Result<&'static str, ToolError> {
    let value = match args.get("visibility") {
        None | Some(Value::Null) => return Ok("unlisted"),
        Some(Value::String(value)) => value.trim(),
        Some(_) => {
            return Err(ToolError::InvalidArguments(
                "visibility debe ser una cadena".to_string(),
            ))
        }
    };
    match value {
        "public" => Ok("public"),
        "unlisted" => Ok("unlisted"),
        "private" => Ok("private"),
        "direct" => Ok("direct"),
        _ => Err(ToolError::InvalidArguments(format!(
            "visibility inválida: {value}; usa public, unlisted, private o direct"
        ))),
    }
}

/// Convierte una mención al objeto JSON devuelto por la herramienta.
fn mention_json(mention: &Mention) -> Value {
    json!({
        "notification_id": mention.notification_id,
        "status_id": mention.status_id,
        "author": mention.author,
        "text": mention.text,
        "visibility": mention.visibility,
        "url": mention.url,
    })
}

/// `true` si el status se registró como `direct` cuando se vio por primera vez.
async fn is_direct(pool: &SqlitePool, status_id: &str) -> Result<bool, ToolError> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM mastodon_seen WHERE status_id = ?1 AND visibility = 'direct' LIMIT 1",
    )
    .bind(status_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

/// Herramienta `mastodon`.
pub struct MastodonTool {
    access: MastodonAccess,
}

impl MastodonTool {
    /// Construye la herramienta sobre la base de datos que resuelve la
    /// configuración en cada ejecución.
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: MastodonAccess::new(db),
        }
    }

    /// `list_mentions` — lee menciones, deduplica avisos y devuelve el texto.
    async fn list_mentions(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let limit = parse_limit(args)?;
        let max_id = optional_str(args, "max_id")?;
        let client = self.access.client().await?;
        let mentions = client
            .list_mentions(limit, max_id.as_deref())
            .await
            .map_err(mastodon_error)?;

        let mut items = Vec::with_capacity(mentions.len());
        let mut created = 0usize;
        for mention in &mentions {
            let inserted = sqlx::query(
                "INSERT OR IGNORE INTO mastodon_seen (notification_id, status_id, visibility)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(&mention.notification_id)
            .bind(&mention.status_id)
            .bind(&mention.visibility)
            .execute(&self.access.db)
            .await?;
            if inserted.rows_affected() == 1 {
                let metadata = json!({
                    "notification_id": mention.notification_id,
                    "status_id": mention.status_id,
                    "url": mention.url,
                })
                .to_string();
                NotificationsRepo::insert(
                    &self.access.db,
                    "mastodon",
                    &mention.author,
                    &mention.text,
                    "info",
                    Some(&metadata),
                )
                .await?;
                created += 1;
            }
            items.push(mention_json(mention));
        }

        Ok(ToolResult {
            success: true,
            data: json!({
                "mentions": items,
                "count": mentions.len(),
                "new_notifications": created,
            }),
            message: Some(format!(
                "{} menciones ({} nuevas en la bandeja)",
                mentions.len(),
                created
            )),
        })
    }

    /// `reply` — publica una respuesta con visibilidad segura.
    async fn reply(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let status_id = required_str(args, "status_id")?.to_string();
        let text = required_str(args, "text")?.to_string();
        let requested = parse_visibility(args)?;
        let spoiler_text = optional_str(args, "spoiler_text")?;

        let visibility = if is_direct(&self.access.db, &status_id).await? {
            "direct"
        } else {
            requested
        };

        let client = self.access.client().await?;
        let url = client
            .reply(&status_id, &text, visibility, spoiler_text.as_deref())
            .await
            .map_err(mastodon_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({
                "status_id": status_id,
                "url": url,
                "visibility": visibility,
            }),
            message: Some(format!("Respuesta publicada como {visibility}")),
        })
    }
}

#[async_trait]
impl Tool for MastodonTool {
    fn name(&self) -> &'static str {
        "mastodon"
    }

    fn description(&self) -> &'static str {
        "Lee menciones de Mastodon (`list_mentions`, sin confirmación) y contesta a un status (`reply`, con aprobación explícita)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": OPERATIONS,
                    "description": "Operación a ejecutar contra Mastodon"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 40,
                    "default": 20,
                    "description": "Número máximo de menciones a leer (list_mentions)"
                },
                "max_id": {
                    "type": "string",
                    "description": "ID de notificación desde el que paginar (list_mentions, opcional)"
                },
                "status_id": {
                    "type": "string",
                    "description": "ID del status al que se contesta (reply, obligatorio)"
                },
                "text": {
                    "type": "string",
                    "description": "Texto de la respuesta (reply, obligatorio)"
                },
                "visibility": {
                    "type": "string",
                    "enum": ["public", "unlisted", "private", "direct"],
                    "default": "unlisted",
                    "description": "Visibilidad de la respuesta (reply); un status direct se fuerza a direct"
                },
                "spoiler_text": {
                    "type": "string",
                    "description": "Advertencia de contenido de la respuesta (reply, opcional)"
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|value| value.as_str()) {
            Some("list_mentions") => Permission::NoConfirm,
            // `reply` y cualquier operación desconocida fallan cerrado.
            _ => Permission::ExplicitApproval,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        match args.get("operation").and_then(|value| value.as_str()) {
            Some("list_mentions") => self.list_mentions(&args).await,
            Some("reply") => self.reply(&args).await,
            Some(other) => Err(ToolError::InvalidArguments(format!(
                "Operación desconocida: {other}"
            ))),
            None => Err(ToolError::InvalidArguments(
                "Falta el argumento obligatorio `operation`".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = "mastodon_SECRET_TOKEN";

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn client_for(server: &MockServer) -> Mastodon {
        Mastodon::new(MastodonConfig {
            token: TOKEN.to_string(),
            base_url: server.uri(),
        })
    }

    fn tool_for(server: &MockServer, pool: SqlitePool) -> MastodonTool {
        MastodonTool {
            access: MastodonAccess::with_client(pool, client_for(server)),
        }
    }

    fn mentions_body() -> Value {
        json!([
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
            {
                "id": "n2",
                "type": "mention",
                "account": { "acct": "bob@example.social" },
                "status": {
                    "id": "s2",
                    "content": "<p>Otra mención</p>",
                    "visibility": "direct",
                    "url": "https://example.social/@bob/2"
                }
            }
        ])
    }

    async fn mock_mentions(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/v1/notifications"))
            .and(query_param("types[]", "mention"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mentions_body()))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn name_and_schema_declare_both_operations() {
        let tool = MastodonTool::new(setup_db().await);

        assert_eq!(tool.name(), "mastodon");
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert_eq!(params["required"], json!(["operation"]));
        assert_eq!(params["properties"]["operation"]["enum"], json!(OPERATIONS));
        assert!(params["properties"].get("status_id").is_some());
        assert!(params["properties"].get("text").is_some());
        assert!(params["properties"].get("visibility").is_some());
        assert!(params["properties"].get("limit").is_some());
        assert!(params["properties"].get("max_id").is_some());
    }

    #[tokio::test]
    async fn permissions_are_distinct_per_operation() {
        let tool = MastodonTool::new(setup_db().await);

        assert_eq!(
            tool.permission(&json!({"operation": "list_mentions"})),
            Permission::NoConfirm
        );
        assert_eq!(
            tool.permission(&json!({"operation": "reply"})),
            Permission::ExplicitApproval
        );
        assert_eq!(
            tool.permission(&json!({"operation": "boost"})),
            Permission::ExplicitApproval
        );
    }

    #[tokio::test]
    async fn unknown_or_missing_operation_fails_closed_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server, setup_db().await);

        for args in [json!({"operation": "boost"}), json!({})] {
            let error = tool
                .execute(args.clone())
                .await
                .expect_err("operación inválida debe fallar");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "args {args}: {error:?}"
            );
        }

        assert!(
            server
                .received_requests()
                .await
                .expect("requests")
                .is_empty(),
            "una operación desconocida no toca la red"
        );
    }

    #[tokio::test]
    async fn invalid_arguments_are_rejected_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server, setup_db().await);

        let cases = [
            json!({"operation": "list_mentions", "limit": 0}),
            json!({"operation": "list_mentions", "limit": 41}),
            json!({"operation": "reply", "status_id": "s1", "text": "   "}),
            json!({"operation": "reply", "status_id": "  ", "text": "hola"}),
            json!({"operation": "reply", "status_id": "s1", "text": "hola", "visibility": "global"}),
        ];
        for args in cases {
            let error = tool
                .execute(args.clone())
                .await
                .expect_err("argumento inválido debe fallar");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "args {args}: {error:?}"
            );
        }

        assert!(
            server
                .received_requests()
                .await
                .expect("requests")
                .is_empty(),
            "la validación no toca la red"
        );
    }

    #[tokio::test]
    async fn list_mentions_creates_deduplicated_notifications() {
        let server = MockServer::start().await;
        mock_mentions(&server).await;
        let pool = setup_db().await;
        let tool = tool_for(&server, pool.clone());

        let first = tool
            .execute(json!({"operation": "list_mentions"}))
            .await
            .expect("primera lectura");
        assert!(first.success);
        let mentions = first.data["mentions"]
            .as_array()
            .expect("array de menciones");
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0]["text"], "Hola mundo & amigos\nfin");
        assert_eq!(mentions[0]["author"], "alice@example.social");

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE origin = 'mastodon' AND level = 'info'",
        )
        .fetch_one(&pool)
        .await
        .expect("contar avisos");
        assert_eq!(count, 2, "dos menciones nuevas, dos avisos");

        let metadata: String = sqlx::query_scalar(
            "SELECT metadata FROM notifications WHERE origin = 'mastodon' ORDER BY author LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("leer metadata");
        assert!(metadata.contains("notification_id"), "metadata: {metadata}");
        assert!(metadata.contains("status_id"), "metadata: {metadata}");
        assert!(
            metadata.contains("https://example.social"),
            "metadata: {metadata}"
        );

        let second = tool
            .execute(json!({"operation": "list_mentions"}))
            .await
            .expect("segunda lectura");
        assert_eq!(second.data["mentions"].as_array().map(Vec::len), Some(2));
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE origin = 'mastodon'")
                .fetch_one(&pool)
                .await
                .expect("contar avisos");
        assert_eq!(count, 2, "la repetición no crea avisos duplicados");
    }

    #[tokio::test]
    async fn reply_forces_direct_visibility_for_direct_statuses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(body_json(json!({
                "status": "Gracias",
                "in_reply_to_id": "s-direct",
                "visibility": "direct"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "s9", "url": "https://example.social/@alice/9"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let pool = setup_db().await;
        sqlx::query(
            "INSERT INTO mastodon_seen (notification_id, status_id, visibility) VALUES (?1, ?2, ?3)",
        )
        .bind("n-direct")
        .bind("s-direct")
        .bind("direct")
        .execute(&pool)
        .await
        .expect("sembrar status direct");
        let tool = tool_for(&server, pool);

        let result = tool
            .execute(json!({
                "operation": "reply",
                "status_id": "s-direct",
                "text": "Gracias",
                "visibility": "public"
            }))
            .await
            .expect("responder");

        assert!(result.success);
        assert_eq!(result.data["url"], json!("https://example.social/@alice/9"));
    }

    #[tokio::test]
    async fn without_token_fails_actionably_without_network() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = MastodonTool {
            access: MastodonAccess::with_base_url(pool, server.uri()),
        };

        let error = tool
            .execute(json!({"operation": "list_mentions"}))
            .await
            .expect_err("sin token debe fallar");

        let detail = error.to_string();
        assert!(
            detail.to_lowercase().contains("ajustes") && detail.to_lowercase().contains("token"),
            "detalle accionable: {detail}"
        );
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
    async fn rate_limited_reply_reports_actionable_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let tool = tool_for(&server, setup_db().await);

        let error = tool
            .execute(json!({
                "operation": "reply", "status_id": "s1", "text": "hola"
            }))
            .await
            .expect_err("429 debe fallar");

        let detail = error.to_string().to_lowercase();
        assert!(detail.contains("limit"), "detalle accionable: {detail}");
    }

    #[tokio::test]
    async fn reply_sends_a_fresh_idempotency_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "s9", "url": "https://example.social/@alice/9"
            })))
            .expect(2)
            .mount(&server)
            .await;
        let tool = tool_for(&server, setup_db().await);
        let args = json!({"operation": "reply", "status_id": "s1", "text": "hola"});

        tool.execute(args.clone()).await.expect("primera respuesta");
        tool.execute(args).await.expect("segunda respuesta");

        let requests = server.received_requests().await.expect("requests");
        let keys: Vec<String> = requests
            .iter()
            .map(|request| {
                request
                    .headers
                    .get("idempotency-key")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| !key.is_empty()));
        assert_ne!(keys[0], keys[1], "cada petición lleva una clave nueva");
    }
}
