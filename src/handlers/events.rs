use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::repos::events::EventsRepo;
use crate::db::repos::profiles::ProfilesRepo;
use crate::errors::AppError;
use crate::AppState;

/// Normalize an RFC 3339 instant to canonical UTC (`...Z`).
///
/// Returns `422 Unprocessable Entity` when the value is not RFC 3339.
fn normalize_rfc3339(raw: &str, field: &str) -> Result<String, AppError> {
    let dt = DateTime::parse_from_rfc3339(raw)
        .map_err(|_| AppError::UnprocessableEntity(format!("Invalid {field}: must be RFC 3339")))?;
    Ok(dt
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub scope: Option<String>,
    pub category: Option<String>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub reminder_minutes_before: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub category: Option<String>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub reminder_minutes_before: Option<i32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

pub async fn list_events(
    State(state): State<AppState>,
    Query(range): Query<DateRange>,
) -> Result<Json<Vec<crate::db::repos::events::Event>>, AppError> {
    let start_raw = range.start.filter(|s| !s.is_empty()).ok_or_else(|| {
        AppError::UnprocessableEntity("Missing required query parameter: start".into())
    })?;
    let end_raw = range.end.filter(|s| !s.is_empty()).ok_or_else(|| {
        AppError::UnprocessableEntity("Missing required query parameter: end".into())
    })?;

    let start =
        crate::db::repos::events::parse_range_bound(&start_raw, false).ok_or_else(|| {
            AppError::UnprocessableEntity(format!("Invalid start bound: {start_raw}"))
        })?;
    let end = crate::db::repos::events::parse_range_bound(&end_raw, true)
        .ok_or_else(|| AppError::UnprocessableEntity(format!("Invalid end bound: {end_raw}")))?;

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let events = EventsRepo::list_by_date_range(&state.db, &profile.id, start, end).await?;
    Ok(Json(events))
}

pub async fn create_event(
    State(state): State<AppState>,
    Json(payload): Json<CreateEventRequest>,
) -> Result<(StatusCode, Json<crate::db::repos::events::Event>), AppError> {
    // Normalize both instants to canonical UTC and reject non-RFC 3339 input.
    let start_time = normalize_rfc3339(&payload.start_time, "start_time")?;
    let end_time = normalize_rfc3339(&payload.end_time, "end_time")?;

    let profile = ProfilesRepo::get_or_create(&state.db).await?;
    let now = chrono::Utc::now().to_rfc3339();
    let event = crate::db::repos::events::Event {
        id: Uuid::new_v4().to_string(),
        profile_id: profile.id,
        title: payload.title,
        description: payload.description,
        start_time,
        end_time,
        location: payload.location,
        scope: payload.scope.unwrap_or_else(|| "personal".to_string()),
        category: payload.category.unwrap_or_else(|| "default".to_string()),
        all_day: payload.all_day.unwrap_or(false),
        rrule: payload.rrule,
        reminder_minutes_before: payload.reminder_minutes_before,
        created_at: now.clone(),
        updated_at: now,
    };
    EventsRepo::create(&state.db, &event).await?;
    Ok((StatusCode::CREATED, Json(event)))
}

pub async fn update_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateEventRequest>,
) -> Result<Json<crate::db::repos::events::Event>, AppError> {
    EventsRepo::find_by_id(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event {} not found", id)))?;

    // Normalize present instants to canonical UTC; reject non-RFC 3339 input.
    let start_time = payload
        .start_time
        .as_deref()
        .map(|value| normalize_rfc3339(value, "start_time"))
        .transpose()?;
    let end_time = payload
        .end_time
        .as_deref()
        .map(|value| normalize_rfc3339(value, "end_time"))
        .transpose()?;

    EventsRepo::update(
        &state.db,
        &id,
        payload.title.as_deref(),
        payload.description.as_deref(),
        payload.location.as_deref(),
        payload.category.as_deref(),
        payload.all_day,
        payload.rrule.as_deref(),
        payload.reminder_minutes_before,
        start_time.as_deref(),
        end_time.as_deref(),
    )
    .await?;

    let updated = EventsRepo::find_by_id(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event {} not found after update", id)))?;
    Ok(Json(updated))
}

pub async fn delete_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if EventsRepo::delete(&state.db, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("Event {} not found", id)))
    }
}
