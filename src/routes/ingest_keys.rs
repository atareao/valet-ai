//! Rutas HTTP de gestión de las claves de ingesta (`/api/ingest-keys`).
//!
//! Tres endpoints, todos **con sesión OIDC** (a diferencia de la ingesta, que se
//! autentica por token): el alta que devuelve el token una única vez, el listado
//! —activas y revocadas— que **nunca** expone un token, y la revocación
//! idempotente. La persistencia vive en [`IngestRepo`]; aquí solo se valida la
//! entrada y se traduce el resultado a HTTP.
//!
//! Ver `openspec/specs/ingest-keys/spec.md`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::db::repos::ingest_keys::{generate_token, IngestKey, IngestRepo};
use crate::errors::AppError;
use crate::AppState;

/// Longitud máxima de `name` en bytes, tras `trim`.
const MAX_NAME_BYTES: usize = 64;

/// Registra las tres rutas de `/api/ingest-keys`.
///
/// Ninguna está exenta de sesión: caen bajo el middleware de sesión OIDC como el
/// resto de rutas de gestión.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/ingest-keys", get(list).post(create))
        .route("/api/ingest-keys/{id}", delete(revoke))
}

/// Cuerpo de `POST /api/ingest-keys`.
#[derive(Debug, Deserialize)]
pub struct CreateIngestKeyRequest {
    /// Nombre de la clave. Obligatorio, no vacío tras `trim` y `≤ 64` bytes.
    pub name: Option<String>,
}

/// Respuesta del alta: **única** vez que viaja el token en claro.
#[derive(Debug, Serialize)]
struct CreatedIngestKey {
    id: String,
    name: String,
    prefix: String,
    /// Token completo. Solo se devuelve aquí; nunca se persiste ni se relee.
    token: String,
}

/// `POST /api/ingest-keys` — crea una clave y devuelve su token una vez.
///
/// `201` con `{"id", "name", "prefix", "token"}`. Un `name` ausente, en blanco o
/// de más de 64 bytes responde `400`; un nombre duplicado responde `409`. El
/// token se genera aquí (ver [`generate_token`]); solo su hash y su prefijo se
/// persisten.
async fn create(
    State(state): State<AppState>,
    Json(payload): Json<CreateIngestKeyRequest>,
) -> Result<(StatusCode, Json<CreatedIngestKey>), AppError> {
    let name = validate_name(payload.name)?;
    let token = generate_token();

    let key = IngestRepo::create(&state.db, &name, &token)
        .await
        .map_err(map_create_error)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedIngestKey {
            id: key.id,
            name: key.name,
            prefix: key.prefix,
            token,
        }),
    ))
}

/// Valida y normaliza el `name`: obligatorio, no vacío tras `trim` y `≤ 64` bytes.
fn validate_name(name: Option<String>) -> Result<String, AppError> {
    let name = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("name is required".to_string()))?;
    if name.len() > MAX_NAME_BYTES {
        return Err(AppError::BadRequest(format!(
            "name must be at most {MAX_NAME_BYTES} bytes"
        )));
    }
    Ok(name)
}

/// Traduce el fallo del alta a HTTP: una violación de `UNIQUE` (nombre o hash ya
/// usados) es un `409`; cualquier otro error, un `500`.
fn map_create_error(error: sqlx::Error) -> AppError {
    if let Some(db) = error.as_database_error() {
        if db.is_unique_violation() {
            return AppError::Conflict("an ingest key with that name already exists".to_string());
        }
    }
    AppError::Internal(error.to_string())
}

/// Item de una clave tal como viaja en el listado.
///
/// Deliberadamente **no** incluye `key_hash` ni un token: [`IngestKey`] no guarda
/// el token, y este DTO tampoco filtra el hash.
#[derive(Debug, Serialize)]
struct IngestKeyItem {
    id: String,
    name: String,
    prefix: String,
    created_at: String,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
}

impl From<IngestKey> for IngestKeyItem {
    fn from(key: IngestKey) -> Self {
        Self {
            id: key.id,
            name: key.name,
            prefix: key.prefix,
            created_at: key.created_at,
            last_used_at: key.last_used_at,
            revoked_at: key.revoked_at,
        }
    }
}

/// `GET /api/ingest-keys` — lista las claves, activas y revocadas, sin token.
///
/// `200` con un arreglo de entradas `{id, name, prefix, created_at,
/// last_used_at, revoked_at}`. Ninguna entrada lleva el token.
async fn list(State(state): State<AppState>) -> Result<Json<Vec<IngestKeyItem>>, AppError> {
    let keys = IngestRepo::list_all(&state.db).await?;
    Ok(Json(keys.into_iter().map(IngestKeyItem::from).collect()))
}

/// `DELETE /api/ingest-keys/{id}` — revoca la clave de forma idempotente.
///
/// Fija `revoked_at` **sin** borrar la fila y responde `204` siempre, exista o no
/// el `id`. Repetirla no cambia el `revoked_at` ya fijado.
async fn revoke(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    IngestRepo::revoke(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
