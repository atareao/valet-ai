//! Herramienta `github`: crea issues en GitHub contra su API REST.
//!
//! Es una herramienta multioperación (molde [`crate::tools::oximeal`]): despacha
//! por un parámetro `operation` y declara un permiso por operación. La única
//! operación es `create_issue`, una escritura **aditiva** que avisa (`Notify`)
//! pero no exige aprobación explícita: un issue se cierra o se borra en GitHub.
//!
//! La configuración (token, repositorio y labels por defecto) se resuelve con
//! [`crate::services::github::Github::resolve_config`] en **cada** ejecución; el
//! token nunca aparece en un error.
//!
//! Toda la validación de argumentos ocurre **antes** de resolver la
//! configuración y de tocar la red: un argumento inválido nunca viaja a GitHub.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

#[cfg(test)]
use crate::services::github::GithubConfig;
use crate::services::github::{Github, GithubError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Operaciones que la herramienta admite, en el orden del enum del esquema.
const OPERATIONS: [&str; 1] = ["create_issue"];

/// Acceso compartido a la configuración de GitHub: la base de datos (de donde
/// `from_settings` la resuelve en cada llamada) más overrides disponibles solo en
/// tests, para apuntar a un servidor `wiremock` sin tocar el entorno ni los
/// ajustes.
struct GithubAccess {
    db: SqlitePool,
    /// Cliente ya construido: sustituye por completo la resolución de ajustes.
    #[cfg(test)]
    client_override: Option<Github>,
    /// URL base para el cliente que **sí** resuelve los ajustes: permite probar
    /// el camino «sin token no se toca la red» contra un `wiremock` real.
    #[cfg(test)]
    base_url_override: Option<String>,
}

impl GithubAccess {
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
    fn with_client(db: SqlitePool, client: Github) -> Self {
        Self {
            db,
            client_override: Some(client),
            base_url_override: None,
        }
    }

    /// Cliente que resuelve los ajustes de la base de datos pero apunta a una URL
    /// base de prueba. Lo usa el test de «sin token no se toca la red».
    #[cfg(test)]
    fn with_base_url(db: SqlitePool, base_url: String) -> Self {
        Self {
            db,
            client_override: None,
            base_url_override: Some(base_url),
        }
    }

    /// Resuelve la configuración en cada llamada y construye el cliente.
    async fn client(&self) -> Result<Github, ToolError> {
        #[cfg(test)]
        if let Some(client) = &self.client_override {
            return Ok(client.clone());
        }
        let config = Github::resolve_config(&self.db)
            .await
            .map_err(github_error)?;
        #[cfg(test)]
        if let Some(base_url) = &self.base_url_override {
            return Ok(Github::with_base_url(base_url.clone(), config));
        }
        Ok(Github::new(config))
    }
}

/// Traduce un [`GithubError`] a un [`ToolError`] accionable en español.
///
/// Ningún mensaje incluye el token: el detalle crudo ya viene redactado desde el
/// servicio.
fn github_error(error: GithubError) -> ToolError {
    match error {
        GithubError::NotConfigured => ToolError::ExecutionError(
            "GitHub no está configurado: pega un Personal Access Token en \
             Ajustes → Integraciones → GitHub."
                .to_string(),
        ),
        GithubError::InvalidArguments(detail) => ToolError::InvalidArguments(detail),
        GithubError::Unauthorized => ToolError::ExecutionError(
            "GitHub rechazó el token: revisa que el Personal Access Token sea \
             válido y tenga permiso sobre el repositorio (Ajustes → \
             Integraciones → GitHub)."
                .to_string(),
        ),
        GithubError::NotFound => ToolError::NotFound(
            "El repositorio no existe o el token no lo ve: revisa el \
             repositorio y los permisos del token."
                .to_string(),
        ),
        GithubError::InvalidInput(detail) => {
            ToolError::ExecutionError(format!("GitHub rechazó los datos del issue: {detail}"))
        }
        GithubError::RateLimited => ToolError::ExecutionError(
            "GitHub ha limitado las peticiones: espera antes de volver a \
             intentarlo, no reintentes en bucle."
                .to_string(),
        ),
        GithubError::Upstream(detail) => {
            ToolError::ExecutionError(format!("GitHub no está disponible: {detail}"))
        }
    }
}

/// Extrae un argumento de cadena obligatorio y no en blanco tras `trim`. Un tipo
/// distinto de cadena (número, booleano, array, objeto) es `InvalidArguments`, no
/// un vacío silencioso. No toca la red.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    match args.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim()),
        Some(value) if !value.is_null() && !value.is_string() => {
            Err(ToolError::InvalidArguments(format!(
                "{key} debe ser una cadena de texto; se recibió {}",
                json_type_name(value)
            )))
        }
        _ => Err(ToolError::InvalidArguments(format!(
            "{key} es obligatorio y no puede estar en blanco"
        ))),
    }
}

/// Extrae un argumento de cadena opcional. Ausente o `null` → `None`; una cadena
/// en blanco tras `trim` → `None`; un tipo distinto de cadena (número, booleano,
/// array, objeto) → `InvalidArguments`, en vez de ignorarlo en silencio. No toca
/// la red.
fn optional_str(args: &Value, key: &str) -> Result<Option<String>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            Ok((!value.is_empty()).then(|| value.to_string()))
        }
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "{key} debe ser una cadena de texto; se recibió {}",
            json_type_name(other)
        ))),
    }
}

/// Nombre legible del tipo JSON, para mensajes accionables.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "un booleano",
        Value::Number(_) => "un número",
        Value::String(_) => "una cadena",
        Value::Array(_) => "un array",
        Value::Object(_) => "un objeto",
    }
}

/// Valida que un repositorio tenga la forma `owner/name` (exactamente una barra,
/// ambos tramos no vacíos y sin espacios internos). No toca la red.
fn validate_repo(repo: &str) -> Result<(), ToolError> {
    if repo.chars().any(char::is_whitespace) {
        return Err(ToolError::InvalidArguments(format!(
            "repo inválido: {repo}; no admite espacios y debe tener la forma owner/name"
        )));
    }
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let extra = parts.next();
    if extra.is_some() || owner.trim().is_empty() || name.trim().is_empty() {
        return Err(ToolError::InvalidArguments(format!(
            "repo inválido: {repo}; debe tener la forma owner/name (una sola barra)"
        )));
    }
    Ok(())
}

/// Parsea el argumento opcional `labels`: un array no vacío de cadenas no
/// vacías. Ausente → `None` (se usarán los del ajuste). No toca la red.
fn parse_labels(args: &Value) -> Result<Option<Vec<String>>, ToolError> {
    let Some(raw) = args.get("labels") else {
        return Ok(None);
    };
    let array = raw
        .as_array()
        .ok_or_else(|| ToolError::InvalidArguments("labels debe ser un array de cadenas".into()))?;
    if array.is_empty() {
        return Err(ToolError::InvalidArguments(
            "labels no puede ser un array vacío; omítelo para usar los del ajuste".to_string(),
        ));
    }

    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let label = entry.as_str().map(str::trim).filter(|v| !v.is_empty());
        match label {
            Some(label) => out.push(label.to_string()),
            None => {
                return Err(ToolError::InvalidArguments(
                    "labels no admite entradas vacías ni no textuales".to_string(),
                ))
            }
        }
    }
    Ok(Some(out))
}

/// Herramienta `github`.
pub struct GithubTool {
    access: GithubAccess,
}

impl GithubTool {
    /// Construye la herramienta sobre la base de datos que resuelve la
    /// configuración en cada llamada.
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: GithubAccess::new(db),
        }
    }

    /// `create_issue` — crea un issue en el repositorio resuelto.
    async fn create_issue(&self, args: Value) -> Result<ToolResult, ToolError> {
        let title = required_str(&args, "title")?.to_string();
        let body = optional_str(&args, "body")?;
        let repo_arg = optional_str(&args, "repo")?;
        if let Some(repo) = &repo_arg {
            validate_repo(repo)?;
        }
        let labels_arg = parse_labels(&args)?;

        let client = self.access.client().await?;

        let repo = repo_arg.unwrap_or_else(|| client.config().default_repo.clone());
        if repo.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "no hay repositorio destino: pasa `repo` (owner/name) o configura \
                 `github_default_repo` en Ajustes → Integraciones → GitHub"
                    .to_string(),
            ));
        }
        validate_repo(&repo)?;

        let labels = labels_arg.unwrap_or_else(|| client.config().default_labels.clone());

        let created = client
            .create_issue(&repo, &title, body.as_deref(), &labels)
            .await
            .map_err(github_error)?;

        let mut data = json!({ "repo": repo, "title": title });
        if let Some(number) = created.number {
            data["number"] = json!(number);
        }
        if let Some(url) = created.url {
            data["url"] = json!(url);
        }

        let message = match created.number {
            Some(number) => format!("Issue #{number} creado en {repo}"),
            None => format!("Issue creado en {repo}"),
        };

        Ok(ToolResult {
            success: true,
            data,
            message: Some(message),
        })
    }
}

#[async_trait]
impl Tool for GithubTool {
    fn name(&self) -> &'static str {
        "github"
    }

    fn description(&self) -> &'static str {
        "Crea issues en GitHub. Multioperación con una sola operación, \
         `create_issue`: publica un issue con `title` (obligatorio) y, si se \
         pasan, `body`, `repo` (owner/name) y `labels`. El repositorio sale de \
         `repo` o del ajuste `github_default_repo`; los labels de `labels` o del \
         ajuste `github_default_labels`."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": OPERATIONS,
                    "description": "Operación a ejecutar contra GitHub"
                },
                "title": {
                    "type": "string",
                    "description": "Título del issue (obligatorio en create_issue, no puede estar vacío)"
                },
                "body": {
                    "type": "string",
                    "description": "Cuerpo del issue (opcional)"
                },
                "repo": {
                    "type": "string",
                    "description": "Repositorio destino en forma owner/name (opcional; si falta se usa github_default_repo)"
                },
                "labels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Labels del issue (opcional; si falta se usan los de github_default_labels)"
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|v| v.as_str()) {
            // Escritura aditiva: avisa, pero no es destructiva.
            Some("create_issue") => Permission::Notify,
            // Operación desconocida o ausente: fail-closed. `execute` la
            // rechazará antes de tocar la red.
            _ => Permission::ExplicitApproval,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("create_issue") => self.create_issue(args).await,
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
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TOKEN: &str = "ghp_SECRET";

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn config_for() -> GithubConfig {
        GithubConfig {
            token: TOKEN.to_string(),
            default_repo: "atareao/valet-ai".to_string(),
            default_labels: vec!["valet".to_string()],
        }
    }

    fn tool_for(server: &MockServer) -> GithubTool {
        GithubTool {
            access: GithubAccess::with_client(
                setup_db_placeholder(),
                Github::with_base_url(server.uri(), config_for()),
            ),
        }
    }

    /// Placeholder sin uso: los tests de red sustituyen la base de datos por un
    /// override de configuración, así que nunca se consulta. Se construye un
    /// pool en memoria vacío para no depender de un archivo.
    fn setup_db_placeholder() -> SqlitePool {
        SqlitePool::connect_lazy("sqlite::memory:").expect("lazy pool")
    }

    // ─── 1.2.1 name y esquema ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_name_and_schema() {
        let tool = GithubTool::new(setup_db().await);

        assert_eq!(tool.name(), "github");

        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert_eq!(params["required"], json!(["operation"]));
        assert_eq!(
            params["properties"]["operation"]["enum"],
            json!(["create_issue"])
        );
        assert!(
            params["properties"].get("title").is_some(),
            "el esquema debe documentar `title`"
        );
        assert!(
            params["properties"].get("body").is_some(),
            "el esquema debe documentar `body`"
        );
        assert!(
            params["properties"].get("repo").is_some(),
            "el esquema debe documentar `repo`"
        );
        assert!(
            params["properties"].get("labels").is_some(),
            "el esquema debe documentar `labels`"
        );
    }

    // ─── 1.2.2 validación sin red ────────────────────────────────────────────

    #[tokio::test]
    async fn test_invalid_arguments_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server);

        let cases = [
            json!({ "operation": "create_issue", "title": "   " }),
            json!({ "operation": "create_issue", "title": "T", "repo": "no-es-un-repo" }),
            json!({ "operation": "create_issue", "title": "T", "repo": "a/b/c" }),
            json!({ "operation": "create_issue", "title": "T", "repo": "a / b" }),
            json!({ "operation": "create_issue", "title": "T", "labels": [] }),
            json!({ "operation": "create_issue", "title": "T", "labels": ["  "] }),
        ];

        for args in cases {
            let err = tool
                .execute(args.clone())
                .await
                .expect_err("un argumento inválido debe rechazarse");
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "args {args}: {err:?}"
            );
        }

        let requests = server.received_requests().await.expect("requests");
        assert!(
            requests.is_empty(),
            "la validación no debe tocar la red: {requests:?}"
        );
    }

    /// Un repo con espacios internos (`"a / b"`) se rechaza con un mensaje
    /// accionable y sin tocar la red.
    #[tokio::test]
    async fn test_repo_with_internal_whitespace_is_rejected_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server);

        let err = tool
            .execute(json!({
                "operation": "create_issue",
                "title": "T",
                "repo": "atareao / valet-ai",
            }))
            .await
            .expect_err("un repo con espacios debe rechazarse");

        match err {
            ToolError::InvalidArguments(message) => assert!(
                message.contains("espacios"),
                "el mensaje debe ser accionable: {message}"
            ),
            other => panic!("se esperaba InvalidArguments, llegó {other:?}"),
        }

        let requests = server.received_requests().await.expect("requests");
        assert!(
            requests.is_empty(),
            "un repo inválido no debe tocar la red: {requests:?}"
        );
    }

    /// Scenario: un `repo` con tipo no textual (número) es `InvalidArguments`, no
    /// un valor ignorado en silencio.
    #[tokio::test]
    async fn test_numeric_repo_is_rejected_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server);

        let err = tool
            .execute(json!({ "operation": "create_issue", "title": "T", "repo": 5 }))
            .await
            .expect_err("un `repo` numérico debe rechazarse");

        match err {
            ToolError::InvalidArguments(message) => {
                assert!(
                    message.contains("repo"),
                    "debe nombrar el argumento: {message}"
                );
                assert!(
                    message.contains("número"),
                    "debe describir el tipo recibido: {message}"
                );
            }
            other => panic!("se esperaba InvalidArguments, llegó {other:?}"),
        }

        let requests = server.received_requests().await.expect("requests");
        assert!(requests.is_empty(), "no debe tocar la red: {requests:?}");
    }

    /// Scenario: un `body` con tipo no textual (número) es `InvalidArguments`, no
    /// un valor ignorado en silencio.
    #[tokio::test]
    async fn test_numeric_body_is_rejected_without_network() {
        let server = MockServer::start().await;
        let tool = tool_for(&server);

        let err = tool
            .execute(json!({ "operation": "create_issue", "title": "T", "body": 5 }))
            .await
            .expect_err("un `body` numérico debe rechazarse");

        match err {
            ToolError::InvalidArguments(message) => {
                assert!(
                    message.contains("body"),
                    "debe nombrar el argumento: {message}"
                );
                assert!(
                    message.contains("número"),
                    "debe describir el tipo recibido: {message}"
                );
            }
            other => panic!("se esperaba InvalidArguments, llegó {other:?}"),
        }

        let requests = server.received_requests().await.expect("requests");
        assert!(requests.is_empty(), "no debe tocar la red: {requests:?}");
    }

    /// Scenario: sin token no se toca la red. El cliente resuelve los ajustes
    /// contra un `wiremock` real, pero sin `github_token` la resolución falla
    /// antes de emitir ninguna petición.
    #[tokio::test]
    async fn test_create_issue_without_token_does_not_touch_the_network() {
        let server = MockServer::start().await;
        // BD migrada pero sin `github_token` (la migración no lo siembra).
        let tool = GithubTool {
            access: GithubAccess::with_base_url(setup_db().await, server.uri()),
        };

        let err = tool
            .execute(json!({ "operation": "create_issue", "title": "T" }))
            .await
            .expect_err("sin token debe fallar");

        match err {
            ToolError::ExecutionError(message) => assert!(
                message.contains("no está configurado"),
                "el mensaje debe decir que falta la configuración: {message}"
            ),
            other => panic!("se esperaba ExecutionError (NotConfigured), llegó {other:?}"),
        }

        let requests = server.received_requests().await.expect("requests");
        assert!(
            requests.is_empty(),
            "sin token no debe emitirse ninguna petición: {requests:?}"
        );
    }

    // ─── 1.2.3 resolución de repo y labels ───────────────────────────────────

    #[tokio::test]
    async fn test_repo_param_wins_over_default() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/otro/repo/issues"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "number": 1,
                "html_url": "https://github.com/otro/repo/issues/1",
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server);
        let result = tool
            .execute(json!({
                "operation": "create_issue",
                "title": "T",
                "repo": "otro/repo",
            }))
            .await
            .expect("el repo del parámetro debe usarse");

        assert_eq!(result.data["repo"], "otro/repo");
    }

    #[tokio::test]
    async fn test_default_repo_used_when_absent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "number": 1 })))
            .mount(&server)
            .await;

        let tool = tool_for(&server);
        let result = tool
            .execute(json!({ "operation": "create_issue", "title": "T" }))
            .await
            .expect("debe caer al repositorio por defecto");

        assert_eq!(result.data["repo"], "atareao/valet-ai");
    }

    #[tokio::test]
    async fn test_labels_param_replaces_default_labels() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .and(body_json(json!({ "title": "T", "labels": ["feature"] })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "number": 1 })))
            .mount(&server)
            .await;

        let tool = tool_for(&server);
        tool.execute(json!({
            "operation": "create_issue",
            "title": "T",
            "labels": ["feature"],
        }))
        .await
        .expect("los labels del parámetro deben sustituir a los del ajuste");
    }

    #[tokio::test]
    async fn test_default_labels_used_when_absent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .and(body_json(json!({ "title": "T", "labels": ["valet"] })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "number": 1 })))
            .mount(&server)
            .await;

        let tool = tool_for(&server);
        tool.execute(json!({ "operation": "create_issue", "title": "T" }))
            .await
            .expect("debe caer a los labels por defecto");
    }

    // ─── 1.2.4 sin repo ni default ───────────────────────────────────────────

    #[tokio::test]
    async fn test_missing_repo_fails_without_network() {
        let server = MockServer::start().await;
        let config = GithubConfig {
            token: TOKEN.to_string(),
            default_repo: String::new(),
            default_labels: Vec::new(),
        };
        let tool = GithubTool {
            access: GithubAccess::with_client(
                setup_db_placeholder(),
                Github::with_base_url(server.uri(), config),
            ),
        };

        let err = tool
            .execute(json!({ "operation": "create_issue", "title": "T" }))
            .await
            .expect_err("sin repositorio debe fallar");

        assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        let requests = server.received_requests().await.expect("requests");
        assert!(requests.is_empty(), "no debe tocar la red");
    }

    // ─── 1.2.5 permiso ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_permission_of_create_issue_is_notify() {
        let tool = GithubTool::new(setup_db().await);

        assert_eq!(
            tool.permission(&json!({ "operation": "create_issue" })),
            Permission::Notify
        );
        assert_eq!(
            tool.permission(&json!({ "operation": "desconocida" })),
            Permission::ExplicitApproval
        );
        assert_eq!(
            tool.permission(&json!({})),
            Permission::ExplicitApproval,
            "sin operación falla cerrado"
        );
    }

    // ─── 1.2.6 traducción de errores ─────────────────────────────────────────

    #[test]
    fn test_error_translation_is_actionable_and_secret_free() {
        let errors = [
            GithubError::NotConfigured,
            GithubError::Unauthorized,
            GithubError::NotFound,
            GithubError::InvalidInput("detalle".to_string()),
            GithubError::RateLimited,
            GithubError::Upstream("caída".to_string()),
        ];

        for error in errors {
            let tool_error = github_error(error);
            let text = tool_error.to_string();
            assert!(
                !text.contains(TOKEN) && !text.contains("SECRET"),
                "un mensaje filtra el token: {text}"
            );
        }
    }

    // ─── integración: 201 → éxito ────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_issue_returns_number_and_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/atareao/valet-ai/issues"))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "number": 42,
                "html_url": "https://github.com/atareao/valet-ai/issues/42",
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server);
        let result = tool
            .execute(json!({ "operation": "create_issue", "title": "T", "body": "B" }))
            .await
            .expect("201 debe ser éxito");

        assert!(result.success);
        assert_eq!(result.data["number"], 42);
        assert_eq!(
            result.data["url"],
            "https://github.com/atareao/valet-ai/issues/42"
        );
    }
}
