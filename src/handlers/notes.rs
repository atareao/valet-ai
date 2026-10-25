//! Handlers de notas (`/api/notes` y `/api/notes/{id}`).
//!
//! Cinco endpoints de sesión: el listado con filtro por categoría y tope, el
//! alta, la lectura, la actualización **parcial** y el borrado. La persistencia
//! vive en [`NotesRepo`] y el perfil se resuelve con
//! [`ProfilesRepo::get_or_create`], nunca del cuerpo. Aquí solo se valida la
//! entrada y se traduce el resultado a HTTP: una categoría fuera del conjunto,
//! un `content` vacío o un `limit` fuera de rango se rechazan **antes** de tocar
//! la base de datos.
//!
//! Como `NotesRepo::find_by_id`/`update`/`delete` **no** filtran por perfil, toda
//! operación sobre `{id}` comprueba que la nota pertenece al perfil de la
//! sesión: una nota inexistente **o de otro perfil** responde `404` (nunca
//! `403`), sin revelar su existencia.
//!
//! Ver `openspec/changes/notes-crud/specs/notes-api/spec.md`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::repos::notes::{Note, NotesRepo};
use crate::db::repos::profiles::ProfilesRepo;
use crate::errors::AppError;
use crate::AppState;

/// `limit` por defecto del listado.
const DEFAULT_LIMIT: i64 = 100;

/// `limit` máximo del listado.
const MAX_LIMIT: i64 = 500;

/// Las categorías válidas (el `CHECK` de la tabla todavía admite `todo` por
/// legado, pero la API no lo ofrece y no lo acepta).
const ALLOWED_CATEGORIES: [&str; 3] = ["idea", "journal", "fact"];

/// Categoría por defecto del alta.
const DEFAULT_CATEGORY: &str = "idea";

/// Parámetros de consulta de `GET /api/notes`.
///
/// `limit` viaja como texto para poder rechazar con un `400` propio los valores
/// no numéricos o fuera de rango, en vez de dejar la respuesta a `Query`.
#[derive(Debug, Deserialize)]
pub struct NotesQuery {
    /// Filtro opcional por categoría (∈ `idea`/`journal`/`fact`).
    category: Option<String>,
    /// Tope del listado (por defecto 100, máximo 500).
    limit: Option<String>,
}

/// Cuerpo de `POST /api/notes`.
///
/// `content` es obligatorio y no vacío; `category` toma `idea` por defecto; los
/// `tags` son opcionales.
#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    content: Option<String>,
    category: Option<String>,
    tags: Option<String>,
}

/// Cuerpo de `PUT /api/notes/{id}`.
///
/// Todos los campos son opcionales a propósito: un campo **ausente** deja el
/// valor anterior, un `content` vacío es un `400` con motivo propio y un
/// `tags: ""` **vacía** las etiquetas (la cadena vacía es un valor, no una
/// ausencia).
#[derive(Debug, Deserialize)]
pub struct UpdateNoteRequest {
    content: Option<String>,
    category: Option<String>,
    tags: Option<String>,
}

/// `GET /api/notes` — las notas del perfil, de más reciente a más antigua.
///
/// `category` filtra por categoría y `limit` (por defecto 100, máximo 500)
/// acota el resultado. Una categoría fuera del conjunto o un `limit` mal formado
/// responden `400` con el motivo.
pub async fn list_notes(
    State(state): State<AppState>,
    Query(query): Query<NotesQuery>,
) -> Result<Json<Vec<Note>>, AppError> {
    let limit = parse_limit(query.limit.as_deref())?;
    let category = optional_category(query.category.as_deref())?;

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let mut notes = NotesRepo::list(&state.db, &profile.id, category.as_deref()).await?;

    // El repositorio no acepta `limit`; se recorta el vector sin tocarlo.
    notes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));

    Ok(Json(notes))
}

/// `POST /api/notes` — crea una nota para el perfil de la sesión.
///
/// `content` es obligatorio y no vacío (recortado); `category` toma `idea` por
/// defecto y se valida contra el conjunto cerrado. Una entrada inválida responde
/// `400` sin crear nada.
pub async fn create_note(
    State(state): State<AppState>,
    Json(payload): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<Note>), AppError> {
    let content = payload
        .content
        .as_deref()
        .map(require_content)
        .transpose()?
        .ok_or_else(|| AppError::BadRequest("content is required and must not be empty".into()))?;
    let category = resolve_category(payload.category.as_deref())?;
    let tags = payload.tags;

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let note = Note {
        id: Uuid::new_v4().to_string(),
        profile_id: profile.id,
        content,
        category,
        tags,
        created_at: now.clone(),
        updated_at: now,
    };

    NotesRepo::create(&state.db, &note).await?;

    Ok((StatusCode::CREATED, Json(note)))
}

/// `GET /api/notes/{id}` — la nota del perfil.
///
/// `200` con la nota, o `404` si no existe o pertenece a otro perfil.
pub async fn get_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Note>, AppError> {
    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let note = NotesRepo::find_by_id(&state.db, &id)
        .await?
        .filter(|note| note.profile_id == profile.id)
        .ok_or_else(|| AppError::NotFound(format!("Note {id} not found")))?;

    Ok(Json(note))
}

/// `PUT /api/notes/{id}` — actualización **parcial** de la nota del perfil.
///
/// Un campo ausente deja el valor anterior; `tags: ""` vacía las etiquetas. Un
/// cuerpo sin ningún campo, un `content` vacío o una `category` fuera del
/// conjunto responden `400` con el motivo; una nota inexistente o de otro perfil
/// responde `404`. En ambos casos la nota queda intacta.
pub async fn update_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateNoteRequest>,
) -> Result<Json<Note>, AppError> {
    let content = payload
        .content
        .as_deref()
        .map(require_content)
        .transpose()?;
    let category = payload
        .category
        .as_deref()
        .map(|category| resolve_category(Some(category)))
        .transpose()?;
    let tags = payload.tags.as_deref();

    if content.is_none() && category.is_none() && tags.is_none() {
        return Err(AppError::BadRequest(
            "at least one of content, category or tags is required".into(),
        ));
    }

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    match NotesRepo::find_by_id(&state.db, &id).await? {
        Some(note) if note.profile_id == profile.id => {}
        _ => return Err(AppError::NotFound(format!("Note {id} not found"))),
    }

    NotesRepo::update(
        &state.db,
        &id,
        content.as_deref(),
        category.as_deref(),
        tags,
    )
    .await?;

    let updated = NotesRepo::find_by_id(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note {id} not found after update")))?;

    Ok(Json(updated))
}

/// `DELETE /api/notes/{id}` — borra la nota del perfil.
///
/// `204` sin cuerpo si la borró; `404` si no existe o pertenece a otro perfil.
pub async fn delete_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    match NotesRepo::find_by_id(&state.db, &id).await? {
        Some(note) if note.profile_id == profile.id => {}
        _ => return Err(AppError::NotFound(format!("Note {id} not found"))),
    }

    NotesRepo::delete(&state.db, &id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Interpreta el `limit`: por defecto 100, y `400` si no es numérico o se sale
/// de `[1, 500]`.
fn parse_limit(raw: Option<&str>) -> Result<i64, AppError> {
    match raw {
        None => Ok(DEFAULT_LIMIT),
        Some(raw) => raw
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|value| (1..=MAX_LIMIT).contains(value))
            .ok_or_else(|| {
                AppError::BadRequest(format!("limit must be an integer in [1, {MAX_LIMIT}]"))
            }),
    }
}

/// Valida una categoría opcional de la consulta: ausente es `None`, y fuera del
/// conjunto cerrado es un `400` con el motivo.
fn optional_category(raw: Option<&str>) -> Result<Option<String>, AppError> {
    match raw {
        None => Ok(None),
        Some(category) => resolve_category(Some(category)).map(Some),
    }
}

/// Valida una categoría: ausente toma `idea`, y una categoría fuera del conjunto
/// cerrado es un `400`.
fn resolve_category(raw: Option<&str>) -> Result<String, AppError> {
    match raw {
        None => Ok(DEFAULT_CATEGORY.to_string()),
        Some(category) if ALLOWED_CATEGORIES.contains(&category) => Ok(category.to_string()),
        Some(category) => Err(AppError::BadRequest(format!(
            "category must be one of idea, journal, fact; got '{category}'"
        ))),
    }
}

/// El contenido de una nota: obligatorio y no vacío tras recortar los espacios.
fn require_content(raw: &str) -> Result<String, AppError> {
    let content = raw.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest(
            "content is required and must not be empty".into(),
        ));
    }
    Ok(content.to_string())
}
