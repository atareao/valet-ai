//! Handlers de la bitácora (`/api/journal`).
//!
//! Cuatro endpoints de sesión: el listado por rango, la lectura de un día, el
//! alta o reemplazo del día y su borrado. La persistencia vive en
//! [`JournalRepo`] —la clave pública es `(profile_id, date)`— y el perfil se
//! resuelve con [`ProfilesRepo::get_or_create`], como el resto de la API de
//! sesión. Aquí solo se valida la entrada y se traduce el resultado a HTTP: una
//! fecha mal formada, un `content` vacío o un `mood` fuera del conjunto cerrado
//! se rechazan **antes** de tocar la base de datos.
//!
//! Ver `openspec/changes/journal-entries/specs/journal-api/spec.md`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::db::repos::journal::JournalRepo;
use crate::db::repos::profiles::ProfilesRepo;
use crate::errors::AppError;
use crate::models::journal::{JournalEntry, Mood};
use crate::AppState;

/// `limit` por defecto del listado.
const DEFAULT_LIMIT: i64 = 50;

/// `limit` máximo del listado.
const MAX_LIMIT: i64 = 200;

/// Extremos del rango cuando la consulta no los trae: todo el diario.
///
/// Son fechas ISO válidas que quedan fuera de cualquier fecha real, de modo que
/// la comparación lexicográfica del repositorio (que para `YYYY-MM-DD` es
/// también la cronológica) incluye todas las entradas.
const MIN_DATE: &str = "0000-01-01";
const MAX_DATE: &str = "9999-12-31";

/// Parámetros de consulta de `GET /api/journal`.
///
/// `limit` viaja como texto para poder rechazar con un `400` propio los valores
/// no numéricos o fuera de rango, en vez de dejar la respuesta a `Query`.
#[derive(Debug, Deserialize)]
pub struct JournalQuery {
    /// Extremo inferior del rango, inclusivo (fecha ISO `YYYY-MM-DD`).
    from: Option<String>,
    /// Extremo superior del rango, inclusivo (fecha ISO `YYYY-MM-DD`).
    to: Option<String>,
    limit: Option<String>,
}

/// Cuerpo de `PUT /api/journal/{date}`.
///
/// Todos los campos son opcionales a propósito: un `content` ausente es un `400`
/// con motivo propio (no la respuesta de serde) y un `mood` se valida contra el
/// conjunto cerrado antes de escribir nada.
#[derive(Debug, Deserialize)]
pub struct UpsertJournalRequest {
    content: Option<String>,
    /// El ánimo tal cual llega, para validarlo a mano contra [`Mood`].
    mood: Option<Value>,
    #[serde(default)]
    achievements: Option<Vec<String>>,
    #[serde(default)]
    struggles: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

/// `GET /api/journal` — las entradas del rango, de más reciente a más antigua.
///
/// `from` y `to` son opcionales e inclusivos; sin ellos el rango es todo el
/// diario. `limit` (por defecto 50, máximo 200) acota el resultado. Una fecha o
/// un `limit` mal formados responden `400` con el motivo.
pub async fn list_entries(
    State(state): State<AppState>,
    Query(query): Query<JournalQuery>,
) -> Result<Json<Vec<JournalEntry>>, AppError> {
    let limit = parse_limit(query.limit.as_deref())?;
    let from =
        optional_date(query.from.as_deref(), "from")?.unwrap_or_else(|| MIN_DATE.to_string());
    let to = optional_date(query.to.as_deref(), "to")?.unwrap_or_else(|| MAX_DATE.to_string());

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let entries = JournalRepo::list(&state.db, &profile.id, &from, &to, limit).await?;

    Ok(Json(entries))
}

/// `GET /api/journal/{date}` — la entrada de ese día.
///
/// `200` con la entrada, o `404` si ese día no tiene entrada. Una `date` mal
/// formada responde `400` con el motivo.
pub async fn get_entry(
    State(state): State<AppState>,
    Path(date): Path<String>,
) -> Result<Json<JournalEntry>, AppError> {
    let date = require_date(&date)?;
    let profile = ProfilesRepo::get_or_create(&state.db).await?;

    JournalRepo::get(&state.db, &profile.id, &date)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("no journal entry for {date}")))
}

/// `PUT /api/journal/{date}` — crea o **reemplaza** la entrada del día.
///
/// El reemplazo es un upsert por `(profile_id, date)`: nunca hay dos entradas
/// del mismo día. `content` es obligatorio, y `mood`, `achievements`,
/// `struggles` y `tags` son opcionales. Un `content` ausente o vacío, una `date`
/// mal formada o un `mood` fuera del conjunto responden `400` sin escribir nada.
pub async fn upsert_entry(
    State(state): State<AppState>,
    Path(date): Path<String>,
    Json(payload): Json<UpsertJournalRequest>,
) -> Result<Json<JournalEntry>, AppError> {
    let date = require_date(&date)?;
    let content = require_content(payload.content.as_deref())?;
    let mood = parse_mood(payload.mood)?;

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let entry = JournalEntry {
        id: Uuid::new_v4().to_string(),
        profile_id: profile.id,
        date,
        mood,
        content,
        achievements: payload.achievements.unwrap_or_default(),
        struggles: payload.struggles.unwrap_or_default(),
        tags: payload.tags.unwrap_or_default(),
        // La base de datos pone las marcas de tiempo; el repositorio devuelve la
        // fila releída, así que estos dos valores no llegan a persistirse.
        created_at: String::new(),
        updated_at: String::new(),
    };

    let stored = JournalRepo::upsert(&state.db, &entry).await?;

    Ok(Json(stored))
}

/// `DELETE /api/journal/{date}` — borra la entrada del día.
///
/// `204` sin cuerpo si la borró; `404` si ese día no tenía entrada.
pub async fn delete_entry(
    State(state): State<AppState>,
    Path(date): Path<String>,
) -> Result<StatusCode, AppError> {
    let date = require_date(&date)?;
    let profile = ProfilesRepo::get_or_create(&state.db).await?;

    if JournalRepo::delete(&state.db, &profile.id, &date).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("no journal entry for {date}")))
    }
}

/// Interpreta el `limit`: por defecto 50, y `400` si no es numérico o se sale de
/// `[1, 200]`.
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

/// Una fecha opcional de la consulta: ausente es `None`, y mal formada es un
/// `400` con el nombre del parámetro en el motivo.
fn optional_date(raw: Option<&str>, field: &str) -> Result<Option<String>, AppError> {
    match raw {
        None => Ok(None),
        Some(raw) => require_date(raw).map(Some).map_err(|error| match error {
            AppError::BadRequest(message) => AppError::BadRequest(format!("{field}: {message}")),
            other => other,
        }),
    }
}

/// Valida una fecha `YYYY-MM-DD` de forma **estricta**: diez caracteres, los
/// guiones en su sitio y un día que exista de verdad.
///
/// La comprobación de forma es a mano a propósito: `%m`/`%d` de `chrono`
/// aceptan uno o dos dígitos, así que `2026-1-1` pasaría el parseo y luego
/// rompería el orden lexicográfico —que para esta tabla *es* el orden
/// cronológico—.
fn require_date(raw: &str) -> Result<String, AppError> {
    let bytes = raw.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    let exists = shaped && chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_ok();

    if !exists {
        return Err(AppError::BadRequest(format!(
            "date must be an ISO date in YYYY-MM-DD format; got '{raw}'"
        )));
    }

    Ok(raw.to_string())
}

/// El relato del día: obligatorio y no vacío tras recortar los espacios.
fn require_content(raw: Option<&str>) -> Result<String, AppError> {
    raw.map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("content must not be empty".to_string()))
}

/// El ánimo, si viene: ausente o `null` es «sin ánimo», y cualquier otra cosa
/// que no sea uno de los cinco valores del conjunto cerrado es un `400`.
fn parse_mood(raw: Option<Value>) -> Result<Option<Mood>, AppError> {
    match raw {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value::<Mood>(value.clone())
            .map(Some)
            .map_err(|_| {
                AppError::BadRequest(format!(
                    "mood must be one of great, good, neutral, bad or terrible; got {value}"
                ))
            }),
    }
}
