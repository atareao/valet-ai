//! Herramientas de correo sobre la API de `apimail` (buzón `INBOX`).
//!
//! Cuatro herramientas sobre el buzón de entrada: listar los no leídos
//! (`email_list_unread`), leer el cuerpo sin marcarlo como leído
//! (`email_get_body`), fijar la bandera `\Seen` (`email_mark_read`) y enviar
//! un correo, opcionalmente respondiendo a uno existente (`email_send`).
//!
//! La configuración (`base_url` y API key) se resuelve con
//! [`Apimail::resolve_config`] en cada ejecución; la API key nunca aparece en
//! un mensaje de error. Ver `src/services/apimail.rs`.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::services::apimail::{Apimail, ApimailConfig, ApimailError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// `limit` por defecto de `email_list_unread` cuando el modelo no lo indica.
const DEFAULT_LIMIT: u64 = 50;

/// Cota inferior aceptada para `limit`.
const MIN_LIMIT: u64 = 1;

/// Cota superior aceptada para `limit`.
const MAX_LIMIT: u64 = 200;

/// Traduce un [`ApimailError`] a un [`ToolError`] accionable.
///
/// El detalle crudo de [`ApimailError::Http`] se descarta a propósito y ningún
/// mensaje incluye la API key.
fn apimail_error(error: ApimailError) -> ToolError {
    match error {
        ApimailError::MissingCredentials => ToolError::ExecutionError(
            "No hay configuración de apimail. Añade la API key en Ajustes → API Keys, \
             o define las variables de entorno APIMAIL_BASE_URL / APIMAIL_API_KEY."
                .to_string(),
        ),
        ApimailError::Unauthorized => {
            ToolError::ExecutionError("La clave de apimail no es válida o ha caducado.".to_string())
        }
        ApimailError::MessageNotFound => {
            ToolError::NotFound("El mensaje indicado no existe en el buzón de entrada.".to_string())
        }
        ApimailError::MailboxNotFound => ToolError::ExecutionError(
            "El buzón de entrada no existe en el servidor de correo.".to_string(),
        ),
        ApimailError::TooLarge => {
            ToolError::ExecutionError("El mensaje es demasiado grande para procesarlo.".to_string())
        }
        ApimailError::NotParsable => {
            ToolError::ExecutionError("El mensaje no se pudo interpretar.".to_string())
        }
        ApimailError::SmtpError => {
            ToolError::ExecutionError("El envío no pudo entregarse al servidor SMTP.".to_string())
        }
        ApimailError::Unavailable => ToolError::ExecutionError(
            "El servidor de correo no está disponible en este momento.".to_string(),
        ),
        ApimailError::Http(_) => ToolError::ExecutionError(
            "No se pudo completar la operación con el servidor de correo.".to_string(),
        ),
    }
}

/// Extrae la dirección del primer remitente de un `envelope`.
///
/// Devuelve `None` si el `envelope` no trae remitente o sin `address`. Se usa
/// para resolver el destinatario de una respuesta.
pub(crate) fn reply_recipient(envelope: &Value) -> Option<String> {
    envelope
        .get("from")
        .and_then(|value| value.as_array())
        .and_then(|from| from.first())
        .and_then(|sender| sender.get("address"))
        .and_then(|address| address.as_str())
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
}

/// Compone el asunto de una respuesta con el prefijo `Re: `.
///
/// Si el asunto original ya empieza por `Re:` (comparación sin distinguir
/// mayúsculas y tras recortar el original), no se duplica el prefijo. Si el
/// original queda vacío tras recortarlo, devuelve una cadena vacía en vez de un
/// `"Re: "` colgando.
pub(crate) fn reply_subject(original: &str) -> String {
    let trimmed = original.trim();
    if trimmed.is_empty() {
        String::new()
    } else if trimmed.to_lowercase().starts_with("re:") {
        trimmed.to_string()
    } else {
        format!("Re: {trimmed}")
    }
}

/// Representación legible de un remitente: `"Nombre <direccion>"` si hay
/// nombre, o solo la dirección si no lo hay.
fn readable_sender(envelope: &Value) -> String {
    let Some(sender) = envelope
        .get("from")
        .and_then(|value| value.as_array())
        .and_then(|from| from.first())
    else {
        return String::new();
    };

    let name = sender
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    let address = sender
        .get("address")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();

    match (name.is_empty(), address.is_empty()) {
        (true, _) => address.to_string(),
        (false, true) => name.to_string(),
        (false, false) => format!("{name} <{address}>"),
    }
}

/// Acceso compartido a la configuración de apimail: base de datos más un
/// override solo disponible en tests (para apuntar a un servidor `wiremock`
/// sin tocar el entorno ni los ajustes).
struct ApimailAccess {
    db: SqlitePool,
    #[cfg(test)]
    config_override: Option<ApimailConfig>,
}

impl ApimailAccess {
    fn new(db: SqlitePool) -> Self {
        Self {
            db,
            #[cfg(test)]
            config_override: None,
        }
    }

    #[cfg(test)]
    fn with_config(db: SqlitePool, config: ApimailConfig) -> Self {
        Self {
            db,
            config_override: Some(config),
        }
    }

    /// Resuelve la configuración de apimail o la traduce a [`ToolError`].
    async fn resolve(&self) -> Result<ApimailConfig, ToolError> {
        #[cfg(test)]
        if let Some(config) = &self.config_override {
            return Ok(config.clone());
        }
        Apimail::resolve_config(&self.db)
            .await
            .map_err(apimail_error)
    }
}

/// `email_list_unread` — lista los correos no leídos del buzón de entrada.
pub struct EmailListUnreadTool {
    access: ApimailAccess,
}

impl EmailListUnreadTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: ApimailAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for EmailListUnreadTool {
    fn name(&self) -> &'static str {
        "email_list_unread"
    }

    fn description(&self) -> &'static str {
        "Lista los correos sin leer del buzón de entrada. Devuelve el total de no leídos y, de \
         cada mensaje, su `uid`, el remitente, el asunto y la fecha, para poder leerlos después \
         con `email_get_body`. El parámetro opcional `limit` acota cuántos mensajes se devuelven \
         (50 por defecto, hasta 200)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "minimum": MIN_LIMIT,
                    "maximum": MAX_LIMIT,
                    "description": "Número máximo de correos a devolver (50 por defecto, entre 1 y 200)"
                }
            },
            "required": []
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let limit = match args.get("limit") {
            Some(raw) => {
                let requested = raw.as_u64().ok_or_else(|| {
                    ToolError::InvalidArguments(format!(
                        "limit inválido: {raw}; debe ser un entero entre {MIN_LIMIT} y {MAX_LIMIT}"
                    ))
                })?;
                if !(MIN_LIMIT..=MAX_LIMIT).contains(&requested) {
                    return Err(ToolError::InvalidArguments(format!(
                        "limit inválido: {requested}; debe ser un entero entre {MIN_LIMIT} y {MAX_LIMIT}"
                    )));
                }
                requested
            }
            None => DEFAULT_LIMIT,
        };

        let config = self.access.resolve().await?;
        let client = Apimail::new(config.base_url);
        let value = client
            .list_unread(&config.api_key, limit as u32)
            .await
            .map_err(apimail_error)?;

        let messages: Vec<Value> = value
            .get("messages")
            .and_then(|value| value.as_array())
            .map(|messages| {
                messages
                    .iter()
                    .map(|message| {
                        let envelope = message.get("envelope").unwrap_or(&Value::Null);
                        json!({
                            "uid": message.get("uid").and_then(|v| v.as_u64()),
                            "from": readable_sender(envelope),
                            "subject": envelope.get("subject").and_then(|v| v.as_str()).unwrap_or(""),
                            "date": envelope.get("date").and_then(|v| v.as_str()).unwrap_or(""),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let total = value
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(messages.len() as u64);

        Ok(ToolResult {
            success: true,
            data: json!({ "total": total, "messages": messages }),
            message: Some(format!("{total} correos sin leer")),
        })
    }
}

/// `email_get_body` — devuelve el cuerpo en texto plano y HTML sin marcar leído.
pub struct EmailGetBodyTool {
    access: ApimailAccess,
}

impl EmailGetBodyTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: ApimailAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for EmailGetBodyTool {
    fn name(&self) -> &'static str {
        "email_get_body"
    }

    fn description(&self) -> &'static str {
        "Devuelve el cuerpo de un correo por su `uid`, tanto en texto plano como en HTML. Leer \
         el cuerpo no marca el mensaje como leído: la bandera `\\Seen` no se modifica. El \
         argumento `uid` es obligatorio."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "uid": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Identificador del mensaje (obligatorio)"
                }
            },
            "required": ["uid"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let uid = positive_uid(&args)?;

        let config = self.access.resolve().await?;
        let client = Apimail::new(config.base_url);
        let value = client
            .get_body(&config.api_key, uid)
            .await
            .map_err(apimail_error)?;

        let text = value
            .get("text")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let html = value
            .get("html")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        Ok(ToolResult {
            success: true,
            data: json!({
                "uid": uid,
                "text": text,
                "html": html,
            }),
            message: None,
        })
    }
}

/// `email_mark_read` — fija la bandera `\Seen` de un mensaje (idempotente).
pub struct EmailMarkReadTool {
    access: ApimailAccess,
}

impl EmailMarkReadTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: ApimailAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for EmailMarkReadTool {
    fn name(&self) -> &'static str {
        "email_mark_read"
    }

    fn description(&self) -> &'static str {
        "Marca un correo como leído fijando la bandera `\\Seen` a partir de su `uid`. La operación \
         es idempotente: marcar un mensaje ya leído no es un error. El argumento `uid` es \
         obligatorio."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "uid": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Identificador del mensaje (obligatorio)"
                }
            },
            "required": ["uid"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let uid = positive_uid(&args)?;

        let config = self.access.resolve().await?;
        let client = Apimail::new(config.base_url);
        let value = client
            .mark_read(&config.api_key, uid)
            .await
            .map_err(apimail_error)?;

        let flags = value
            .get("flags")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));

        Ok(ToolResult {
            success: true,
            data: json!({ "uid": uid, "flags": flags }),
            message: Some(format!("Mensaje {uid} marcado como leído")),
        })
    }
}

/// `email_send` — envía un correo, opcionalmente respondiendo a otro.
pub struct EmailSendTool {
    access: ApimailAccess,
}

impl EmailSendTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: ApimailAccess::new(db),
        }
    }

    #[cfg(test)]
    fn with_config(db: SqlitePool, config: ApimailConfig) -> Self {
        Self {
            access: ApimailAccess::with_config(db, config),
        }
    }
}

#[async_trait]
impl Tool for EmailSendTool {
    fn name(&self) -> &'static str {
        "email_send"
    }

    fn description(&self) -> &'static str {
        "Envía un correo. El cuerpo `text` es obligatorio. Si se indica `reply_to_uid`, la \
         herramienta resuelve por su cuenta el destinatario y el asunto del mensaje original \
         (anteponiendo `Re: ` sin duplicarlo), por lo que `to` y `subject` no son necesarios. Si \
         no se responde, son obligatorios `to` (destinatarios) y `subject`."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Cuerpo del correo en texto plano (obligatorio)"
                },
                "to": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Direcciones de destino; obligatorio salvo cuando se responde con reply_to_uid"
                },
                "subject": {
                    "type": "string",
                    "description": "Asunto del correo; obligatorio salvo cuando se responde con reply_to_uid"
                },
                "reply_to_uid": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "uid del mensaje al que se responde; resuelve destinatario y asunto"
                }
            },
            "required": ["text"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::ExplicitApproval
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if text.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "text es obligatorio y no puede estar vacío".to_string(),
            ));
        }

        let reply_uid = match args.get("reply_to_uid") {
            Some(_) => Some(positive_uid_with(&args, "reply_to_uid")?),
            None => None,
        };

        // Todo lo que no necesita red se valida antes de resolver la
        // configuración, para no salir a la red ante argumentos inválidos.
        let direct = match reply_uid {
            Some(_) => None,
            None => Some((recipients(&args)?, subject_arg(&args)?)),
        };

        // La configuración se resuelve una sola vez para toda la operación, de
        // modo que el `GET` del original y el `POST` van al mismo servidor.
        let config = self.access.resolve().await?;

        let (to, subject) = match (reply_uid, direct) {
            (Some(uid), _) => {
                let client = Apimail::new(config.base_url.clone());
                let summary = client
                    .get_summary(&config.api_key, uid)
                    .await
                    .map_err(apimail_error)?;
                let envelope = summary.get("envelope").unwrap_or(&Value::Null);
                let recipient = reply_recipient(envelope).ok_or_else(|| {
                    ToolError::ExecutionError(
                        "No se pudo determinar el destinatario del mensaje original.".to_string(),
                    )
                })?;
                let original_subject = envelope
                    .get("subject")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                (vec![recipient], reply_subject(original_subject))
            }
            (None, Some((to, subject))) => (to, subject),
            (None, None) => {
                return Err(ToolError::InvalidArguments(
                    "to es obligatorio (o usa reply_to_uid) y no puede estar vacío".to_string(),
                ))
            }
        };

        let client = Apimail::new(config.base_url);
        client
            .send(&config.api_key, &to, &subject, text)
            .await
            .map_err(apimail_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "status": "sent", "to": to, "subject": subject }),
            message: Some("Correo enviado".to_string()),
        })
    }
}

/// Valida el argumento `uid` (obligatorio y mayor que 0) sin tocar la red.
fn positive_uid(args: &Value) -> Result<u32, ToolError> {
    positive_uid_with(args, "uid")
}

/// Valida un argumento entero obligatorio y mayor que 0.
fn positive_uid_with(args: &Value, key: &str) -> Result<u32, ToolError> {
    match args.get(key).and_then(|v| v.as_u64()) {
        Some(value) if value > 0 => u32::try_from(value)
            .map_err(|_| ToolError::InvalidArguments(format!("{key} fuera de rango: {value}"))),
        _ => Err(ToolError::InvalidArguments(format!(
            "{key} es obligatorio y debe ser un entero mayor que 0"
        ))),
    }
}

/// Extrae y valida la lista de destinatarios `to`.
///
/// Descarta las entradas que no sean string y las que queden en blanco tras
/// `trim`. Si no sobrevive ninguna dirección, es un argumento inválido.
fn recipients(args: &Value) -> Result<Vec<String>, ToolError> {
    let to: Vec<String> = args
        .get("to")
        .and_then(|v| v.as_array())
        .map(|to| {
            to.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    if to.is_empty() {
        return Err(ToolError::InvalidArguments(
            "to es obligatorio (o usa reply_to_uid) y no puede estar vacío".to_string(),
        ));
    }
    Ok(to)
}

/// Extrae y valida el `subject` (obligatorio y no vacío tras `trim`).
fn subject_arg(args: &Value) -> Result<String, ToolError> {
    let subject = args
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if subject.is_empty() {
        return Err(ToolError::InvalidArguments(
            "subject es obligatorio (o usa reply_to_uid) y no puede estar vacío".to_string(),
        ));
    }
    Ok(subject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use crate::services::apimail::ApimailConfig;
    use serial_test::serial;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn clear_apimail_env() {
        std::env::remove_var("APIMAIL_BASE_URL");
        std::env::remove_var("APIMAIL_API_KEY");
    }

    #[tokio::test]
    async fn test_names() {
        let db = setup_db().await;
        assert_eq!(
            EmailListUnreadTool::new(db.clone()).name(),
            "email_list_unread"
        );
        assert_eq!(EmailGetBodyTool::new(db.clone()).name(), "email_get_body");
        assert_eq!(EmailMarkReadTool::new(db.clone()).name(), "email_mark_read");
        assert_eq!(EmailSendTool::new(db).name(), "email_send");
    }

    #[tokio::test]
    async fn test_descriptions_are_spanish_and_mention_required() {
        let db = setup_db().await;
        let list = EmailListUnreadTool::new(db.clone());
        let body = EmailGetBodyTool::new(db.clone());
        let mark = EmailMarkReadTool::new(db.clone());
        let send = EmailSendTool::new(db);

        assert!(
            list.description().contains("sin leer"),
            "{}",
            list.description()
        );
        assert!(
            list.description().contains("limit"),
            "{}",
            list.description()
        );

        assert!(
            body.description().contains("cuerpo"),
            "{}",
            body.description()
        );
        assert!(
            body.description().contains("no") && body.description().contains("leído"),
            "{}",
            body.description()
        );

        assert!(
            mark.description().contains("Seen"),
            "{}",
            mark.description()
        );
        assert!(
            mark.description().contains("idempotente"),
            "{}",
            mark.description()
        );

        assert!(
            send.description().contains("obligatorio"),
            "{}",
            send.description()
        );
        assert!(
            send.description().contains("reply_to_uid"),
            "{}",
            send.description()
        );
    }

    #[tokio::test]
    async fn test_parameters_uid_and_required() {
        let db = setup_db().await;

        for params in [
            EmailGetBodyTool::new(db.clone()).parameters(),
            EmailMarkReadTool::new(db.clone()).parameters(),
        ] {
            let props = params["properties"].as_object().unwrap();
            assert!(props.contains_key("uid"));
            assert_eq!(params["properties"]["uid"]["type"], "integer");
            let required: Vec<&str> = params["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(required, vec!["uid"]);
        }

        let send = EmailSendTool::new(db.clone()).parameters();
        assert_eq!(send["properties"]["reply_to_uid"]["type"], "integer");
        let required: Vec<&str> = send["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, vec!["text"]);

        let list = EmailListUnreadTool::new(db).parameters();
        assert_eq!(list["properties"]["limit"]["type"], "integer");
        assert_eq!(list["properties"]["limit"]["minimum"], 1);
        assert_eq!(list["properties"]["limit"]["maximum"], 200);
        assert!(list["required"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_permissions() {
        let db = setup_db().await;
        assert_eq!(
            EmailListUnreadTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            EmailGetBodyTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            EmailMarkReadTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            EmailSendTool::new(db).permission(&json!({})),
            Permission::ExplicitApproval
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_invalid_args_without_network() {
        clear_apimail_env();
        let db = setup_db().await;

        for args in [json!({}), json!({ "uid": 0 })] {
            let err = EmailGetBodyTool::new(db.clone())
                .execute(args)
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        for args in [json!({}), json!({ "uid": 0 })] {
            let err = EmailMarkReadTool::new(db.clone())
                .execute(args)
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        for args in [json!({ "limit": 0 }), json!({ "limit": 500 })] {
            let err = EmailListUnreadTool::new(db.clone())
                .execute(args)
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        let err = EmailSendTool::new(db.clone())
            .execute(json!({ "to": ["a@b.c"], "subject": "S" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");

        let err = EmailSendTool::new(db.clone())
            .execute(json!({ "text": "hola" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");

        for to in [json!([""]), json!(["   "])] {
            let err = EmailSendTool::new(db.clone())
                .execute(json!({ "text": "hola", "to": to, "subject": "S" }))
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_missing_api_key_returns_error_without_network() {
        clear_apimail_env();
        let db = setup_db().await;

        let results = vec![
            EmailListUnreadTool::new(db.clone())
                .execute(json!({}))
                .await,
            EmailGetBodyTool::new(db.clone())
                .execute(json!({ "uid": 1 }))
                .await,
            EmailMarkReadTool::new(db.clone())
                .execute(json!({ "uid": 1 }))
                .await,
            EmailSendTool::new(db.clone())
                .execute(json!({ "text": "hola", "to": ["a@b.c"], "subject": "S" }))
                .await,
        ];

        for result in results {
            assert!(
                matches!(result, Err(ToolError::ExecutionError(_))),
                "expected ExecutionError, got {result:?}"
            );
        }
    }

    #[test]
    fn test_reply_subject_does_not_duplicate() {
        assert_eq!(reply_subject("Presupuesto"), "Re: Presupuesto");
        assert_eq!(reply_subject("Re: Presupuesto"), "Re: Presupuesto");
        assert_eq!(reply_subject("RE: Presupuesto"), "RE: Presupuesto");
        assert_eq!(reply_subject("  Re: Presupuesto  "), "Re: Presupuesto");
        assert_eq!(reply_subject("re: hola"), "re: hola");
        assert_eq!(reply_subject(""), "");
        assert_eq!(reply_subject("   "), "");
    }

    #[test]
    fn test_reply_recipient_from_envelope() {
        let envelope = json!({
            "from": [{ "name": "Juan", "address": "juan@example.com" }]
        });
        assert_eq!(
            reply_recipient(&envelope).as_deref(),
            Some("juan@example.com")
        );
        assert_eq!(reply_recipient(&json!({ "from": [] })), None);
    }

    #[tokio::test]
    async fn test_send_via_reply_uid_resolves_recipient_and_subject() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/messages/7"))
            .and(query_param("mailbox", "INBOX"))
            .and(query_param("format", "summary"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "uid": 7,
                "seq": 7,
                "flags": [],
                "size": 1,
                "internal_date": "x",
                "envelope": {
                    "from": [{ "name": "Juan", "address": "juan@example.com" }],
                    "to": [], "cc": [], "subject": "Presupuesto",
                    "date": "x", "message_id": "m"
                },
                "format": "summary"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/messages"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({
                "to": ["juan@example.com"],
                "subject": "Re: Presupuesto",
                "text": "Gracias"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "sent" })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailSendTool::with_config(
            db,
            ApimailConfig {
                base_url: server.uri(),
                api_key: "test-key".to_string(),
            },
        );

        let result = tool
            .execute(json!({ "text": "Gracias", "reply_to_uid": 7 }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["status"], "sent");
        assert_eq!(result.data["subject"], "Re: Presupuesto");
        assert_eq!(result.data["to"][0], "juan@example.com");

        let requests = server.received_requests().await.unwrap();
        let post = requests
            .iter()
            .find(|r| r.method.as_str() == "POST")
            .expect("a POST request must have been sent");
        let body: Value = serde_json::from_slice(&post.body).unwrap();
        assert_eq!(body["to"][0], "juan@example.com");
        assert_eq!(body["subject"], "Re: Presupuesto");
    }

    /// Config apuntando al servidor `wiremock` con la clave de test.
    fn config_for(server: &MockServer) -> ApimailConfig {
        ApimailConfig {
            base_url: server.uri(),
            api_key: "test-key".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // email_list_unread (extremo a extremo con la tool)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_unread_flattens_sender_subject_date_and_uid() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages"))
            .and(query_param("mailbox", "INBOX"))
            .and(query_param("unseen", "true"))
            .and(query_param("limit", "50"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "total": 2,
                "limit": 50,
                "offset": 0,
                "messages": [
                    {
                        "uid": 10,
                        "envelope": {
                            "from": [{ "name": "Ana", "address": "ana@example.com" }],
                            "subject": "Hola",
                            "date": "2026-01-02T03:04:05Z"
                        }
                    },
                    {
                        "uid": 11,
                        "envelope": {
                            "from": [{ "address": "bob@example.com" }],
                            "subject": "Sin nombre",
                            "date": "2026-01-03"
                        }
                    }
                ]
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailListUnreadTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({})).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["total"], 2);

        let messages = result.data["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);

        assert_eq!(messages[0]["uid"], 10);
        assert_eq!(messages[0]["from"], "Ana <ana@example.com>");
        assert_eq!(messages[0]["subject"], "Hola");
        assert_eq!(messages[0]["date"], "2026-01-02T03:04:05Z");

        assert_eq!(messages[1]["uid"], 11);
        assert_eq!(messages[1]["from"], "bob@example.com");
        assert_eq!(messages[1]["subject"], "Sin nombre");
        assert_eq!(messages[1]["date"], "2026-01-03");
    }

    // -----------------------------------------------------------------------
    // email_get_body (extremo a extremo con la tool)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_body_returns_text_and_html() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/5/body"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "uid": 5,
                "text": "cuerpo plano",
                "html": "<p>cuerpo plano</p>"
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailGetBodyTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "uid": 5 })).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["uid"], 5);
        assert_eq!(result.data["text"], "cuerpo plano");
        assert_eq!(result.data["html"], "<p>cuerpo plano</p>");
    }

    #[tokio::test]
    async fn test_get_body_null_bodies_stay_null() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/6/body"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "uid": 6,
                "text": null,
                "html": null
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailGetBodyTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "uid": 6 })).await.unwrap();

        assert!(
            result.data["text"].is_null(),
            "text: {}",
            result.data["text"]
        );
        assert!(
            result.data["html"].is_null(),
            "html: {}",
            result.data["html"]
        );
    }

    #[tokio::test]
    async fn test_get_body_missing_bodies_stay_null() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/8/body"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "uid": 8
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailGetBodyTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "uid": 8 })).await.unwrap();

        assert!(
            result.data["text"].is_null(),
            "text: {}",
            result.data["text"]
        );
        assert!(
            result.data["html"].is_null(),
            "html: {}",
            result.data["html"]
        );
    }

    // -----------------------------------------------------------------------
    // email_mark_read (extremo a extremo con la tool)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_mark_read_passes_through_flags() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/messages/9/flags"))
            .and(query_param("mailbox", "INBOX"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({ "add": ["\\Seen"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "mailbox": "INBOX",
                "uid": 9,
                "flags": ["\\Seen", "\\Recent"]
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailMarkReadTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "uid": 9 })).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["uid"], 9);
        assert_eq!(result.data["flags"], json!(["\\Seen", "\\Recent"]));
    }

    // -----------------------------------------------------------------------
    // email_send sin reply_to_uid (extremo a extremo con la tool)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_direct_posts_to_subject_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/messages"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({
                "to": ["ana@example.com"],
                "subject": "Hola",
                "text": "Cuerpo"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "sent" })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailSendTool::with_config(db, config_for(&server));

        let result = tool
            .execute(json!({
                "to": ["ana@example.com"],
                "subject": "  Hola  ",
                "text": "Cuerpo"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["status"], "sent");
        assert_eq!(result.data["subject"], "Hola");
        assert_eq!(result.data["to"][0], "ana@example.com");

        let requests = server.received_requests().await.unwrap();
        let post = requests
            .iter()
            .find(|r| r.method.as_str() == "POST")
            .expect("a POST request must have been sent");
        let body: Value = serde_json::from_slice(&post.body).unwrap();
        assert_eq!(body["to"], json!(["ana@example.com"]));
        assert_eq!(body["subject"], "Hola");
        assert_eq!(body["text"], "Cuerpo");
    }

    // -----------------------------------------------------------------------
    // Traducción ApimailError -> ToolError a nivel de tool
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_error_401_maps_to_invalid_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": "unauthorized",
                "message": "nope"
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailListUnreadTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        match tool.execute(json!({})).await {
            Err(ToolError::ExecutionError(message)) => {
                let lower = message.to_lowercase();
                assert!(lower.contains("clave"), "{message}");
                assert!(lower.contains("válida"), "{message}");
            }
            other => panic!("expected ExecutionError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_error_404_message_not_found_maps_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/messages/77/body"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": "message_not_found",
                "message": "gone"
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailGetBodyTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        let err = tool.execute(json!({ "uid": 77 })).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)), "{err:?}");
    }

    #[tokio::test]
    async fn test_error_502_smtp_maps_to_delivery_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/messages"))
            .respond_with(ResponseTemplate::new(502).set_body_json(json!({
                "error": "smtp_error",
                "message": "smtp down"
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailSendTool::with_config(db, config_for(&server));

        match tool
            .execute(json!({
                "to": ["ana@example.com"],
                "subject": "S",
                "text": "hola"
            }))
            .await
        {
            Err(ToolError::ExecutionError(message)) => {
                assert!(message.to_lowercase().contains("entregar"), "{message}");
            }
            other => panic!("expected ExecutionError, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_error_503_maps_to_server_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/messages/9/flags"))
            .respond_with(ResponseTemplate::new(503).set_body_json(json!({
                "error": "imap_unavailable",
                "message": "imap down"
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = EmailMarkReadTool {
            access: ApimailAccess::with_config(db, config_for(&server)),
        };

        match tool.execute(json!({ "uid": 9 })).await {
            Err(ToolError::ExecutionError(message)) => {
                assert!(message.to_lowercase().contains("disponible"), "{message}");
            }
            other => panic!("expected ExecutionError, got {other:?}"),
        }
    }
}
