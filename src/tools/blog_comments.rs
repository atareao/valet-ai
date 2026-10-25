//! Herramientas del agente para el blog (`blog_comment_reply` y
//! `blog_comment_delete`).
//!
//! Publican una respuesta y mandan un comentario a la **papelera** por la API
//! REST del núcleo de WordPress. Los identificadores del comentario y de la
//! entrada **siempre** se resuelven del `metadata` de la notificación que anunció
//! el comentario —también en el borrado, que **no** admite identificadores
//! explícitos—, salvo `blog_comment_reply`, que además acepta `comment_id` y
//! `post_id`; las herramientas **nunca** los inventan. Toda la validación ocurre
//! **antes** de tocar la red. El borrado **nunca** es definitivo y no tiene
//! estado propio: la idempotencia viene del `410 rest_already_trashed` del sitio.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::db::repos::comment_replies::{CommentRepliesRepo, Reservation};
use crate::db::repos::notifications::NotificationsRepo;
use crate::services::wordpress::{TrashOutcome, Wordpress, WordpressConfig, WordpressError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Longitud máxima de una respuesta, en caracteres, comprobada **sin** salir a
/// la red.
const MAX_CONTENT_CHARS: usize = 4000;

/// Los dos identificadores de una respuesta, normalizados a su forma decimal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentIds {
    /// El id del comentario al que se responde, en forma decimal.
    pub comment_id: String,
    /// El id de la entrada del blog, en forma decimal.
    pub post_id: String,
}

/// El motivo por el que un `metadata` no permite resolver los identificadores.
///
/// Ninguna variante arrastra un valor del `metadata`: el mensaje nombra el campo
/// o el problema, pero **nunca** un identificador inventado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// La notificación no trae `metadata` (ausente o `NULL`).
    Missing,
    /// El `metadata` no parsea como JSON.
    InvalidJson,
    /// El `metadata` no es un objeto JSON (es un arreglo o un escalar).
    NotAnObject,
    /// Al objeto le falta un campo obligatorio.
    MissingField(&'static str),
    /// Un campo tiene una forma que no es un identificador válido.
    InvalidField { field: &'static str },
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetadataError::Missing => write!(
                f,
                "la notificación no trae `metadata`: no hay de dónde tomar los \
                 identificadores del comentario"
            ),
            MetadataError::InvalidJson => {
                write!(f, "el `metadata` de la notificación no es JSON válido")
            }
            MetadataError::NotAnObject => {
                write!(f, "el `metadata` de la notificación no es un objeto JSON")
            }
            MetadataError::MissingField(field) => write!(
                f,
                "el `metadata` de la notificación no trae el campo `{field}`"
            ),
            MetadataError::InvalidField { field } => write!(
                f,
                "el campo `{field}` del `metadata` no es un identificador válido \
                 (se espera un entero positivo)"
            ),
        }
    }
}

impl std::error::Error for MetadataError {}

/// Resuelve `comment_id` y `post_id` del `metadata` de una notificación.
///
/// El `metadata` llega como **cadena JSON** (o `None` si la columna es `NULL`).
/// Debe ser un objeto con las claves `comment_id` y `post_id`, cada una un
/// entero **positivo** o una cadena de dígitos; en ambos casos el valor se
/// **normaliza a su forma decimal**. Cualquier otra forma —`0`, negativo,
/// decimal, `null`, booleano, vacío, arreglo u objeto— se rechaza nombrando el
/// campo. Es una función **pura**: no toca la red ni la base de datos, y
/// **nunca** inventa un identificador.
pub fn comment_ids_from_metadata(metadata: Option<&str>) -> Result<CommentIds, MetadataError> {
    let raw = metadata.ok_or(MetadataError::Missing)?;
    let value: Value = serde_json::from_str(raw).map_err(|_| MetadataError::InvalidJson)?;
    let object = value.as_object().ok_or(MetadataError::NotAnObject)?;

    let comment_id = field_id(object, "comment_id")?;
    let post_id = field_id(object, "post_id")?;

    Ok(CommentIds {
        comment_id,
        post_id,
    })
}

/// Extrae y normaliza un campo obligatorio del objeto del `metadata`.
fn field_id(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, MetadataError> {
    let value = object
        .get(field)
        .ok_or(MetadataError::MissingField(field))?;
    normalize_id(value, field)
}

/// Normaliza un valor de identificador: entero positivo o cadena de dígitos a su
/// forma decimal; cualquier otra forma es un error.
fn normalize_id(value: &Value, field: &'static str) -> Result<String, MetadataError> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .filter(|digits| *digits > 0)
            .map(|digits| digits.to_string())
            .ok_or(MetadataError::InvalidField { field }),
        Value::String(text) => normalize_digits(text, field),
        _ => Err(MetadataError::InvalidField { field }),
    }
}

/// Normaliza una cadena de dígitos a su forma decimal. Rechaza la cadena vacía,
/// los caracteres que no son dígitos y el valor cero.
fn normalize_digits(text: &str, field: &'static str) -> Result<String, MetadataError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MetadataError::InvalidField { field });
    }
    let normalized = text.trim_start_matches('0');
    if normalized.is_empty() {
        // Solo había ceros: el valor es 0, que no es un identificador válido.
        return Err(MetadataError::InvalidField { field });
    }
    Ok(normalized.to_string())
}

/// Normaliza un identificador **explícito** a su forma decimal, con la misma
/// pieza de validación que la vía del `metadata` (`normalize_digits`), para que
/// ninguna ruta de identificadores se libre: un valor que no sea una cadena de
/// dígitos no nula —o vacío, con signo, decimal, con parámetros— es
/// [`ToolError::InvalidArguments`] y **no** llega a formatear ninguna URL.
fn normalize_explicit_id(raw: &str, field: &'static str) -> Result<String, ToolError> {
    normalize_digits(raw, field).map_err(|error| ToolError::InvalidArguments(error.to_string()))
}

/// Valida y **recorta** el argumento `content` sin salir a la red.
///
/// Rechaza el contenido ausente, vacío o en blanco y el que supere
/// [`MAX_CONTENT_CHARS`] caracteres; en caso contrario devuelve el texto ya
/// recortado (`trim`), que es lo que se envía a WordPress.
fn validate_content(args: &Value) -> Result<String, ToolError> {
    let raw = args.get("content").and_then(Value::as_str).unwrap_or("");
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ToolError::InvalidArguments(
            "`content` es obligatorio y no puede estar vacío ni en blanco".to_string(),
        ));
    }
    if trimmed.chars().count() > MAX_CONTENT_CHARS {
        return Err(ToolError::InvalidArguments(format!(
            "`content` no puede superar los {MAX_CONTENT_CHARS} caracteres"
        )));
    }
    Ok(trimmed.to_string())
}

/// Herramienta `blog_comment_reply`: publica una respuesta a un comentario del
/// blog por la API REST del núcleo de WordPress.
///
/// La secuencia de una invocación es **validar → resolver → publicar →
/// registrar**, con **toda** la validación (contenido y procedencia de los
/// identificadores) **antes** de tocar la red. La respuesta se publica ya
/// **aprobada** y la credencial es una Application Password que nunca aparece en
/// un registro; publicar un comentario es **irreversible**, por lo que la
/// herramienta exige aprobación explícita.
pub struct BlogCommentReplyTool {
    db: SqlitePool,
    /// Configuración inyectada (solo en tests); en producción se resuelve del
    /// entorno en cada llamada.
    config: Option<WordpressConfig>,
}

impl BlogCommentReplyTool {
    /// Herramienta de producción: la configuración se resuelve del entorno en
    /// cada llamada.
    pub fn new(db: SqlitePool) -> Self {
        Self { db, config: None }
    }

    /// Constructor de test: inyecta la configuración (base y credenciales) para
    /// apuntar la publicación a un servidor de prueba.
    pub fn with_config(db: SqlitePool, config: WordpressConfig) -> Self {
        Self {
            db,
            config: Some(config),
        }
    }

    /// La configuración a usar: la inyectada o la resuelta del entorno.
    fn resolve_config(&self) -> Result<WordpressConfig, ToolError> {
        match &self.config {
            Some(config) => Ok(config.clone()),
            None => Wordpress::resolve_config()
                .map_err(|error| ToolError::ExecutionError(error.to_string())),
        }
    }
}

/// Resuelve los dos identificadores **antes** de tocar la red. La comparten las
/// dos herramientas del blog.
///
/// Si se indica `notification_id`, se lee esa notificación de la bandeja y los
/// ids salen de su `metadata`; indicarlo junto con `comment_id` o `post_id` es un
/// error de argumentos, y si la notificación no existe, un `NotFound`. Sin
/// `notification_id` se exigen los dos identificadores explícitos, y su ausencia
/// es un `InvalidArguments` que remite a la herramienta `notifications`. Los
/// identificadores explícitos pasan por la **misma** normalización que los del
/// `metadata` (`normalize_digits`) **antes** de formatear cualquier URL: una
/// forma que no sea una cadena de dígitos es un `InvalidArguments` y no sale a
/// la red.
async fn resolve_comment_ids(db: &SqlitePool, args: &Value) -> Result<CommentIds, ToolError> {
    let notification_id = args.get("notification_id").and_then(Value::as_str);
    let comment_id = args.get("comment_id").and_then(Value::as_str);
    let post_id = args.get("post_id").and_then(Value::as_str);

    if let Some(notification_id) = notification_id {
        if comment_id.is_some() || post_id.is_some() {
            return Err(ToolError::InvalidArguments(
                "`notification_id` es mutuamente excluyente con `comment_id` y \
                 `post_id`: indica la notificación o los identificadores, pero \
                 no ambos"
                    .into(),
            ));
        }

        return ids_from_notification(db, notification_id).await;
    }

    match (comment_id, post_id) {
        (Some(comment_id), Some(post_id)) => Ok(CommentIds {
            comment_id: normalize_explicit_id(comment_id, "comment_id")?,
            post_id: normalize_explicit_id(post_id, "post_id")?,
        }),
        _ => Err(ToolError::InvalidArguments(
            "faltan los identificadores del comentario: indica `comment_id` y \
             `post_id`, o `notification_id` para tomarlos del `metadata` del \
             aviso que devuelve la herramienta `notifications`"
                .into(),
        )),
    }
}

/// La resolución de identificadores del **borrado**: **únicamente** del
/// `metadata` del aviso apuntado por `notification_id`.
///
/// Un `comment_id` o un `post_id` aportado por quien llama se rechaza con
/// `InvalidArguments` —la vía explícita **no** existe en el borrado—, de modo que
/// quien llama no puede elegir qué comentario se manda a la papelera: los
/// identificadores salen **siempre** del aviso, y sin `notification_id` no hay
/// ninguna vía. La respuesta (`blog_comment_reply`) conserva su vía explícita;
/// esta asimetría es deliberada.
async fn resolve_delete_ids(db: &SqlitePool, args: &Value) -> Result<CommentIds, ToolError> {
    let has_explicit = ["comment_id", "post_id"]
        .iter()
        .any(|field| args.get(*field).is_some_and(|value| !value.is_null()));
    if has_explicit {
        return Err(ToolError::InvalidArguments(
            "`comment_id` y `post_id` no se aceptan en el borrado: los \
             identificadores salen siempre del `metadata` del aviso, indicado \
             con `notification_id`"
                .into(),
        ));
    }

    let notification_id = args
        .get("notification_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::InvalidArguments(
                "falta `notification_id`: los identificadores del borrado salen \
                 del `metadata` del aviso que devuelve la herramienta \
                 `notifications`"
                    .into(),
            )
        })?;

    ids_from_notification(db, notification_id).await
}

/// Lee la notificación `notification_id` de la bandeja y resuelve sus dos
/// identificadores del `metadata`. La compartida por ambas herramientas.
async fn ids_from_notification(
    db: &SqlitePool,
    notification_id: &str,
) -> Result<CommentIds, ToolError> {
    let notification = NotificationsRepo::get_by_id(db, notification_id)
        .await?
        .ok_or_else(|| {
            ToolError::NotFound(format!(
                "la notificación `{notification_id}` no existe en la bandeja"
            ))
        })?;

    comment_ids_from_metadata(notification.metadata.as_deref())
        .map_err(|error| ToolError::InvalidArguments(error.to_string()))
}

#[async_trait]
impl Tool for BlogCommentReplyTool {
    fn name(&self) -> &'static str {
        "blog_comment_reply"
    }

    fn description(&self) -> &'static str {
        "Blog — responde a un comentario del sitio publicando una respuesta ya aprobada; `content` es obligatorio y los identificadores del comentario (`comment_id`) y de la entrada (`post_id`) salen del aviso o se indican; publicar no se puede deshacer"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "La respuesta a publicar. Obligatorio. Publicar no se puede deshacer."
                },
                "comment_id": {
                    "type": "string",
                    "description": "Id del comentario al que se responde. Obligatorio salvo que se indique `notification_id`."
                },
                "post_id": {
                    "type": "string",
                    "description": "Id de la entrada del comentario. Obligatorio salvo que se indique `notification_id`."
                },
                "notification_id": {
                    "type": "string",
                    "description": "Id de la notificación que anunció el comentario; de su `metadata` se toman los identificadores. Mutuamente excluyente con `comment_id` y `post_id`."
                }
            },
            "required": ["content"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::ExplicitApproval
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let started = std::time::Instant::now();

        // 1. Validación (sin red): contenido y procedencia de los ids.
        let content = validate_content(&args)?;
        let ids = resolve_comment_ids(&self.db, &args).await?;
        let notification_id = args
            .get("notification_id")
            .and_then(Value::as_str)
            .map(str::to_string);

        // 2. Configuración: se resuelve en cada llamada; sin credenciales no se
        //    reserva ni se sale a la red.
        let config = self.resolve_config()?;

        // 3. Reserva atómica **antes** de la red. Si ya hay una fila en
        //    `reserved`, `published` o `uncertain`, no se publica y se informa.
        let reservation = CommentRepliesRepo::reserve(
            &self.db,
            &ids.comment_id,
            &ids.post_id,
            notification_id.as_deref(),
        )
        .await?;
        if reservation == Reservation::AlreadyReserved {
            return self.duplicate_result(&ids).await;
        }

        // 4. Publicación: una sola petición, sin reintentos. El cliente usa el
        //    timeout de la configuración (15 s en producción, inyectable en
        //    tests).
        let client = Wordpress::with_timeout(config.base_url.clone(), config.timeout);
        let outcome = client
            .publish_comment_reply(&config, &ids.post_id, &ids.comment_id, &content)
            .await;
        let latency_ms = started.elapsed().as_millis() as u64;

        match outcome {
            Ok(reply) => {
                CommentRepliesRepo::mark_published(
                    &self.db,
                    &ids.comment_id,
                    reply.wp_comment_id.as_deref(),
                    201,
                )
                .await?;
                tracing::info!(
                    comment_id = %ids.comment_id,
                    post_id = %ids.post_id,
                    notification_id = notification_id.as_deref().unwrap_or(""),
                    wp_comment_id = reply.wp_comment_id.as_deref().unwrap_or(""),
                    status = "approved",
                    latency_ms,
                    "blog_comment_reply: respuesta publicada y aprobada"
                );
                Ok(ToolResult {
                    success: true,
                    data: json!({ "wp_comment_id": reply.wp_comment_id, "status": "approved" }),
                    message: Some(format!(
                        "respuesta al comentario {} publicada",
                        ids.comment_id
                    )),
                })
            }
            Err(error) => {
                // Solo se suelta la reserva cuando el desenlace **garantiza** que
                // no se creó nada; un desenlace ambiguo la conserva como
                // `uncertain` (publicar dos veces es el fallo más caro).
                if releases_reservation(&error) {
                    CommentRepliesRepo::release(&self.db, &ids.comment_id).await?;
                } else {
                    let last_status = match &error {
                        WordpressError::Unexpected { status, .. } => Some(i64::from(*status)),
                        _ => None,
                    };
                    CommentRepliesRepo::mark_uncertain(&self.db, &ids.comment_id, last_status)
                        .await?;
                }
                tracing::warn!(
                    comment_id = %ids.comment_id,
                    reason = %error,
                    "blog_comment_reply: no se pudo publicar la respuesta"
                );
                Err(ToolError::ExecutionError(error.to_string()))
            }
        }
    }
}

/// El resultado de una llamada que choca con un registro previo: **sin éxito**,
/// con el estado y la fecha del registro, y **sin** salir a la red.
impl BlogCommentReplyTool {
    async fn duplicate_result(&self, ids: &CommentIds) -> Result<ToolResult, ToolError> {
        let existing = CommentRepliesRepo::find(&self.db, &ids.comment_id)
            .await?
            .ok_or_else(|| {
                ToolError::ExecutionError(
                    "la reserva chocó pero no hay ninguna fila registrada".to_string(),
                )
            })?;
        tracing::info!(
            comment_id = %existing.comment_id,
            state = existing.state.as_str(),
            "blog_comment_reply: el comentario ya tiene un registro de respuesta"
        );
        Ok(ToolResult {
            success: false,
            data: json!({
                "comment_id": existing.comment_id,
                "post_id": existing.post_id,
                "state": existing.state.as_str(),
                "created_at": existing.created_at,
            }),
            message: Some(format!(
                "el comentario {} ya tiene una respuesta registrada en estado `{}` (desde {}); \
                 no se publica de nuevo",
                existing.comment_id,
                existing.state.as_str(),
                existing.created_at
            )),
        })
    }
}

/// Indica si un desenlace **garantiza** que no se creó ningún comentario y, por
/// tanto, permite **soltar** la reserva. `400`, `401`, `403` y `429` son esas
/// garantías (WordPress responde un `429` sin crear el comentario); un `404`
/// —tanto `rest_no_route` como `rest_comment_invalid_id`— también lo garantiza:
/// la ruta o el comentario no existen, así que nada pudo crearse. Cualquier otro
/// desenlace —timeout, fallo de red o `5xx`— es ambiguo y **conserva** la
/// reserva.
fn releases_reservation(error: &WordpressError) -> bool {
    matches!(
        error,
        WordpressError::BadRequest
            | WordpressError::Unauthorized
            | WordpressError::Forbidden
            | WordpressError::RateLimited
            | WordpressError::CommentNotFound
            | WordpressError::RouteNotFound
    )
}

/// Herramienta `blog_comment_delete`: manda a la **papelera** un comentario del
/// blog **notificado**, por la API REST del núcleo de WordPress.
///
/// La secuencia de una invocación es **validar → resolver ids → resolver config
/// → leer → borrar → informar**. Los identificadores salen **exclusivamente** del
/// `metadata` de la notificación indicada en `notification_id`; un `comment_id` o
/// un `post_id` aportado por quien llama se rechaza, de modo que nadie puede
/// elegir qué comentario se manda a la papelera. La lectura previa es obligatoria:
/// sin ella no se envía ningún `DELETE`, y es también la fuente de la evidencia.
/// El comentario **nunca** se borra definitivamente (`force=true` no se envía
/// jamás). La idempotencia la da el `410 rest_already_trashed` del sitio, que se
/// trata como éxito; la herramienta no tiene tabla, reserva ni contador propios.
pub struct BlogCommentDeleteTool {
    db: SqlitePool,
    /// Configuración inyectada (solo en tests); en producción se resuelve del
    /// entorno en cada llamada.
    config: Option<WordpressConfig>,
}

impl BlogCommentDeleteTool {
    /// Herramienta de producción: la configuración se resuelve del entorno en
    /// cada llamada.
    pub fn new(db: SqlitePool) -> Self {
        Self { db, config: None }
    }

    /// Constructor de test: inyecta la configuración para apuntar la lectura y
    /// el borrado a un servidor de prueba.
    pub fn with_config(db: SqlitePool, config: WordpressConfig) -> Self {
        Self {
            db,
            config: Some(config),
        }
    }

    /// La configuración a usar: la inyectada o la resuelta del entorno.
    fn resolve_config(&self) -> Result<WordpressConfig, ToolError> {
        match &self.config {
            Some(config) => Ok(config.clone()),
            None => Wordpress::resolve_config()
                .map_err(|error| ToolError::ExecutionError(error.to_string())),
        }
    }
}

#[async_trait]
impl Tool for BlogCommentDeleteTool {
    fn name(&self) -> &'static str {
        "blog_comment_delete"
    }

    fn description(&self) -> &'static str {
        "Blog — manda a la papelera un comentario del sitio (no se borra definitivamente); los identificadores del comentario y de la entrada salen del `metadata` de la notificación indicada en `notification_id` (el aviso que devuelve la herramienta `notifications`), nunca de argumentos explícitos"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "notification_id": {
                    "type": "string",
                    "description": "Id de la notificación que anunció el comentario; de su `metadata` salen los identificadores del comentario y de la entrada. Obligatorio: es la única vía."
                }
            },
            "required": ["notification_id"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::ExplicitApproval
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let started = std::time::Instant::now();

        // 1. Validación (sin red): procedencia de los identificadores. Solo el
        //    `metadata` del aviso: la vía explícita está retirada en el borrado.
        let ids = resolve_delete_ids(&self.db, &args).await?;

        // 2. Configuración: se resuelve en cada llamada; sin credenciales no se
        //    sale a la red.
        let config = self.resolve_config()?;

        let client = Wordpress::with_timeout(config.base_url.clone(), config.timeout);

        // 3. Lectura previa **obligatoria**: sin ella no se envía ningún DELETE.
        //    Es también la fuente de la evidencia.
        let snapshot = match client.fetch_comment(&config, &ids.comment_id).await {
            Ok(snapshot) => snapshot,
            Err(WordpressError::CommentNotFound) => {
                return Err(ToolError::NotFound(format!(
                    "el comentario {} no existe en el sitio",
                    ids.comment_id
                )));
            }
            Err(error) => {
                return Err(ToolError::ExecutionError(format!(
                    "no se pudo leer el comentario {} para borrarlo: {error}; no se ha borrado nada",
                    ids.comment_id
                )));
            }
        };

        // 4. Borrado a la papelera: una sola petición, sin reintentos y sin
        //    `force=true`.
        let outcome = client.trash_comment(&config, &ids.comment_id).await;
        let latency_ms = started.elapsed().as_millis() as u64;

        match outcome {
            Ok(outcome) => {
                let already_trashed = outcome == TrashOutcome::AlreadyTrashed;
                let outcome_label = if already_trashed {
                    "already_trashed"
                } else {
                    "trashed"
                };
                tracing::info!(
                    comment_id = %snapshot.comment_id,
                    post_id = %snapshot.post_id,
                    previous_state = %snapshot.previous_state,
                    outcome = outcome_label,
                    latency_ms,
                    "blog_comment_delete: borrado a la papelera"
                );
                let message = if already_trashed {
                    format!("el comentario {} ya estaba en la papelera", ids.comment_id)
                } else {
                    format!("el comentario {} ha quedado en la papelera", ids.comment_id)
                };
                Ok(ToolResult {
                    success: true,
                    data: json!({
                        "comment_id": snapshot.comment_id,
                        "post_id": snapshot.post_id,
                        "author": snapshot.author,
                        "excerpt": snapshot.excerpt,
                        "previous_state": snapshot.previous_state,
                        "already_trashed": already_trashed,
                    }),
                    message: Some(message),
                })
            }
            Err(WordpressError::CommentNotFound) => Err(ToolError::NotFound(format!(
                "el comentario {} no existe en el sitio",
                ids.comment_id
            ))),
            Err(error) => {
                tracing::warn!(
                    comment_id = %snapshot.comment_id,
                    post_id = %snapshot.post_id,
                    previous_state = %snapshot.previous_state,
                    outcome = %error,
                    latency_ms,
                    "blog_comment_delete: no se pudo borrar el comentario"
                );
                Err(ToolError::ExecutionError(error.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::comment_replies::{CommentRepliesRepo, CommentReplyState};
    use crate::db::repos::notifications::NotificationsRepo;
    use crate::tools::r#trait::Tool;
    use serde_json::json;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;
    use std::time::Duration;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_db() -> SqlitePool {
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

    fn config_for(server: &MockServer) -> WordpressConfig {
        WordpressConfig {
            base_url: server.uri(),
            service_user: "alice".to_string(),
            app_password: "s3cret".to_string(),
            timeout: WordpressConfig::default_timeout(),
        }
    }

    // -----------------------------------------------------------------------
    // 3.1 — la función pura `comment_ids_from_metadata`
    // -----------------------------------------------------------------------

    /// Scenario: un `metadata` con identificadores numéricos se resuelve.
    #[test]
    fn metadata_numeric_ids_resolve() {
        let ids = comment_ids_from_metadata(Some(r#"{"comment_id": 901, "post_id": 42}"#)).unwrap();
        assert_eq!(ids.comment_id, "901");
        assert_eq!(ids.post_id, "42");
    }

    /// Scenario: un `metadata` con identificadores en cadena se normaliza.
    #[test]
    fn metadata_string_ids_are_normalized() {
        let ids =
            comment_ids_from_metadata(Some(r#"{"comment_id": "901", "post_id": "42"}"#)).unwrap();
        assert_eq!(ids.comment_id, "901");
        assert_eq!(ids.post_id, "42");
    }

    /// Scenario: un `metadata` ausente o `NULL` se rechaza nombrando el problema.
    #[test]
    fn metadata_missing_is_rejected() {
        let err = comment_ids_from_metadata(None).unwrap_err();
        assert_eq!(err, MetadataError::Missing);
        assert!(err.to_string().to_lowercase().contains("metadata"), "{err}");
    }

    /// Scenario: un `metadata` que no es JSON válido se rechaza.
    #[test]
    fn metadata_invalid_json_is_rejected() {
        let err = comment_ids_from_metadata(Some("no soy json")).unwrap_err();
        assert_eq!(err, MetadataError::InvalidJson);
        assert!(err.to_string().to_lowercase().contains("json"), "{err}");
    }

    /// Scenario: un `metadata` que no es objeto (arreglo o escalar) se rechaza.
    #[test]
    fn metadata_not_an_object_is_rejected() {
        for raw in [r#"[1, 2]"#, r#""901""#, "42", "true", "null"] {
            let err = comment_ids_from_metadata(Some(raw)).unwrap_err();
            assert_eq!(err, MetadataError::NotAnObject, "raw: {raw}");
            assert!(
                err.to_string().to_lowercase().contains("objeto"),
                "raw: {raw}, msg: {err}"
            );
        }
    }

    /// Scenario: un objeto sin `comment_id` o sin `post_id` se rechaza nombrando
    /// el campo que falta.
    #[test]
    fn metadata_missing_field_is_rejected_naming_it() {
        let err = comment_ids_from_metadata(Some(r#"{"comment_id": 901}"#)).unwrap_err();
        assert_eq!(err, MetadataError::MissingField("post_id"));
        assert!(err.to_string().contains("post_id"), "{err}");

        let err = comment_ids_from_metadata(Some(r#"{"post_id": 42}"#)).unwrap_err();
        assert_eq!(err, MetadataError::MissingField("comment_id"));
        assert!(err.to_string().contains("comment_id"), "{err}");
    }

    /// Scenario: un identificador con forma inválida se rechaza nombrando el
    /// campo, y el motivo **nunca** contiene un identificador inventado.
    #[test]
    fn metadata_invalid_id_values_are_rejected_naming_field() {
        let bad = [
            r#"{"comment_id": 0, "post_id": 42}"#,
            r#"{"comment_id": -3, "post_id": 42}"#,
            r#"{"comment_id": 1.5, "post_id": 42}"#,
            r#"{"comment_id": null, "post_id": 42}"#,
            r#"{"comment_id": true, "post_id": 42}"#,
            r#"{"comment_id": [], "post_id": 42}"#,
            r#"{"comment_id": {}, "post_id": 42}"#,
            r#"{"comment_id": "", "post_id": 42}"#,
            r#"{"comment_id": "0", "post_id": 42}"#,
            r#"{"comment_id": "abc", "post_id": 42}"#,
            r#"{"comment_id": 901, "post_id": 0}"#,
            r#"{"comment_id": 901, "post_id": -1}"#,
            r#"{"comment_id": 901, "post_id": null}"#,
        ];
        for raw in bad {
            let err = comment_ids_from_metadata(Some(raw)).unwrap_err();
            let message = err.to_string();
            assert!(
                matches!(&err, MetadataError::InvalidField { .. }),
                "raw: {raw}, err: {err:?}"
            );
            assert!(
                message.contains("comment_id") || message.contains("post_id"),
                "raw: {raw}, msg: {message}"
            );
            assert!(
                !message.chars().any(|c| c.is_ascii_digit()),
                "el motivo no debe contener un identificador inventado: raw: {raw}, msg: {message}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 3.3 — resolución por `notification_id` contra la base en memoria
    // -----------------------------------------------------------------------

    /// Scenario: un `notification_id` existente con `metadata` válido publica
    /// contra los ids del `metadata`.
    #[tokio::test]
    async fn notification_id_resolves_ids_from_metadata() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let notification_id = NotificationsRepo::insert(
            &pool,
            "wordpress",
            "blog",
            "nuevo comentario",
            "info",
            Some(r#"{"comment_id": 901, "post_id": 42}"#),
        )
        .await
        .unwrap();
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .and(body_json(json!({
                "post": 42,
                "parent": 901,
                "content": "hola",
                "status": "approved"
            })))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));
        let result = tool
            .execute(json!({ "notification_id": notification_id, "content": "hola" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["wp_comment_id"], "7701");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    /// Scenario: un `notification_id` inexistente devuelve `NotFound` sin salir
    /// a la red.
    #[tokio::test]
    async fn unknown_notification_id_is_not_found_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({ "notification_id": "no-existe", "content": "hola" }))
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::NotFound(_)), "err: {err:?}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un notification_id inexistente no debe salir a la red"
        );
    }

    /// Scenario: sin `notification_id` y sin identificadores explícitos el
    /// mensaje menciona la herramienta `notifications` y no hay petición.
    #[tokio::test]
    async fn missing_identifiers_mentions_notifications_tool() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({ "content": "hola" }))
            .await
            .unwrap_err();

        match err {
            ToolError::InvalidArguments(message) => assert!(
                message.contains("notifications"),
                "el mensaje debe mencionar la herramienta `notifications`: {message}"
            ),
            other => panic!("esperaba InvalidArguments, obtuve {other:?}"),
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    /// Scenario: mezclar `notification_id` con identificadores explícitos se
    /// rechaza y no hay petición.
    #[tokio::test]
    async fn notification_id_with_explicit_ids_is_invalid() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({
                "notification_id": "n",
                "comment_id": "901",
                "post_id": "42",
                "content": "hola"
            }))
            .await
            .unwrap_err();

        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "err: {err:?}"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    /// Scenario: una notificación cuyo `metadata` no es válido devuelve
    /// `InvalidArguments` sin salir a la red.
    #[tokio::test]
    async fn notification_with_invalid_metadata_is_invalid_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let notification_id =
            NotificationsRepo::insert(&pool, "wordpress", "blog", "aviso", "info", Some("no json"))
                .await
                .unwrap();
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({ "notification_id": notification_id, "content": "hola" }))
            .await
            .unwrap_err();

        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "err: {err:?}"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // 4.1 — camino feliz
    // -----------------------------------------------------------------------

    /// Scenario: una respuesta válida se publica aprobada, con el `content`
    /// recortado y **sin** `author_name` ni `author_email` en el cuerpo.
    #[tokio::test]
    async fn publishes_trimmed_content_approved_without_author_fields() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .and(body_json(json!({
                "post": 42,
                "parent": 901,
                "content": "hola mundo",
                "status": "approved"
            })))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));
        let result = tool
            .execute(json!({
                "comment_id": "901",
                "post_id": "42",
                "content": "  hola mundo \n"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["wp_comment_id"], "7701");
        assert_eq!(result.data["status"], "approved");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body = String::from_utf8_lossy(&requests[0].body);
        assert!(
            !body.contains("author_name"),
            "el cuerpo no debe llevar `author_name` (nacería pendiente): {body}"
        );
        assert!(
            !body.contains("author_email"),
            "el cuerpo no debe llevar `author_email` (nacería pendiente): {body}"
        );
    }

    // -----------------------------------------------------------------------
    // 4.2 — validación de argumentos sin salir a la red
    // -----------------------------------------------------------------------

    /// Scenario: un `content` ausente, en blanco o de más de 4000 caracteres se
    /// rechaza con `InvalidArguments` y el servidor de prueba no recibe ninguna
    /// petición.
    #[tokio::test]
    async fn content_validation_rejects_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

        let cases = [
            ("ausente", json!({ "comment_id": "901", "post_id": "42" })),
            (
                "vacío",
                json!({ "comment_id": "901", "post_id": "42", "content": "" }),
            ),
            (
                "en blanco",
                json!({ "comment_id": "901", "post_id": "42", "content": "   \n\t " }),
            ),
            (
                "demasiado largo",
                json!({
                    "comment_id": "901",
                    "post_id": "42",
                    "content": "a".repeat(4001)
                }),
            ),
        ];

        for (label, args) in cases {
            let err = tool.execute(args).await.unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "content {label}: esperaba InvalidArguments, obtuve {err:?}"
            );
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "ninguna validación debe salir a la red"
        );
    }

    // -----------------------------------------------------------------------
    // 4.4 — registro de traza (captura con un suscriptor global y buffer por hilo)
    // -----------------------------------------------------------------------

    use crate::test_support::capture_traces;

    /// Scenario: el registro de un éxito lleva los identificadores y la latencia,
    /// pero **no** el contenido de la respuesta.
    #[tokio::test]
    async fn success_trace_has_ids_and_latency_without_content() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let notification_id = NotificationsRepo::insert(
            &pool,
            "wordpress",
            "blog",
            "nuevo comentario",
            "info",
            Some(r#"{"comment_id": 901, "post_id": 42}"#),
        )
        .await
        .unwrap();
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let buffer = capture_traces();
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));
        let content = "respuesta confidencial xyz";
        let result = tool
            .execute(json!({ "notification_id": notification_id, "content": content }))
            .await
            .unwrap();
        assert!(result.success);

        let log = buffer.contents();
        assert!(
            log.contains("comment_id"),
            "falta el campo comment_id: {log}"
        );
        assert!(log.contains("post_id"), "falta el campo post_id: {log}");
        assert!(
            log.contains("wp_comment_id"),
            "falta el campo wp_comment_id: {log}"
        );
        assert!(
            log.contains("latency"),
            "falta la latencia en el registro: {log}"
        );
        assert!(log.contains("901"), "falta el comment_id: {log}");
        assert!(log.contains("42"), "falta el post_id: {log}");
        assert!(
            log.contains(&notification_id),
            "falta el notification_id: {log}"
        );
        assert!(log.contains("7701"), "falta el wp_comment_id: {log}");
        assert!(
            !log.contains(content),
            "el registro NO debe incluir el contenido: {log}"
        );
    }

    /// Scenario: el registro de un fallo por credencial lleva el `comment_id` y
    /// el motivo, y **ni** la contraseña **ni** la cabecera `Authorization`.
    #[tokio::test]
    async fn failure_trace_has_comment_id_without_credentials() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let buffer = capture_traces();
        let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));
        let err = tool
            .execute(json!({
                "comment_id": "901",
                "post_id": "42",
                "content": "hola"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ExecutionError(_)), "err: {err:?}");

        let log = buffer.contents();
        assert!(
            log.contains("901"),
            "falta el comment_id en el fallo: {log}"
        );
        assert!(
            !log.contains("s3cret"),
            "el registro no debe incluir la contraseña: {log}"
        );
        assert!(
            !log.to_lowercase().contains("authorization"),
            "el registro no debe incluir la cabecera Authorization: {log}"
        );
    }

    // -----------------------------------------------------------------------
    // 5.1 — el ciclo de la reserva y sus desenlaces
    // -----------------------------------------------------------------------

    /// Scenario: en un éxito la fila queda `published` con el `wp_comment_id`.
    #[tokio::test]
    async fn success_marks_row_published_with_wp_id() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let result = tool
            .execute(json!({ "comment_id": "901", "post_id": "42", "content": "hola" }))
            .await
            .unwrap();
        assert!(result.success);

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Published);
        assert_eq!(row.wp_comment_id.as_deref(), Some("7701"));
        assert_eq!(row.last_status, Some(201));
    }

    /// Los desenlaces que **garantizan** que no se creó nada liberan la reserva:
    /// la fila desaparece y una segunda llamada puede publicar.
    async fn assert_released_then_republishable(status: u16, code: Option<&str>) {
        let endpoint = "/wp-json/wp/v2/comments";
        let server = MockServer::start().await;
        let pool = setup_db().await;

        let body = match code {
            Some(code) => json!({ "code": code }),
            None => json!({ "message": "sin código" }),
        };
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let args = json!({ "comment_id": "901", "post_id": "42", "content": "hola" });

        let first = tool.execute(args.clone()).await;
        assert!(first.is_err(), "status {status}: {first:?}");
        assert!(
            CommentRepliesRepo::find(&pool, "901")
                .await
                .unwrap()
                .is_none(),
            "status {status}: la reserva debe liberarse"
        );

        let second = tool.execute(args).await.unwrap();
        assert!(
            second.success,
            "status {status}: la segunda llamada debe publicar"
        );
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            2,
            "status {status}: una petición por llamada"
        );
    }

    #[tokio::test]
    async fn bad_request_releases_reservation() {
        assert_released_then_republishable(400, Some("rest_invalid_param")).await;
    }

    #[tokio::test]
    async fn unauthorized_releases_reservation() {
        assert_released_then_republishable(401, None).await;
    }

    #[tokio::test]
    async fn forbidden_releases_reservation() {
        assert_released_then_republishable(403, Some("rest_cannot_create")).await;
    }

    #[tokio::test]
    async fn rate_limited_releases_reservation() {
        assert_released_then_republishable(429, Some("atareao_valet_reply_rate_limited")).await;
    }

    /// Al publicar, un `404` con `rest_comment_invalid_id` **garantiza** que no
    /// se creó nada: la reserva se libera (la fila no queda `uncertain`) y una
    /// segunda llamada puede publicar.
    #[tokio::test]
    async fn publish_404_comment_invalid_id_releases_reservation() {
        assert_released_then_republishable(404, Some("rest_comment_invalid_id")).await;
    }

    /// Al publicar, un `404` con `rest_no_route` también garantiza que no se creó
    /// nada y libera la reserva.
    #[tokio::test]
    async fn publish_404_rest_no_route_releases_reservation() {
        assert_released_then_republishable(404, Some("rest_no_route")).await;
    }

    /// Un desenlace inesperado **conserva** su código HTTP en `last_status`: el
    /// mecanismo que registra el desenlace no se pierde.
    #[tokio::test]
    async fn unexpected_status_keeps_last_status() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(418).set_body_json(json!({ "code": "teapot" })))
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let err = tool
            .execute(json!({ "comment_id": "901", "post_id": "42", "content": "hola" }))
            .await
            .unwrap_err();
        assert!(matches!(&err, ToolError::ExecutionError(_)), "err: {err:?}");

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Uncertain);
        assert_eq!(row.last_status, Some(418));
    }

    /// Un `503` **conserva** la reserva como `uncertain` y una segunda llamada no
    /// vuelve a salir a la red.
    #[tokio::test]
    async fn server_error_leaves_reservation_uncertain() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(ResponseTemplate::new(503).set_body_string("down"))
            .mount(&server)
            .await;

        let tool = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let args = json!({ "comment_id": "901", "post_id": "42", "content": "hola" });

        assert!(tool.execute(args.clone()).await.is_err());
        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Uncertain);

        let second = tool.execute(args).await.unwrap();
        assert!(
            !second.success,
            "una reserva indeterminada no vuelve a publicar"
        );
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "la segunda llamada no debe salir a la red"
        );
    }

    /// Un fallo de red también conserva la reserva como `uncertain`.
    #[tokio::test]
    async fn network_failure_leaves_reservation_uncertain() {
        let pool = setup_db().await;
        let config = WordpressConfig {
            base_url: "http://127.0.0.1:1".to_string(),
            service_user: "alice".to_string(),
            app_password: "s3cret".to_string(),
            timeout: WordpressConfig::default_timeout(),
        };
        let tool = BlogCommentReplyTool::with_config(pool.clone(), config);

        assert!(tool
            .execute(json!({ "comment_id": "901", "post_id": "42", "content": "hola" }))
            .await
            .is_err());
        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Uncertain);
    }

    /// Un timeout conserva la reserva como `uncertain`. El timeout se **inyecta**
    /// corto (200 ms) y el servidor de prueba tarda mucho más (30 s), así que la
    /// carrera se resuelve siempre a favor del timeout **sin** pausar el reloj:
    /// el test es determinista y no depende de la carga de la máquina.
    #[tokio::test]
    async fn timeout_leaves_reservation_uncertain() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701 }))
                    .set_delay(Duration::from_secs(30)),
            )
            .mount(&server)
            .await;

        let config = WordpressConfig {
            base_url: server.uri(),
            service_user: "alice".to_string(),
            app_password: "s3cret".to_string(),
            timeout: Duration::from_millis(200),
        };
        let tool = BlogCommentReplyTool::with_config(pool.clone(), config);
        let outcome = tool
            .execute(json!({ "comment_id": "901", "post_id": "42", "content": "hola" }))
            .await;
        assert!(outcome.is_err(), "un timeout no es un éxito: {outcome:?}");

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Uncertain);
    }

    // -----------------------------------------------------------------------
    // 5.2 — negativa por duplicado
    // -----------------------------------------------------------------------

    /// Scenario: con una fila previa en cualquiera de los tres estados la
    /// herramienta devuelve un resultado **sin éxito** con el estado y la fecha,
    /// y no sale a la red.
    #[tokio::test]
    async fn duplicate_registration_refuses_without_request() {
        let states = [
            CommentReplyState::Reserved,
            CommentReplyState::Published,
            CommentReplyState::Uncertain,
        ];

        for state in states {
            let server = MockServer::start().await;
            let pool = setup_db().await;

            // Siembra la fila previa en el estado pedido.
            CommentRepliesRepo::reserve(&pool, "901", "42", Some("notif-1"))
                .await
                .unwrap();
            match state {
                CommentReplyState::Reserved => {}
                CommentReplyState::Published => {
                    CommentRepliesRepo::mark_published(&pool, "901", Some("7701"), 201)
                        .await
                        .unwrap();
                }
                CommentReplyState::Uncertain => {
                    CommentRepliesRepo::mark_uncertain(&pool, "901", Some(503))
                        .await
                        .unwrap();
                }
            }
            let existing = CommentRepliesRepo::find(&pool, "901")
                .await
                .unwrap()
                .unwrap();

            let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));
            let result = tool
                .execute(json!({ "comment_id": "901", "post_id": "42", "content": "hola" }))
                .await
                .unwrap();

            assert!(!result.success, "estado {state:?}: no debe publicar");
            let message = result.message.unwrap_or_default();
            assert!(
                message.contains(state.as_str()),
                "estado {state:?}: el mensaje debe incluir el estado: {message}"
            );
            assert!(
                message.contains(&existing.created_at),
                "estado {state:?}: el mensaje debe incluir la fecha: {message}"
            );
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "estado {state:?}: no debe haber ninguna petición"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 5.3 — concurrencia
    // -----------------------------------------------------------------------

    /// Scenario: dos llamadas concurrentes para el mismo comentario publican
    /// **una sola** vez y solo una informa de un éxito.
    #[tokio::test]
    async fn concurrent_calls_publish_once() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        Mock::given(method("POST"))
            .and(path("/wp-json/wp/v2/comments"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(json!({ "id": 7701, "status": "approved" })),
            )
            .mount(&server)
            .await;

        let tool_a = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let tool_b = BlogCommentReplyTool::with_config(pool.clone(), config_for(&server));
        let args = json!({ "comment_id": "901", "post_id": "42", "content": "hola" });

        let (a, b) = tokio::join!(tool_a.execute(args.clone()), tool_b.execute(args.clone()));

        assert!(
            a.is_ok() && b.is_ok(),
            "ambas llamadas deben resolverse con un resultado, no con un error: a={a:?} b={b:?}"
        );
        let successes = [&a, &b]
            .into_iter()
            .filter(|result| matches!(result, Ok(t) if t.success))
            .count();
        assert_eq!(successes, 1, "exactamente un éxito: a={a:?} b={b:?}");
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "exactamente una petición"
        );
    }

    // -----------------------------------------------------------------------
    // 6.1 — el registro: figura en las definiciones con sus parámetros
    // -----------------------------------------------------------------------

    /// Scenario: la herramienta figura en las definiciones del registry con sus
    /// parámetros y `content` como único obligatorio.
    #[tokio::test]
    async fn blog_comment_reply_figures_in_definitions_with_its_parameters() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);

        let def = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "blog_comment_reply")
            .expect("`blog_comment_reply` must be advertised in definitions()");

        let properties = def.parameters["properties"]
            .as_object()
            .expect("el esquema debe declarar `properties`");
        for param in ["content", "comment_id", "post_id", "notification_id"] {
            assert!(
                properties.contains_key(param),
                "el esquema debe declarar `{param}`: {properties:?}"
            );
        }

        let required: Vec<&str> = def.parameters["required"]
            .as_array()
            .expect("el esquema debe declarar `required`")
            .iter()
            .filter_map(|value| value.as_str())
            .collect();
        assert_eq!(
            required,
            vec!["content"],
            "`content` es el único campo obligatorio del esquema"
        );
    }

    /// Scenario: el permiso de `blog_comment_reply` es `ExplicitApproval`, el
    /// aprobado en 1.1 (publicar es público e irreversible).
    #[tokio::test]
    async fn blog_comment_reply_permission_is_explicit_approval() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);

        assert_eq!(
            registry.permission("blog_comment_reply", &json!({})),
            Some(Permission::ExplicitApproval)
        );
    }

    /// Scenario: la descripción está en español y nombra `content` y los
    /// identificadores del comentario y de la entrada.
    #[tokio::test]
    async fn blog_comment_reply_description_is_spanish_and_names_required() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);
        let def = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "blog_comment_reply")
            .expect("`blog_comment_reply` must be advertised in definitions()");

        let description = def.description;
        assert!(
            description.contains("comentario"),
            "la descripción debe estar en español: {description}"
        );
        assert!(
            description.contains("content"),
            "la descripción debe nombrar `content`: {description}"
        );
        assert!(
            description.contains("comment_id"),
            "la descripción debe nombrar `comment_id`: {description}"
        );
        assert!(
            description.contains("post_id"),
            "la descripción debe nombrar `post_id`: {description}"
        );
    }

    /// Scenario: registrar la herramienta dos veces es idempotente: sigue
    /// habiendo una sola definición con su nombre.
    #[tokio::test]
    async fn registering_blog_comment_reply_twice_is_idempotent() {
        let pool = setup_db().await;
        let mut registry = crate::tools::registry::ToolRegistry::new();
        registry.register(Box::new(BlogCommentReplyTool::new(pool.clone())));
        registry.register(Box::new(BlogCommentReplyTool::new(pool.clone())));

        let count = registry
            .definitions()
            .into_iter()
            .filter(|d| d.name == "blog_comment_reply")
            .count();
        assert_eq!(
            count, 1,
            "registrar dos veces no debe duplicar la definición"
        );
    }

    // -----------------------------------------------------------------------
    // 2.1 / 5.1-5.5 — la herramienta `blog_comment_delete`
    // -----------------------------------------------------------------------

    /// Cuerpo de la lectura previa de un comentario `901` de la entrada `42`.
    fn read_body(content: &str) -> serde_json::Value {
        json!({
            "id": 901,
            "post": 42,
            "author_name": "Ana",
            "content": { "rendered": content },
            "status": "approved"
        })
    }

    /// Monta la lectura previa con un `200` y el cuerpo dado.
    async fn mount_read(server: &MockServer, body: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    /// Monta el borrado con el estado dado.
    async fn mount_trash(server: &MockServer, status: u16) {
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({ "id": 901 })))
            .mount(server)
            .await;
    }

    /// Peticiones recibidas por el servidor con el método dado.
    async fn received_count(server: &MockServer, method: &str) -> usize {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.method.as_str() == method)
            .count()
    }

    /// Args del borrado en el mundo de D8: **solo** `notification_id`, tomado de
    /// una notificación recién insertada cuyo `metadata` apunta al comentario y
    /// la entrada dados. Sustituye a la vía explícita retirada.
    async fn delete_args(pool: &SqlitePool, comment_id: &str, post_id: &str) -> Value {
        let metadata = format!(r#"{{"comment_id": "{comment_id}", "post_id": "{post_id}"}}"#);
        let notification_id =
            NotificationsRepo::insert(pool, "wordpress", "blog", "aviso", "info", Some(&metadata))
                .await
                .unwrap();
        json!({ "notification_id": notification_id })
    }

    // ─── 2.1 — resolución de identificadores (reuso del parseo compartido) ───

    #[tokio::test]
    async fn delete_missing_identifiers_is_invalid_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

        let err = tool.execute(json!({})).await.unwrap_err();
        assert!(
            matches!(&err, ToolError::InvalidArguments(_)),
            "err: {err:?}"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    /// D8 — el borrado rechaza **toda** forma de identificador explícito que
    /// acompañe a `notification_id`, no solo cadenas: números, arrays y objetos
    /// también. La notificación es **real** y su `metadata` sería resoluble, así
    /// que el rechazo tiene que venir del identificador explícito y no de la
    /// ausencia de la notificación ni de un `metadata` inválido. Blinda
    /// `has_explicit` frente a tipos no previstos: una mutación que solo
    /// rechazara ids explícitos de tipo cadena dejaría «colar» el numérico hasta
    /// la red y este test se pondría rojo.
    #[tokio::test]
    async fn delete_notification_id_with_explicit_ids_is_invalid() {
        let shapes = [
            json!("901"),         // cadena
            json!(901),           // número
            json!([901]),         // array
            json!({ "id": 901 }), // objeto
        ];
        for field in ["comment_id", "post_id"] {
            for shape in &shapes {
                let server = MockServer::start().await;
                let pool = setup_db().await;
                let mut args = delete_args(&pool, "901", "42").await;
                args[field] = shape.clone();

                let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
                let err = tool.execute(args).await.unwrap_err();
                assert!(
                    matches!(&err, ToolError::InvalidArguments(_)),
                    "field `{field}`, forma {shape}: err: {err:?}"
                );
                assert!(
                    server.received_requests().await.unwrap().is_empty(),
                    "field `{field}`, forma {shape}: el rechazo no debe salir a la red"
                );
            }
        }
    }

    /// INFO (D8) — un `comment_id`/`post_id` presente pero `null` se **tolera**
    /// (`null` = ausente): el borrado procede del `metadata` de la notificación y
    /// no lo confunde con un identificador explícito. Fija el comportamiento
    /// esperado: si `has_explicit` dejara de tratar `null` como ausente, este test
    /// se pondría rojo.
    #[tokio::test]
    async fn delete_tolerates_null_explicit_ids_as_absent() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let mut args = delete_args(&pool, "901", "42").await;
        args["comment_id"] = json!(null);
        args["post_id"] = json!(null);
        mount_read_and_trash_for(&server, "901").await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let result = tool.execute(args).await.unwrap();

        assert!(result.success, "{result:?}");
        assert_eq!(result.data["comment_id"], "901");
        assert_eq!(received_count(&server, "GET").await, 1);
        assert_eq!(received_count(&server, "DELETE").await, 1);
    }

    #[tokio::test]
    async fn delete_unknown_notification_id_is_not_found_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({ "notification_id": "no-existe" }))
            .await
            .unwrap_err();
        assert!(matches!(&err, ToolError::NotFound(_)), "err: {err:?}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_invalid_metadata_is_invalid_without_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let notification_id =
            NotificationsRepo::insert(&pool, "wordpress", "blog", "aviso", "info", Some("no json"))
                .await
                .unwrap();
        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

        let err = tool
            .execute(json!({ "notification_id": notification_id }))
            .await
            .unwrap_err();
        assert!(
            matches!(&err, ToolError::InvalidArguments(_)),
            "err: {err:?}"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    // ─── C1 / D8 — los identificadores explícitos: validados (respuesta) y retirados (borrado) ─

    /// Ninguna petición a WordPress debe llevar `force` distinto de `false` ni
    /// una ruta reescrita, y un identificador inválido no debe salir a la red.
    async fn assert_no_unsafe_request(server: &MockServer, case: &str) {
        let requests = server.received_requests().await.unwrap();
        for request in &requests {
            let url = request.url.as_str();
            assert!(
                !url.contains("force=true") && !url.contains("force=1"),
                "caso `{case}`: ninguna petición debe llevar force distinto de false: {url}"
            );
            assert!(
                !url.contains(".."),
                "caso `{case}`: la ruta no debe reescribirse: {url}"
            );
        }
        assert!(
            requests.is_empty(),
            "caso `{case}`: un identificador inválido no debe salir a la red: {:?}",
            requests
                .iter()
                .map(|request| (request.method.as_str(), request.url.as_str()))
                .collect::<Vec<_>>()
        );
    }

    /// Identificadores que, interpolados crudos, inyectarían `force=true` o
    /// reescribirían la ruta; todos deben rechazarse sin tocar la red.
    const INJECTED_IDS: [&str; 4] = ["901?force=true", "901?force=1", "1/../../wp/v2/posts/5", ""];

    /// Rework C1/D8 — en el mundo **sin** vía explícita, un `comment_id` —aunque
    /// sea una carga de inyección— se rechaza con `InvalidArguments` sin salir a
    /// la red; se conserva la comprobación de que ninguna URL lleva `force`
    /// distinto de `false` ni una ruta reescrita.
    #[tokio::test]
    async fn delete_rejects_injected_comment_id_without_any_request() {
        for case in INJECTED_IDS {
            let server = MockServer::start().await;
            let pool = setup_db().await;
            let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

            let err = tool
                .execute(json!({ "comment_id": case, "post_id": "42" }))
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ToolError::InvalidArguments(_)),
                "caso `{case}`: err: {err:?}"
            );
            assert_no_unsafe_request(&server, case).await;
        }
    }

    /// Rework C1/D8 — lo mismo por `post_id`: en el mundo sin vía explícita se
    /// rechaza sin salir a la red, con la URL a salvo.
    #[tokio::test]
    async fn delete_rejects_injected_post_id_without_any_request() {
        for case in INJECTED_IDS {
            let server = MockServer::start().await;
            let pool = setup_db().await;
            let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

            let err = tool
                .execute(json!({ "comment_id": "901", "post_id": case }))
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ToolError::InvalidArguments(_)),
                "caso `{case}`: err: {err:?}"
            );
            assert_no_unsafe_request(&server, case).await;
        }
    }

    #[tokio::test]
    async fn reply_rejects_injected_comment_id_without_any_request() {
        for case in INJECTED_IDS {
            let server = MockServer::start().await;
            let pool = setup_db().await;
            let tool = BlogCommentReplyTool::with_config(pool, config_for(&server));

            let err = tool
                .execute(json!({
                    "comment_id": case,
                    "post_id": "42",
                    "content": "hola"
                }))
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ToolError::InvalidArguments(_)),
                "caso `{case}`: err: {err:?}"
            );
            assert_no_unsafe_request(&server, case).await;
        }
    }

    /// La vía del `metadata` sigue igual de estricta: una forma inválida que se
    /// rechaza al responder se rechaza también al borrar, con la ruta y el
    /// `force` a salvo.
    #[tokio::test]
    async fn metadata_path_stays_strict_for_injected_ids() {
        for case in INJECTED_IDS {
            let server = MockServer::start().await;
            let pool = setup_db().await;
            let metadata = format!(r#"{{"comment_id": "{case}", "post_id": "42"}}"#);
            let notification_id = NotificationsRepo::insert(
                &pool,
                "wordpress",
                "blog",
                "aviso",
                "info",
                Some(&metadata),
            )
            .await
            .unwrap();
            let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));

            let err = tool
                .execute(json!({ "notification_id": notification_id }))
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ToolError::InvalidArguments(_)),
                "caso `{case}`: err: {err:?}"
            );
            assert_no_unsafe_request(&server, case).await;
        }
    }

    // ─── D8 — el borrado NO admite identificadores explícitos ────────────────

    /// Scenario (D8): los identificadores del borrado salen **exclusivamente**
    /// del `metadata` del aviso apuntado por `notification_id`. Aunque quien
    /// llama aporte un `comment_id` que **existe** y sería borrable —el servidor
    /// de prueba respondería `200`—, sin `notification_id` la herramienta falla
    /// con `InvalidArguments` y **no envía ninguna petición**: el comentario
    /// sigue sin tocarse. Mide la retirada de la vía explícita.
    #[tokio::test]
    async fn delete_retires_explicit_ids_and_never_reaches_the_site() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        // Un montaje que **serviría** el borrado de `901` con éxito: como la vía
        // explícita está retirada, no debe llegarle ninguna petición.
        mount_read_and_trash_for(&server, "901").await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool
            .execute(json!({ "comment_id": "901", "post_id": "42" }))
            .await
            .unwrap_err();
        assert!(
            matches!(&err, ToolError::InvalidArguments(_)),
            "err: {err:?}"
        );
        let message = err.to_string().to_lowercase();
        assert!(
            message.contains("notification_id"),
            "el mensaje debe remitir a `notification_id`: {message}"
        );
        assert_no_unsafe_request(&server, "comment_id explícito").await;
    }

    // ─── 5.1 — camino feliz ──────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_reads_then_trashes_with_force_false_and_no_body() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("Un comentario de prueba")).await;
        mount_trash(&server, 200).await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let result = tool.execute(args).await.unwrap();

        assert!(result.success, "{result:?}");
        let message = result.message.unwrap_or_default().to_lowercase();
        assert!(message.contains("papelera"), "{message}");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method.as_str(), "GET");
        assert_eq!(requests[1].method.as_str(), "DELETE");
        assert!(
            requests[1].body.is_empty(),
            "el DELETE nunca debe llevar cuerpo"
        );
        let query = requests[1].url.query().unwrap_or("");
        assert!(query.contains("force=false"), "falta force=false: {query}");
        assert!(!query.contains("force=true"), "{query}");
    }

    #[tokio::test]
    async fn delete_with_notification_id_uses_metadata_ids() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let notification_id = NotificationsRepo::insert(
            &pool,
            "wordpress",
            "blog",
            "aviso",
            "info",
            Some(r#"{"comment_id": "901", "post_id": "42"}"#),
        )
        .await
        .unwrap();
        mount_read(&server, read_body("hola")).await;
        mount_trash(&server, 200).await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let result = tool
            .execute(json!({ "notification_id": notification_id }))
            .await
            .unwrap();
        assert!(result.success, "{result:?}");
        assert_eq!(result.data["comment_id"], "901");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].url.path(), "/wp-json/wp/v2/comments/901");
    }

    // ─── 5.2 — lectura previa obligatoria ────────────────────────────────────

    #[tokio::test]
    async fn delete_read_failure_sends_no_delete() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(ResponseTemplate::new(503).set_body_string("down"))
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        assert!(matches!(&err, ToolError::ExecutionError(_)), "err: {err:?}");
        assert!(err.to_string().to_lowercase().contains("no se ha borrado"));
        assert_eq!(received_count(&server, "DELETE").await, 0);
    }

    #[tokio::test]
    #[serial]
    async fn delete_without_credentials_sends_no_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        std::env::remove_var("BLOG_SERVICE_USER");
        std::env::remove_var("BLOG_APP_PASSWORD");

        let tool = BlogCommentDeleteTool::new(pool);
        let err = tool.execute(args).await.unwrap_err();
        let message = err.to_string();
        assert!(message.contains("BLOG_SERVICE_USER"), "{message}");
        assert!(message.contains("BLOG_APP_PASSWORD"), "{message}");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    // ─── 5.3 — evidencia sin credenciales ────────────────────────────────────

    #[tokio::test]
    async fn delete_result_carries_evidence_without_credentials() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        let long = "x".repeat(250);
        mount_read(&server, read_body(&long)).await;
        mount_trash(&server, 200).await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let result = tool.execute(args).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["comment_id"], "901");
        assert_eq!(result.data["post_id"], "42");
        assert_eq!(result.data["author"], "Ana");
        assert_eq!(result.data["previous_state"], "approved");
        let excerpt = result.data["excerpt"].as_str().unwrap();
        assert_eq!(
            excerpt.chars().count(),
            200,
            "el extracto debe estar acotado a 200"
        );

        let rendered = format!("{:?} {:?}", result.data, result.message);
        assert!(
            !rendered.contains("s3cret"),
            "la credencial no debe aparecer: {rendered}"
        );
        assert!(
            !rendered.to_lowercase().contains("authorization"),
            "la cabecera no debe aparecer: {rendered}"
        );
    }

    #[tokio::test]
    async fn delete_failure_trace_has_ids_without_content_or_credentials() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        let secret = "contenido confidencial xyz";
        mount_read(&server, read_body(secret)).await;
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "code": "rest_cannot_delete" })),
            )
            .mount(&server)
            .await;

        let buffer = capture_traces();
        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        assert!(matches!(&err, ToolError::ExecutionError(_)), "err: {err:?}");

        let log = buffer.contents();
        assert!(log.contains("901"), "falta el comment_id: {log}");
        assert!(
            log.contains("previous_state"),
            "falta el estado previo: {log}"
        );
        assert!(log.contains("outcome"), "falta el desenlace: {log}");
        assert!(log.contains("latency"), "falta la latencia: {log}");
        assert!(
            !log.contains(secret),
            "el registro no debe incluir el contenido: {log}"
        );
        assert!(
            !log.contains("s3cret"),
            "el registro no debe incluir la contraseña: {log}"
        );
        assert!(!log.to_lowercase().contains("authorization"), "{log}");
    }

    // ─── 5.4 — idempotencia del `410` ────────────────────────────────────────

    #[tokio::test]
    async fn delete_410_is_already_trashed_success_without_extra_request() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("hola")).await;
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(410).set_body_json(json!({ "code": "rest_already_trashed" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let result = tool.execute(args).await.unwrap();

        assert!(result.success, "{result:?}");
        assert_eq!(result.data["already_trashed"], true);
        let message = result.message.unwrap_or_default().to_lowercase();
        assert!(message.contains("ya estaba"), "{message}");
        assert_eq!(received_count(&server, "GET").await, 1);
        assert_eq!(received_count(&server, "DELETE").await, 1);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn delete_twice_sends_one_get_one_delete_each() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("hola")).await;
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(410).set_body_json(json!({ "code": "rest_already_trashed" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let first = tool.execute(args.clone()).await.unwrap();
        let second = tool.execute(args).await.unwrap();

        assert!(first.success && second.success);
        assert_eq!(first.data["already_trashed"], true);
        assert_eq!(second.data["already_trashed"], true);
        assert_eq!(received_count(&server, "GET").await, 2);
        assert_eq!(received_count(&server, "DELETE").await, 2);
    }

    // ─── M1 — varios borrados distintos no se frenan entre sí ────────────────

    /// Monta la lectura y el borrado (`200`) de un comentario cualquiera.
    async fn mount_read_and_trash_for(server: &MockServer, id: &str) {
        let numeric: u64 = id.parse().unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/wp-json/wp/v2/comments/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": numeric,
                "post": 42,
                "author_name": "Ana",
                "content": { "rendered": "hola" },
                "status": "approved"
            })))
            .mount(server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!("/wp-json/wp/v2/comments/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": numeric })))
            .mount(server)
            .await;
    }

    /// Scenario: dos comentarios **distintos** borrados en el mismo turno: dos
    /// lecturas y dos borrados, ninguno bloqueado por el anterior.
    #[tokio::test]
    async fn two_distinct_deletions_in_one_turn_are_not_blocked() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let first_args = delete_args(&pool, "901", "42").await;
        let second_args = delete_args(&pool, "902", "42").await;
        mount_read_and_trash_for(&server, "901").await;
        mount_read_and_trash_for(&server, "902").await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let first = tool.execute(first_args).await.unwrap();
        let second = tool.execute(second_args).await.unwrap();

        assert!(
            first.success && second.success,
            "first={first:?} second={second:?}"
        );
        assert_eq!(first.data["comment_id"], "901");
        assert_eq!(second.data["comment_id"], "902");
        assert_eq!(received_count(&server, "GET").await, 2);
        assert_eq!(received_count(&server, "DELETE").await, 2);
    }

    /// Scenario: el borrado **no declara ninguna cota**: N borrados de
    /// comentarios distintos, todos con éxito, sin crear ninguna tabla y sin
    /// escribir ninguna fila de estado (ni contador, ni ventana, ni tabla).
    #[tokio::test]
    async fn many_distinct_deletions_declare_no_quota_and_write_nothing() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let ids: Vec<String> = (901..=905).map(|n| n.to_string()).collect();
        let mut args = Vec::new();
        for id in &ids {
            args.push(delete_args(&pool, id, "42").await);
            mount_read_and_trash_for(&server, id).await;
        }

        let tables_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'")
                .fetch_one(&pool)
                .await
                .unwrap();

        let tool = BlogCommentDeleteTool::with_config(pool.clone(), config_for(&server));
        for (id, arg) in ids.iter().zip(args) {
            let result = tool.execute(arg).await.unwrap();
            assert!(
                result.success,
                "id {id}: ninguna cota debe frenar: {result:?}"
            );
        }

        assert_eq!(received_count(&server, "GET").await, ids.len());
        assert_eq!(received_count(&server, "DELETE").await, ids.len());

        let tables_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            tables_before, tables_after,
            "el borrado no debe crear ninguna tabla de estado"
        );
        let state_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comment_replies")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            state_rows, 0,
            "el borrado no debe escribir ninguna fila de estado"
        );
    }

    // ─── 5.5 — política de errores de la herramienta ─────────────────────────

    #[tokio::test]
    async fn delete_404_in_read_is_not_found_without_delete() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        Mock::given(method("GET"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_json(json!({ "code": "rest_comment_invalid_id" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        assert!(matches!(&err, ToolError::NotFound(_)), "err: {err:?}");
        assert!(
            err.to_string().to_lowercase().contains("no existe"),
            "{err}"
        );
        assert_eq!(received_count(&server, "DELETE").await, 0);
    }

    #[tokio::test]
    async fn delete_403_reports_role_with_single_delete() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("hola")).await;
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "code": "rest_cannot_delete" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        assert!(matches!(&err, ToolError::ExecutionError(_)), "err: {err:?}");
        assert!(err.to_string().to_lowercase().contains("rol"), "{err}");
        assert_eq!(received_count(&server, "DELETE").await, 1);
    }

    #[tokio::test]
    async fn delete_501_reports_no_trash_and_never_force_true() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("hola")).await;
        Mock::given(method("DELETE"))
            .and(path("/wp-json/wp/v2/comments/901"))
            .respond_with(
                ResponseTemplate::new(501)
                    .set_body_json(json!({ "code": "rest_trash_not_supported" })),
            )
            .mount(&server)
            .await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        let message = err.to_string().to_lowercase();
        assert!(message.contains("papelera"), "{message}");
        assert!(
            message.contains("no se ha borrado") || message.contains("no borra"),
            "{message}"
        );
        for request in server.received_requests().await.unwrap() {
            let query = request.url.query().unwrap_or("");
            assert!(!query.contains("force=true"), "{query}");
        }
        assert_eq!(received_count(&server, "DELETE").await, 1);
    }

    #[tokio::test]
    async fn delete_503_gets_exactly_one_get_and_one_delete() {
        let server = MockServer::start().await;
        let pool = setup_db().await;
        let args = delete_args(&pool, "901", "42").await;
        mount_read(&server, read_body("hola")).await;
        mount_trash(&server, 503).await;

        let tool = BlogCommentDeleteTool::with_config(pool, config_for(&server));
        let err = tool.execute(args).await.unwrap_err();
        assert!(matches!(&err, ToolError::ExecutionError(_)), "err: {err:?}");
        assert_eq!(received_count(&server, "GET").await, 1);
        assert_eq!(received_count(&server, "DELETE").await, 1);
    }

    // ─── 6.1 — registro ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn blog_comment_delete_advertises_only_notification_id_as_required() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);

        let def = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "blog_comment_delete")
            .expect("`blog_comment_delete` must be advertised in definitions()");

        let properties = def.parameters["properties"]
            .as_object()
            .expect("el esquema debe declarar `properties`");
        assert!(
            properties.contains_key("notification_id"),
            "el esquema debe declarar `notification_id`: {properties:?}"
        );
        for param in ["comment_id", "post_id"] {
            assert!(
                !properties.contains_key(param),
                "el borrado NO debe declarar `{param}` (D8): {properties:?}"
            );
        }
        let required = def.parameters["required"]
            .as_array()
            .expect("el esquema debe declarar `required`");
        let required: Vec<&str> = required.iter().filter_map(|value| value.as_str()).collect();
        assert_eq!(
            required,
            vec!["notification_id"],
            "`notification_id` debe ser el único campo obligatorio del borrado"
        );
    }

    #[tokio::test]
    async fn blog_comment_delete_permission_is_explicit_approval() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);

        assert_eq!(
            registry.permission("blog_comment_delete", &json!({})),
            Some(Permission::ExplicitApproval)
        );
    }

    #[tokio::test]
    async fn blog_comment_delete_description_is_spanish_and_documents_trash() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);
        let def = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "blog_comment_delete")
            .expect("`blog_comment_delete` must be advertised in definitions()");

        let description = def.description;
        assert!(
            description.to_lowercase().contains("comentario"),
            "la descripción debe estar en español: {description}"
        );
        assert!(
            description.contains("papelera"),
            "la descripción debe documentar la papelera: {description}"
        );
        assert!(
            description.contains("comment_id") || description.contains("notification_id"),
            "la descripción debe nombrar la procedencia de los identificadores: {description}"
        );
    }

    #[tokio::test]
    async fn registering_blog_comment_delete_twice_is_idempotent() {
        let pool = setup_db().await;
        let mut registry = crate::tools::registry::ToolRegistry::new();
        registry.register(Box::new(BlogCommentDeleteTool::new(pool.clone())));
        registry.register(Box::new(BlogCommentDeleteTool::new(pool.clone())));

        let count = registry
            .definitions()
            .into_iter()
            .filter(|d| d.name == "blog_comment_delete")
            .count();
        assert_eq!(
            count, 1,
            "registrar dos veces no debe duplicar la definición"
        );
    }

    #[tokio::test]
    async fn production_registry_has_thirty_two_tools() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);
        assert_eq!(
            registry.definitions().len(),
            32,
            "el registry de producción debe registrar treinta y dos herramientas"
        );
    }
}
