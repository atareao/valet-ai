//! Rutas HTTP de la bandeja de notificaciones (`/api/notifications`).
//!
//! Tres endpoints: la **ingesta** desde máquinas (autenticada por un token de
//! aplicación, ver [`crate::middleware::auth::require_ingest_key`]), el listado
//! de no leídas y la marca idempotente de lectura (ambos con sesión OIDC). La
//! persistencia vive en [`NotificationsRepo`]; aquí solo se valida la entrada y
//! se traduce el resultado a HTTP. El `INSERT` precede a la respuesta y el
//! despacho WebPush es *best-effort*, en segundo plano.
//!
//! Ver `openspec/specs/notifications/spec.md`.

use axum::extract::DefaultBodyLimit;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::repos::notifications::{Notification, NotificationsRepo};
use crate::errors::AppError;
use crate::middleware::auth::{require_ingest_key, AuthenticatedIngestKey};
use crate::models::pagination::PaginatedResponse;
use crate::services::push::{self, BroadcastOutcome, PushPayload};
use crate::AppState;

/// Longitud máxima de `message` en bytes, tras `trim`.
const MAX_MESSAGE_BYTES: usize = 4096;

/// Longitud máxima de `origin` en bytes.
const MAX_ORIGIN_BYTES: usize = 64;

/// Longitud máxima de `author` en bytes.
const MAX_AUTHOR_BYTES: usize = 128;

/// Longitud máxima de `metadata` serializado, en bytes (tras fusionar
/// `body_markdown`).
const MAX_METADATA_BYTES: usize = 8192;

/// Límite del cuerpo HTTP de `POST /api/notifications`, en bytes (64 KiB).
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Clave de `metadata` donde se anexa el `body_markdown` del envelope.
const BODY_MARKDOWN_KEY: &str = "body_markdown";

/// `limit` por defecto de `GET /api/notifications/unread`.
const DEFAULT_UNREAD_LIMIT: i64 = 50;

/// `limit` máximo de `GET /api/notifications/unread`.
const MAX_UNREAD_LIMIT: i64 = 200;

/// Icono del aviso WebPush despachado al ingerir.
const PUSH_ICON: &str = "/icon-192.png";

/// Registra las rutas de `/api/notifications`.
///
/// El middleware de ingesta y el límite de cuerpo se aplican **solo** a
/// `POST /api/notifications` mediante un sub-router con `route_layer` (que solo
/// envuelve las rutas ya añadidas antes de la llamada); el listado y la marca se
/// añaden después, sin ese middleware, para quedar bajo la sesión OIDC.
pub fn routes(state: AppState) -> Router<AppState> {
    let ingest = Router::new()
        .route("/api/notifications", post(ingest))
        .route_layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            require_ingest_key,
        ));

    ingest
        .route("/api/notifications/unread", get(list_unread))
        .route("/api/notifications/mark-read", post(mark_read))
}

/// Cuerpo de `POST /api/notifications`.
#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    /// Texto de la notificación. Opcional si `title` aporta el texto.
    pub message: Option<String>,
    /// Título del aviso. Sirve de mensaje cuando `message` falta o queda en
    /// blanco (envelope de webhooks, p. ej. OpenObserve).
    pub title: Option<String>,
    /// Origen del aviso. Opcional: por defecto, el nombre de la clave.
    pub origin: Option<String>,
    /// Autor del aviso. Opcional: por defecto, el nombre de la clave.
    pub author: Option<String>,
    /// Nivel del aviso (`info`, `warning`, `critical`). Por defecto, `info`.
    pub level: Option<String>,
    /// Severidad del envelope de webhooks (`critical`, `warning`, `info`).
    /// Solo se consulta cuando falta `level`; cualquier otro valor cae a `info`.
    pub severity: Option<String>,
    /// Cuerpo en Markdown del aviso (envelope de webhooks). Si trae texto, se
    /// anexa a `metadata` bajo la clave `body_markdown` sin pisarla.
    pub body_markdown: Option<String>,
    /// Metadatos libres. Si está presente, debe ser un objeto JSON.
    pub metadata: Option<Value>,
}

/// Cuerpo ya validado y normalizado, listo para persistir.
struct ValidatedIngest {
    origin: String,
    author: String,
    message: String,
    level: String,
    metadata: Option<String>,
}

/// `POST /api/notifications` — ingesta autenticada por token de aplicación.
///
/// Valida y normaliza el cuerpo, persiste la fila (`INSERT` **antes** de
/// responder) y devuelve `201` con `{"id": "<uuid>"}`. Después, en segundo
/// plano, despacha el aviso WebPush a todas las suscripciones: un fallo del
/// despacho se registra pero **no** altera la respuesta ya emitida. Cualquier
/// validación fallida responde `400` con `{"error": ...}`; un cuerpo mayor de
/// 64 KiB responde `413` (límite aplicado por el sub-router).
async fn ingest(
    State(state): State<AppState>,
    Extension(key): Extension<AuthenticatedIngestKey>,
    Json(payload): Json<IngestRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let validated = validate(&key.name, payload)?;

    let id = NotificationsRepo::insert(
        &state.db,
        &validated.origin,
        &validated.author,
        &validated.message,
        &validated.level,
        validated.metadata.as_deref(),
    )
    .await?;

    dispatch_push(
        state.db.clone(),
        id.clone(),
        validated.origin,
        validated.author,
        validated.message,
    );

    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// Valida y normaliza el cuerpo de ingesta.
///
/// `message` es opcional si `title` aporta texto: el mensaje se deriva de
/// `message` y, si falta o queda en blanco, de `title`. `origin` y `author`
/// ausentes o en blanco caen al nombre de la clave. Los límites son en bytes,
/// tras recortar los espacios de `message` y de los campos opcionales.
fn validate(key_name: &str, payload: IngestRequest) -> Result<ValidatedIngest, AppError> {
    let message = non_blank(payload.message)
        .or_else(|| non_blank(payload.title))
        .ok_or_else(|| AppError::BadRequest("message is required".to_string()))?;
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(AppError::BadRequest(format!(
            "message must be at most {MAX_MESSAGE_BYTES} bytes"
        )));
    }

    let origin = normalize_optional(payload.origin, key_name, MAX_ORIGIN_BYTES, "origin")?;
    let author = normalize_optional(payload.author, key_name, MAX_AUTHOR_BYTES, "author")?;

    let level = match payload.level {
        Some(level) => {
            if !matches!(level.as_str(), "info" | "warning" | "critical") {
                return Err(AppError::BadRequest(
                    "level must be one of info, warning or critical".to_string(),
                ));
            }
            level
        }
        None => severity_level(payload.severity.as_deref()),
    };

    let metadata = merge_body_markdown(payload.metadata, payload.body_markdown)?;
    let metadata = match metadata {
        None => None,
        Some(value) => {
            let serialized = serde_json::to_string(&value)
                .map_err(|error| AppError::BadRequest(format!("invalid metadata: {error}")))?;
            if serialized.len() > MAX_METADATA_BYTES {
                return Err(AppError::BadRequest(format!(
                    "metadata must serialize to at most {MAX_METADATA_BYTES} bytes"
                )));
            }
            Some(serialized)
        }
    };

    Ok(ValidatedIngest {
        origin,
        author,
        message,
        level,
        metadata,
    })
}

/// Normaliza un campo opcional de texto: `trim`, blanco → `fallback`, y límite
/// de bytes.
fn normalize_optional(
    value: Option<String>,
    fallback: &str,
    max_bytes: usize,
    field: &str,
) -> Result<String, AppError> {
    let value = non_blank(value).unwrap_or_else(|| fallback.to_string());
    if value.len() > max_bytes {
        return Err(AppError::BadRequest(format!(
            "{field} must be at most {max_bytes} bytes"
        )));
    }
    Ok(value)
}

/// Recorta un texto opcional y trata el resultado en blanco como ausente.
fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Nivel derivado de `severity`: normaliza a minúsculas y mapea a la lista
/// blanca, cayendo a `info` ante cualquier otro valor (nunca rechaza).
fn severity_level(severity: Option<&str>) -> String {
    match severity.map(str::to_lowercase).as_deref() {
        Some("critical") => "critical",
        Some("warning") => "warning",
        _ => "info",
    }
    .to_string()
}

/// Valida `metadata` y le anexa `body_markdown` bajo su propia clave.
///
/// Un `body_markdown` en blanco tras `trim` no añade nada, y una clave
/// `body_markdown` ya presente **no** se pisa. Sin `metadata` ni `body_markdown`
/// el resultado es `None` (columna NULL). El tamaño se comprueba después, sobre
/// el objeto ya fusionado, en [`validate`].
fn merge_body_markdown(
    metadata: Option<Value>,
    body_markdown: Option<String>,
) -> Result<Option<Value>, AppError> {
    let body_markdown = non_blank(body_markdown);

    let mut value = match metadata {
        Some(value) if !value.is_object() => {
            return Err(AppError::BadRequest(
                "metadata must be a JSON object".to_string(),
            ))
        }
        Some(value) => value,
        None => match body_markdown {
            None => return Ok(None),
            Some(_) => Value::Object(serde_json::Map::new()),
        },
    };

    if let Some(body) = body_markdown {
        if let Some(object) = value.as_object_mut() {
            object
                .entry(BODY_MARKDOWN_KEY)
                .or_insert(Value::String(body));
        }
    }

    Ok(Some(value))
}

/// Despacha el aviso WebPush en segundo plano, *best-effort*.
///
/// Carga las claves VAPID y el cliente HTTP **dentro** de la tarea (como hace el
/// worker de recordatorios), construye el payload y lo envía a todas las
/// suscripciones con [`push::send_to_all`]. Cualquier fallo (claves, cliente o
/// envío) se registra y no afecta a la respuesta ya emitida.
fn dispatch_push(
    pool: sqlx::SqlitePool,
    id: String,
    origin: String,
    author: String,
    message: String,
) {
    tokio::spawn(async move {
        let keys = match push::load_or_generate(&pool).await {
            Ok(keys) => keys,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "notifications: no se pudieron cargar las claves VAPID; no se despacha"
                );
                return;
            }
        };
        let client = match push::client() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "notifications: no se pudo construir el cliente HTTP; no se despacha"
                );
                return;
            }
        };

        let payload = PushPayload {
            title: format!("[{origin}] {author}"),
            body: message,
            icon: Some(PUSH_ICON.to_string()),
            url: Some("/".to_string()),
            tag: Some(format!("notification-{id}")),
        };

        let sender = push::WebPushSender::new(client, keys);
        match push::send_to_all(&pool, &sender, &payload).await {
            Ok(outcome) => {
                // Solo se registra si hubo trabajo, como el worker.
                if outcome != BroadcastOutcome::default() {
                    tracing::info!(
                        attempted = %outcome.attempted,
                        accepted = %outcome.accepted,
                        failed = %outcome.failed,
                        pruned = %outcome.pruned,
                        "notifications: despacho completado"
                    );
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "notifications: fallo el despacho");
            }
        }
    });
}

/// Parámetros de consulta de `GET /api/notifications/unread`.
#[derive(Debug, Deserialize)]
struct UnreadQuery {
    /// Límite de resultados como texto, para poder rechazar los no numéricos
    /// con un `400` JSON propio en vez de la respuesta de `Query`.
    limit: Option<String>,
    /// Cursor opaco de paginación (`"<created_at>|<rowid>"`).
    cursor: Option<String>,
}

/// Item de una notificación tal como viaja en el listado.
#[derive(Debug, Serialize)]
struct NotificationItem {
    id: String,
    origin: String,
    author: String,
    message: String,
    level: String,
    /// `metadata` ya parseado a objeto JSON, o `null` si la columna es NULL.
    metadata: Option<Value>,
    created_at: String,
}

/// `GET /api/notifications/unread` — lista las no leídas, más recientes primero.
///
/// `200` con un [`PaginatedResponse`] (`data`, `next_cursor`, `total`). La
/// paginación es por cursor keyset: si hay más filas que `limit`, la respuesta
/// trae un `next_cursor` no nulo que la siguiente petición pasa en `?cursor=`.
/// Un `limit` no numérico o fuera de `[1, 200]`, o un `cursor` malformado,
/// responde `400` con `{"error": ...}`.
async fn list_unread(
    State(state): State<AppState>,
    Query(query): Query<UnreadQuery>,
) -> Result<Json<PaginatedResponse<NotificationItem>>, AppError> {
    let limit = parse_limit(query.limit.as_deref())?;

    if let Some(raw) = query.cursor.as_deref() {
        if NotificationsRepo::parse_cursor(raw).is_none() {
            return Err(AppError::BadRequest("cursor is malformed".to_string()));
        }
    }

    let (items, next_cursor, total) =
        NotificationsRepo::list_unread(&state.db, limit, query.cursor.as_deref()).await?;

    let data = items.into_iter().map(to_item).collect();

    Ok(Json(PaginatedResponse {
        data,
        next_cursor,
        total: Some(total),
    }))
}

/// Interpreta el `limit`: por defecto 50, y `400` si no es numérico o se sale de
/// `[1, 200]`.
fn parse_limit(raw: Option<&str>) -> Result<i64, AppError> {
    match raw {
        None => Ok(DEFAULT_UNREAD_LIMIT),
        Some(raw) => raw
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|value| (1..=MAX_UNREAD_LIMIT).contains(value))
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "limit must be an integer in [1, {MAX_UNREAD_LIMIT}]"
                ))
            }),
    }
}

/// Convierte una [`Notification`] persistida en su item de respuesta,
/// parseando `metadata` a objeto JSON (o `null` si falta o no parsea).
fn to_item(notification: Notification) -> NotificationItem {
    let metadata = notification
        .metadata
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    NotificationItem {
        id: notification.id,
        origin: notification.origin,
        author: notification.author,
        message: notification.message,
        level: notification.level,
        metadata,
        created_at: notification.created_at,
    }
}

/// Cuerpo de `POST /api/notifications/mark-read`.
#[derive(Debug, Deserialize)]
struct MarkReadRequest {
    /// `None` = campo ausente (marca todas); `Some(None)` = `null` explícito.
    #[serde(default, deserialize_with = "deserialize_some")]
    ids: Option<Option<Vec<String>>>,
}

/// Distingue un campo ausente (`None`) de un `null` explícito (`Some(None)`).
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

/// `POST /api/notifications/mark-read` — marca como leídas de forma idempotente.
///
/// Responde `200` con `{"marked": <n>}`, el número de filas efectivamente
/// marcadas. Los ids inexistentes se ignoran. Un `ids` **ausente** marca
/// **todas** las no leídas; un `ids` **nulo (`null`) o explícitamente vacío**
/// responde `400`, porque es una petición de marcar *nada*.
async fn mark_read(
    State(state): State<AppState>,
    Json(payload): Json<MarkReadRequest>,
) -> Result<Json<Value>, AppError> {
    let marked = match payload.ids {
        None => NotificationsRepo::mark_all_read(&state.db).await?,
        Some(None) => return Err(AppError::BadRequest("ids must not be null".to_string())),
        Some(Some(ids)) if ids.is_empty() => {
            return Err(AppError::BadRequest("ids must not be empty".to_string()))
        }
        Some(Some(ids)) => NotificationsRepo::mark_read(&state.db, &ids).await?,
    };

    Ok(Json(json!({ "marked": marked })))
}
