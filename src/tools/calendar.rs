use async_trait::async_trait;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::repos::events::{Event, EventsRepo};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

pub struct CalendarTool {
    db: SqlitePool,
}

impl CalendarTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    async fn get_events(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;
        let start = args
            .get("start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing start".into()))?;
        let duration = args
            .get("duration")
            .and_then(|v| v.as_i64())
            .unwrap_or(1440);

        // Compute end = start + duration minutes
        let start_dt = DateTime::parse_from_rfc3339(start)
            .map_err(|_| ToolError::InvalidArguments("Invalid start format".into()))?
            .with_timezone(&Utc);
        let end_dt = start_dt + Duration::minutes(duration);
        let end = end_dt.to_rfc3339_opts(SecondsFormat::Secs, true);

        // Use start directly (no normalization needed since it's ISO 8601)
        let events = EventsRepo::list_by_date_range(&self.db, profile_id, start, &end).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(events).unwrap_or_default(),
            message: None,
        })
    }

    async fn check_availability(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;
        let start = args
            .get("start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing start".into()))?;
        let duration = args
            .get("duration")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing duration".into()))?;

        // Extract date from start ISO 8601 string (first 10 chars: YYYY-MM-DD)
        let date = &start[..10.min(start.len())];

        let slots = EventsRepo::find_free_slots(&self.db, profile_id, date, duration).await?;

        let slots_json: Vec<Value> = slots
            .into_iter()
            .map(|(start, end)| serde_json::json!({"start": start, "end": end}))
            .collect();

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({"slots": slots_json}),
            message: None,
        })
    }

    async fn create_event(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing title".into()))?;
        let start = args
            .get("start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing start".into()))?;
        let duration = args
            .get("duration")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing duration".into()))?;
        let start_dt = DateTime::parse_from_rfc3339(start)
            .map_err(|_| ToolError::InvalidArguments("Invalid start format".into()))?
            .with_timezone(&Utc);
        let end_dt = start_dt + Duration::minutes(duration);
        let end = end_dt.to_rfc3339_opts(SecondsFormat::Secs, true);

        let now = Utc::now().to_rfc3339();
        let event = Event {
            id: Uuid::new_v4().to_string(),
            profile_id: profile_id.to_string(),
            title: title.to_string(),
            description: args
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            start_time: start.to_string(),
            end_time: end.to_string(),
            location: args
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            scope: args
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("shared")
                .to_string(),
            category: args
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string(),
            all_day: args
                .get("all_day")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            rrule: args
                .get("rrule")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            reminder_minutes_before: args
                .get("reminder_minutes_before")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
            created_at: now.clone(),
            updated_at: now,
        };

        EventsRepo::create(&self.db, &event).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&event).unwrap_or_default(),
            message: Some("Event created successfully".into()),
        })
    }

    async fn update_event(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing id".into()))?;

        // Resolve end from duration only (end parameter removed)
        let end_str: Option<String> = if let Some(dur) =
            args.get("duration").and_then(|v| v.as_i64())
        {
            let effective_start = match args.get("start").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    let current = EventsRepo::find_by_id(&self.db, id).await?.ok_or_else(|| {
                        ToolError::ExecutionError(format!("No event found with id: {}", id))
                    })?;
                    current.start_time
                }
            };
            let start_dt = DateTime::parse_from_rfc3339(&effective_start)
                .map_err(|_| ToolError::InvalidArguments("Invalid start format".into()))?
                .with_timezone(&Utc);
            let end_dt = start_dt + Duration::minutes(dur);
            Some(end_dt.to_rfc3339_opts(SecondsFormat::Secs, true))
        } else {
            None // No end change — EventsRepo::update handles None
        };

        if !EventsRepo::update(
            &self.db,
            id,
            args.get("title").and_then(|v| v.as_str()),
            args.get("description").and_then(|v| v.as_str()),
            args.get("location").and_then(|v| v.as_str()),
            args.get("category").and_then(|v| v.as_str()),
            args.get("all_day").and_then(|v| v.as_bool()),
            args.get("rrule").and_then(|v| v.as_str()),
            args.get("reminder_minutes_before")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32),
            args.get("start").and_then(|v| v.as_str()),
            end_str.as_deref(),
        )
        .await?
        {
            return Err(ToolError::ExecutionError(format!(
                "No event found with id: {}",
                id
            )));
        }

        let updated = EventsRepo::find_by_id(&self.db, id).await?.ok_or_else(|| {
            ToolError::ExecutionError(format!("Event not found after update: {}", id))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&updated).unwrap_or_default(),
            message: Some("Event updated successfully".into()),
        })
    }

    async fn delete_event(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing id".into()))?;

        let event = EventsRepo::find_by_id(&self.db, id)
            .await?
            .ok_or_else(|| ToolError::ExecutionError(format!("No event found with id: {}", id)))?;

        EventsRepo::delete(&self.db, id).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&event).unwrap_or_default(),
            message: Some("Event deleted successfully".into()),
        })
    }

    async fn list_by_category(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing category".into()))?;
        let events = EventsRepo::list_by_category(&self.db, profile_id, category).await?;
        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(events).unwrap_or_default(),
            message: None,
        })
    }
}

#[async_trait]
impl Tool for CalendarTool {
    fn name(&self) -> &'static str {
        "calendar"
    }

    fn description(&self) -> &'static str {
        "Agenda y calendario — eventos, citas, reuniones, cumpleaños, disponibilidad y huecos libres"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "get_events",
                        "check_availability",
                        "create_event",
                        "update_event",
                        "delete_event",
                        "list_by_category"
                    ]
                },
                "title": { "type": "string", "description": "Título del evento — obligatorio en create_event" },
                "start": { "type": "string", "description": "ISO 8601. Inicio del evento/rango. Ej: 2026-09-26T21:00:00Z" },
                "duration": { "type": "integer", "description": "Duración en minutos. Ej: 60 = 1h, 1440 = 1 día. Opcional en get_events (default: 1440)" },
                "id": { "type": "string", "description": "ID del evento — obligatorio en update_event y delete_event" },
                "category": { "type": "string", "enum": ["default", "work", "personal", "health", "birthday", "holiday"] },
                "scope": { "type": "string", "enum": ["shared", "personal"] },
                "location": { "type": "string" },
                "description": { "type": "string" },
                "all_day": { "type": "boolean" },
                "rrule": { "type": "string" },
                "reminder_minutes_before": { "type": "integer" }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("delete_event") => Permission::ExplicitApproval,
            Some("create_event") | Some("update_event") => Permission::Notify,
            // get_events, check_availability, list_by_category, ausente o desconocida:
            _ => Permission::NoConfirm,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        match args.get("operation").and_then(|v| v.as_str()).unwrap_or("") {
            "get_events" => self.get_events(args).await,
            "check_availability" => self.check_availability(args).await,
            "create_event" => self.create_event(args).await,
            "update_event" => self.update_event(args).await,
            "delete_event" => self.delete_event(args).await,
            "list_by_category" => self.list_by_category(args).await,
            op => Err(ToolError::InvalidArguments(format!(
                "Unknown operation: {}",
                op
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Result<(SqlitePool, CalendarTool), sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await?;
        let tool = CalendarTool::new(pool.clone());
        Ok((pool, tool))
    }

    #[tokio::test]
    async fn test_parameters_does_not_expose_profile_id() {
        // Profile_id debe inyectarse desde el orquestador, NO desde el LLM
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let params = CalendarTool::parameters(&CalendarTool::new(pool));
        let properties = params["properties"].as_object().unwrap();
        assert!(
            !properties.contains_key("profile_id"),
            "profile_id NO debe estar en el schema expuesto al LLM"
        );
    }

    #[tokio::test]
    async fn test_get_events_empty() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-09-24T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data.as_array().unwrap().len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_without_duration_defaults_to_1440min(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // LLM calls get_events with just start, no duration — should default to 1440min
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-09-27T00:00:00+02:00"
                // no duration — should default to 1440
            }))
            .await;
        assert!(
            result.is_ok(),
            "get_events sin duration debería funcionar con default 1440"
        );
        assert!(result.unwrap().success);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_with_date() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create event
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Test Event",
            "start": "2026-09-24T10:00:00Z",
            "duration": 60,
            "scope": "shared"
        }))
        .await
        .unwrap();

        // Query with start + duration instead of date
        let result = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-09-24T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"].as_str().unwrap(), "Test Event");
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_with_bare_dates() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create event at 10:00-11:00
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Meeting",
            "start": "2026-09-28T10:00:00Z",
            "duration": 60,
            "scope": "shared"
        }))
        .await
        .unwrap();

        // Query with start + duration (was bare dates before)
        let result = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-09-28T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"].as_str().unwrap(), "Meeting");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_event() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Reunión",
            "start": "2026-09-24T10:00:00Z",
            "duration": 60,
            "scope": "shared"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["title"], "Reunión");
        assert_eq!(result.data["scope"], "shared");
        Ok(())
    }

    #[tokio::test]
    async fn test_check_availability() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // First create an event
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Ocupado",
            "start": "2026-09-24T10:00:00Z",
            "duration": 60
        }))
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({
                "operation": "check_availability",
                "profile_id": "profile-1",
                "start": "2026-09-24T00:00:00Z",
                "duration": 30
            }))
            .await
            .unwrap();
        assert!(result.success);
        let slots = result.data["slots"].as_array().unwrap();
        assert!(!slots.is_empty());
        // Should have at least one slot before 10:00 and one after 11:00
        assert!(slots.len() >= 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_operation() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "nonexistent"
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_update_event() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create an event on 2026-10-03
        let created = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Original",
                "start": "2026-10-03T10:00:00Z",
                "duration": 60
            }))
            .await
            .unwrap();
        let event_id = created.data["id"].as_str().unwrap().to_string();

        // Update it — move from 2026-10-03 to 2026-10-10, with 90min duration
        let result = tool
            .execute(serde_json::json!({
                "operation": "update_event",
                "id": event_id,
                "title": "Moved",
                "start": "2026-10-10T12:00:00Z",
                "duration": 90
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], event_id);

        // The old date should now have 0 events (the event was moved)
        let old_date = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-10-03T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        let old_events = old_date.data.as_array().unwrap();
        assert_eq!(
            old_events.len(),
            0,
            "Event should have moved from 2026-10-03"
        );

        // The new date should have 1 event with the updated title
        let new_date = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-10-10T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        let new_events = new_date.data.as_array().unwrap();
        assert_eq!(new_events.len(), 1, "Event should now be on 2026-10-10");
        assert_eq!(new_events[0]["title"], "Moved");

        Ok(())
    }

    #[tokio::test]
    async fn test_missing_required_args() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1"
                // missing title, start, duration
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    // ── delete_event ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_event() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create an event
        let created = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "To Delete",
                "start": "2026-09-24T10:00:00Z",
                "duration": 60
            }))
            .await
            .unwrap();
        let event_id = created.data["id"].as_str().unwrap().to_string();

        // Delete it
        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_event",
                "id": event_id
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], event_id);
        assert_eq!(
            result.message.as_deref(),
            Some("Event deleted successfully")
        );

        // Verify it's gone
        let events = tool
            .execute(serde_json::json!({
                "operation": "get_events",
                "profile_id": "profile-1",
                "start": "2026-09-24T00:00:00Z",
                "duration": 1440
            }))
            .await
            .unwrap();
        assert_eq!(events.data.as_array().unwrap().len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_event_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Deleting a non-existent event should error
        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_event",
                "id": "nonexistent-id"
            }))
            .await;
        assert!(result.is_err());
        Ok(())
    }

    // ── list_by_category ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_category() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;

        // Create work event
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Work Meeting",
            "start": "2026-09-24T10:00:00Z",
            "duration": 60,
            "category": "work"
        }))
        .await
        .unwrap();

        // Create personal event
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Gym",
            "start": "2026-09-24T18:00:00Z",
            "duration": 60,
            "category": "personal"
        }))
        .await
        .unwrap();

        // Create birthday event
        tool.execute(serde_json::json!({
            "operation": "create_event",
            "profile_id": "profile-1",
            "title": "Birthday Party",
            "start": "2026-09-25T20:00:00Z",
            "duration": 180,
            "category": "birthday"
        }))
        .await
        .unwrap();

        // Filter by "work"
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1",
                "category": "work"
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"], "Work Meeting");

        // Filter by "personal"
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1",
                "category": "personal"
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"], "Gym");

        // Filter by "birthday"
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1",
                "category": "birthday"
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"], "Birthday Party");

        // Filter by non-existent category returns empty
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1",
                "category": "health"
            }))
            .await
            .unwrap();
        assert!(result.success);
        let events = result.data.as_array().unwrap();
        assert!(events.is_empty());

        Ok(())
    }

    // ── create_event with all_day ───────────────────────────────────────

    #[tokio::test]
    async fn test_create_event_with_all_day() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "All Day Event",
                "start": "2026-09-24T00:00:00Z",
                "duration": 1439,
                "all_day": true
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["all_day"], true);
        assert_eq!(result.data["title"], "All Day Event");
        Ok(())
    }

    // ── create_event with rrule ─────────────────────────────────────────

    #[tokio::test]
    async fn test_create_event_with_rrule() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Recurring Standup",
                "start": "2026-09-24T09:00:00Z",
                "duration": 30,
                "rrule": "FREQ=DAILY"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["rrule"], "FREQ=DAILY");
        assert_eq!(result.data["title"], "Recurring Standup");
        Ok(())
    }

    // ── create_event with reminder_minutes_before ───────────────────────

    #[tokio::test]
    async fn test_create_event_with_reminder() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "With Reminder",
                "start": "2026-09-24T15:00:00Z",
                "duration": 60,
                "reminder_minutes_before": 15
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["reminder_minutes_before"], 15);
        assert_eq!(result.data["title"], "With Reminder");
        Ok(())
    }

    // ── create_event with specific category ─────────────────────────────

    #[tokio::test]
    async fn test_create_event_with_category() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Health Checkup",
                "start": "2026-09-25T09:00:00Z",
                "duration": 60,
                "category": "health"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["category"], "health");
        assert_eq!(result.data["title"], "Health Checkup");

        // Verify it appears in list_by_category
        let list_result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1",
                "category": "health"
            }))
            .await
            .unwrap();
        let events = list_result.data.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"], "Health Checkup");

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_event_missing_id() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_event"
                // missing id
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_list_by_category_missing_args() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Missing profile_id
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "category": "work"
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));

        // Missing category
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_by_category",
                "profile_id": "profile-1"
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_parameters_includes_new_operations() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let params = CalendarTool::parameters(&CalendarTool::new(pool));
        let op_enum = params["properties"]["operation"]["enum"]
            .as_array()
            .unwrap();
        let ops: Vec<&str> = op_enum.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(ops.contains(&"delete_event"));
        assert!(ops.contains(&"list_by_category"));
    }

    #[tokio::test]
    async fn test_parameters_includes_new_fields() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let params = CalendarTool::parameters(&CalendarTool::new(pool));
        let properties = params["properties"].as_object().unwrap();

        // category field
        let category = properties.get("category").unwrap();
        assert_eq!(category["type"], "string");
        let cat_enum = category["enum"].as_array().unwrap();
        let cats: Vec<&str> = cat_enum.iter().map(|v| v.as_str().unwrap()).collect();
        for expected in &[
            "default", "work", "personal", "health", "birthday", "holiday",
        ] {
            assert!(cats.contains(expected), "missing category: {}", expected);
        }

        // all_day field
        let all_day = properties.get("all_day").unwrap();
        assert_eq!(all_day["type"], "boolean");

        // rrule field
        let rrule = properties.get("rrule").unwrap();
        assert_eq!(rrule["type"], "string");

        // reminder_minutes_before field
        let reminder = properties.get("reminder_minutes_before").unwrap();
        assert_eq!(reminder["type"], "integer");
    }

    #[tokio::test]
    async fn test_create_event_with_duration() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Cena",
                "start": "2026-09-26T21:00:00Z",
                "duration": 120
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["title"], "Cena");
        assert_eq!(result.data["start_time"], "2026-09-26T21:00:00Z");
        assert_eq!(result.data["end_time"], "2026-09-26T23:00:00Z");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_event_end_not_accepted() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // end is no longer accepted in create_event
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Test",
                "start": "2026-09-27T10:00:00Z",
                "end": "2026-09-27T11:00:00Z"
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_create_event_missing_duration() {
        let (_, tool) = setup().await.unwrap();
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Sin duración",
                "start": "2026-09-26T21:00:00Z"
            }))
            .await;
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidArguments(_)) => {}
            _ => panic!("Expected InvalidArguments error"),
        }
    }

    #[tokio::test]
    async fn test_create_event_duration_midnight_boundary() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "create_event",
                "profile_id": "profile-1",
                "title": "Late night",
                "start": "2026-09-26T23:00:00Z",
                "duration": 90
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["end_time"], "2026-09-27T00:30:00Z");
        Ok(())
    }
}
