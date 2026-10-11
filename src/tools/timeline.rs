//! Herramientas del timeline: consultar, registrar y borrar hechos cronológicos.
//!
//! Las tres herramientas operan sobre la tabla `timeline_events`
//! (ver `src/db/repos/timeline.rs`). El modelo ve primero la consulta de solo
//! lectura (`timeline_get_events`), después la escritura reversible
//! (`timeline_add_event`) y, por último, el borrado (`timeline_delete_event`),
//! que exige aprobación explícita por ser destructivo.
//!
//! Ver `openspec/changes/activity-timeline/specs/tools/timeline/spec.md`.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::db::repos::timeline::TimelineRepo;
use crate::models::timeline::{
    is_valid_category, NewTimelineEvent, DEFAULT_TIMELINE_CATEGORY, TIMELINE_CATEGORIES,
};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Número máximo de hechos devueltos cuando el modelo no indica un `limit`.
const DEFAULT_LIMIT: i64 = 50;

/// Tope duro de hechos devueltos en una sola consulta, aunque el modelo pida
/// más. Acota el consumo de recursos para que el argumento no sea una vía de DoS.
const MAX_LIMIT: i64 = 500;

/// Tope de longitud de un `location` en `timeline_add_event`: uno más largo se
/// descarta (queda a `NULL`) sin fallar la llamada. Espeja el límite del
/// extractor (`TIMELINE_MAX_LOCATION_CHARS` en `src/workers/episodic_memory.rs`).
const MAX_LOCATION_CHARS: usize = 120;

/// `timeline_get_events` — lee los hechos de un rango, filtrables por categoría.
///
/// Todos los parámetros son opcionales: sin rango devuelve los hechos más
/// recientes, sin `category` recorre todas las categorías y sin `limit` se
/// aplica [`DEFAULT_LIMIT`]. El orden lo garantiza el repositorio (más reciente
/// primero).
pub struct TimelineGetEventsTool {
    db: SqlitePool,
}

impl TimelineGetEventsTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for TimelineGetEventsTool {
    fn name(&self) -> &'static str {
        "timeline_get_events"
    }

    fn description(&self) -> &'static str {
        "Devuelve los hechos del timeline dentro de un rango de fechas, opcionalmente acotados \
         por categoría y limitados en número. Los hechos vienen ordenados del más reciente al \
         más antiguo y cada uno lleva su `id`, su `timestamp`, su `category`, su `fact` y su \
         `location` (nulo cuando no se conoce el lugar). Todos los parámetros son opcionales: \
         `start` y `end` son fechas ISO 8601 inclusivas, `category` filtra por una de las \
         categorías del conjunto cerrado y `limit` acota el número de hechos devueltos (50 por \
         defecto)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "start": {
                    "type": "string",
                    "description": "Fecha ISO 8601 inicial (inclusiva); sin ella no se acota por el principio"
                },
                "end": {
                    "type": "string",
                    "description": "Fecha ISO 8601 final (inclusiva); sin ella no se acota por el final"
                },
                "category": {
                    "type": "string",
                    "enum": TIMELINE_CATEGORIES,
                    "description": "Categoría por la que filtrar; si se omite, se devuelven todas las categorías"
                },
                "limit": {
                    "type": "integer",
                    "description": "Número máximo de hechos a devolver (50 por defecto)"
                }
            },
            "required": []
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let start = args.get("start").and_then(|v| v.as_str());
        let end = args.get("end").and_then(|v| v.as_str());
        let category = args.get("category").and_then(|v| v.as_str());

        // Las fechas deben ser RFC 3339 válidas antes de llegar al repositorio:
        // un valor no-ISO rompería el orden y los filtros lexicográficos.
        for (name, value) in [("start", start), ("end", end)] {
            if let Some(value) = value {
                if chrono::DateTime::parse_from_rfc3339(value).is_err() {
                    return Err(ToolError::InvalidArguments(format!(
                        "{name} inválido: '{value}'; debe ser una fecha ISO 8601 (RFC 3339)"
                    )));
                }
            }
        }

        // Una categoría fuera del conjunto cerrado se rechaza antes de tocar la
        // base de datos: el error debe ser de argumentos, no una lista vacía.
        if let Some(category) = category {
            if !is_valid_category(category) {
                return Err(ToolError::InvalidArguments(format!(
                    "category inválida: '{category}'; debe ser una de {TIMELINE_CATEGORIES:?}"
                )));
            }
        }

        // El `limit` se acota aquí, en la herramienta: un valor negativo
        // significaría «sin límite» en SQLite y un valor enorme sería una vía
        // de DoS. El repositorio es una capa de datos fina y compartida.
        let limit = if let Some(raw) = args.get("limit") {
            let requested = raw.as_i64().ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "limit inválido: {raw}; debe ser un entero mayor o igual que 1"
                ))
            })?;
            if requested < 1 {
                return Err(ToolError::InvalidArguments(format!(
                    "limit inválido: {requested}; debe ser un entero mayor o igual que 1"
                )));
            }
            requested.min(MAX_LIMIT)
        } else {
            DEFAULT_LIMIT
        };

        let events = TimelineRepo::list(&self.db, start, end, category, limit).await?;
        let count = events.len();

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&events).unwrap_or_default(),
            message: Some(format!("{count} hecho(s) en el timeline")),
        })
    }
}

/// `timeline_add_event` — registra un hecho nuevo.
///
/// `fact` es obligatorio y no puede estar vacío ni compuesto solo por espacios.
/// `category` es opcional (`lifestyle` por defecto), `timestamp` también
/// (hora actual en UTC si se omite) y `location` es opcional: se recorta y, si
/// queda vacío o pasa de [`MAX_LOCATION_CHARS`], se guarda a `NULL` sin error.
pub struct TimelineAddEventTool {
    db: SqlitePool,
}

impl TimelineAddEventTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for TimelineAddEventTool {
    fn name(&self) -> &'static str {
        "timeline_add_event"
    }

    fn description(&self) -> &'static str {
        "Registra un hecho en el timeline. El argumento `fact` es obligatorio y no puede quedar \
         vacío. `category` es opcional (por defecto `lifestyle`), `timestamp` es opcional: si se \
         omite, se usa la hora actual en UTC, y `location` es opcional: el lugar donde ocurrió el \
         hecho (si falta o pasa de 120 caracteres, se omite). Si el hecho ya estaba registrado \
         ese mismo día, no se duplica y se informa de ello; en caso contrario, devuelve el \
         identificador del hecho creado."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "fact": {
                    "type": "string",
                    "description": "Hecho a registrar (obligatorio, no puede estar vacío)"
                },
                "category": {
                    "type": "string",
                    "enum": TIMELINE_CATEGORIES,
                    "description": "Categoría del hecho (por defecto `lifestyle`)"
                },
                "timestamp": {
                    "type": "string",
                    "description": "Fecha ISO 8601 del hecho; si se omite, se usa la hora actual en UTC"
                },
                "location": {
                    "type": "string",
                    "description": "Lugar donde ocurrió el hecho (opcional; si falta, está en blanco o pasa de 120 caracteres, se omite)"
                }
            },
            "required": ["fact"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let fact = args.get("fact").and_then(|v| v.as_str()).unwrap_or("");
        if fact.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "fact es obligatorio y no puede estar vacío".into(),
            ));
        }

        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_TIMELINE_CATEGORY);
        if !is_valid_category(category) {
            return Err(ToolError::InvalidArguments(format!(
                "category inválida: '{category}'; debe ser una de {TIMELINE_CATEGORIES:?}"
            )));
        }

        let timestamp = if let Some(raw) = args.get("timestamp") {
            let value = raw.as_str().ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "timestamp inválido: {raw}; debe ser una fecha ISO 8601 (RFC 3339)"
                ))
            })?;
            if chrono::DateTime::parse_from_rfc3339(value).is_err() {
                return Err(ToolError::InvalidArguments(format!(
                    "timestamp inválido: '{value}'; debe ser una fecha ISO 8601 (RFC 3339)"
                )));
            }
            value.to_string()
        } else {
            Utc::now().to_rfc3339()
        };

        let location = args
            .get("location")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.chars().count() <= MAX_LOCATION_CHARS)
            .map(str::to_string);

        let event = NewTimelineEvent {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            category: category.to_string(),
            fact: fact.to_string(),
            location,
            source_message_id: None,
        };

        // `insert_many` is `INSERT OR IGNORE`: a duplicate of the unique index
        // (same fact, same day) is a silent no-op that returns `0`.
        let inserted = TimelineRepo::insert_many(&self.db, std::slice::from_ref(&event)).await?;

        if inserted == 0 {
            return Ok(ToolResult {
                success: true,
                data: json!({}),
                message: Some(
                    "El hecho ya estaba registrado ese mismo día; no se ha duplicado".into(),
                ),
            });
        }

        Ok(ToolResult {
            success: true,
            data: json!({ "id": event.id }),
            message: Some("Hecho registrado en el timeline".into()),
        })
    }
}

/// `timeline_delete_event` — borra un hecho por su id.
///
/// Operación destructiva: declara [`Permission::ExplicitApproval`] siempre. Un
/// id inexistente no es un error; se informa de que no existe.
pub struct TimelineDeleteEventTool {
    db: SqlitePool,
}

impl TimelineDeleteEventTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for TimelineDeleteEventTool {
    fn name(&self) -> &'static str {
        "timeline_delete_event"
    }

    fn description(&self) -> &'static str {
        "Borra un hecho del timeline por su identificador. El argumento `id` es obligatorio. Si \
         no existe ningún hecho con ese id, no falla: informa de que no existe."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identificador del hecho a borrar (obligatorio)"
                }
            },
            "required": ["id"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::ExplicitApproval
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "id es obligatorio y no puede estar vacío".into(),
            ));
        }

        let deleted = TimelineRepo::delete(&self.db, id).await?;

        if deleted {
            Ok(ToolResult {
                success: true,
                data: json!({ "id": id, "deleted": true }),
                message: Some(format!("Hecho '{id}' borrado del timeline")),
            })
        } else {
            Ok(ToolResult {
                success: true,
                data: json!({ "id": id, "deleted": false }),
                message: Some(format!("No existe ningún hecho con el id '{id}'")),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_db() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        Ok(pool)
    }

    async fn insert_event(
        pool: &SqlitePool,
        id: &str,
        timestamp: &str,
        category: &str,
        fact: &str,
    ) {
        let event = NewTimelineEvent {
            id: id.to_string(),
            timestamp: timestamp.to_string(),
            category: category.to_string(),
            fact: fact.to_string(),
            source_message_id: None,
            location: None,
        };
        TimelineRepo::insert_many(pool, &[event]).await.unwrap();
    }

    fn timestamps(result: &ToolResult) -> Vec<String> {
        result
            .data
            .as_array()
            .expect("data must be an array of events")
            .iter()
            .map(|e| e["timestamp"].as_str().unwrap().to_string())
            .collect()
    }

    // -----------------------------------------------------------------------
    // 1.8 / tools — timeline_get_events
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_events_range_filters_and_orders_newest_first(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(&db, "a", "2026-01-10T08:00:00+00:00", "work", "old").await;
        insert_event(
            &db,
            "b",
            "2026-01-15T09:00:00+00:00",
            "work",
            "same day early",
        )
        .await;
        insert_event(
            &db,
            "c",
            "2026-01-15T20:00:00+00:00",
            "work",
            "same day late",
        )
        .await;
        insert_event(&db, "d", "2026-01-20T10:00:00+00:00", "work", "future").await;

        let tool = TimelineGetEventsTool::new(db);
        let result = tool
            .execute(json!({
                "start": "2026-01-15T00:00:00+00:00",
                "end": "2026-01-15T23:59:59+00:00"
            }))
            .await?;

        let stamps = timestamps(&result);
        assert_eq!(
            stamps,
            vec![
                "2026-01-15T20:00:00+00:00".to_string(),
                "2026-01-15T09:00:00+00:00".to_string(),
            ],
            "only the events of the day, newest first"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_filters_by_category() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(&db, "a", "2026-01-15T08:00:00+00:00", "sport", "run").await;
        insert_event(&db, "b", "2026-01-15T09:00:00+00:00", "work", "meeting").await;
        insert_event(&db, "c", "2026-01-15T10:00:00+00:00", "sport", "swim").await;

        let tool = TimelineGetEventsTool::new(db);
        let result = tool.execute(json!({ "category": "sport" })).await?;

        assert_eq!(timestamps(&result).len(), 2);
        for event in result.data.as_array().unwrap() {
            assert_eq!(event["category"], "sport");
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_rejects_unknown_category_without_querying(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Empty database: an invalid category must fail as an argument error,
        // not as an empty (but successful) list.
        let db = setup_db().await?;
        let tool = TimelineGetEventsTool::new(db);

        let err = tool
            .execute(json!({ "category": "random" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_limit_takes_newest() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(&db, "a", "2026-01-01T00:00:00+00:00", "work", "e1").await;
        insert_event(&db, "b", "2026-01-02T00:00:00+00:00", "work", "e2").await;
        insert_event(&db, "c", "2026-01-03T00:00:00+00:00", "work", "e3").await;
        insert_event(&db, "d", "2026-01-04T00:00:00+00:00", "work", "e4").await;

        let tool = TimelineGetEventsTool::new(db);
        let result = tool.execute(json!({ "limit": 2 })).await?;

        assert_eq!(
            timestamps(&result),
            vec![
                "2026-01-04T00:00:00+00:00".to_string(),
                "2026-01-03T00:00:00+00:00".to_string(),
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_rejects_non_positive_limit_without_querying(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Empty database: a non-positive limit must fail as an argument error,
        // not silently fall back to SQLite's "no limit".
        let db = setup_db().await?;
        let tool = TimelineGetEventsTool::new(db);

        for limit in [0, -1] {
            let err = tool.execute(json!({ "limit": limit })).await.unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "limit {limit} must be rejected as invalid arguments, got: {err:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_caps_limit_at_max() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(&db, "a", "2026-01-01T00:00:00+00:00", "work", "e1").await;
        insert_event(&db, "b", "2026-01-02T00:00:00+00:00", "work", "e2").await;

        assert_eq!(MAX_LIMIT, 500, "the hard cap must be 500");

        let tool = TimelineGetEventsTool::new(db);
        // A huge limit is accepted but silently capped to MAX_LIMIT.
        let result = tool.execute(json!({ "limit": 99999 })).await?;
        assert!(result.success);
        // Fewer rows than the cap: all of them come back, but the request never
        // reaches the repository with an unbounded value.
        assert_eq!(timestamps(&result).len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_rejects_non_iso_start_and_end_without_querying(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Empty database: a non-ISO date must fail as an argument error before
        // reaching the repository's lexicographic filters and ordering.
        let db = setup_db().await?;
        let tool = TimelineGetEventsTool::new(db);

        for args in [json!({ "start": "ayer" }), json!({ "end": "ayer" })] {
            let err = tool.execute(args.clone()).await.unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidArguments(_)),
                "non-ISO date in {args} must be rejected, got: {err:?}"
            );
        }

        // A valid RFC 3339 range is still accepted.
        let ok = tool
            .execute(json!({
                "start": "2026-01-01T00:00:00+00:00",
                "end": "2026-01-31T23:59:59+00:00"
            }))
            .await?;
        assert!(ok.success);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 1.8 / tools — timeline_add_event
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_add_event_only_fact_uses_now_and_default_category(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let before = Utc::now();
        let result = tool
            .execute(json!({ "fact": "caminé por el parque" }))
            .await?;
        let after = Utc::now();

        assert!(result.success);
        let id = result.data["id"]
            .as_str()
            .expect("must return the created id");

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.id, id);
        assert_eq!(event.category, "lifestyle");
        assert_eq!(event.fact, "caminé por el parque");

        let parsed = chrono::DateTime::parse_from_rfc3339(&event.timestamp)
            .expect("timestamp must be a valid RFC3339 string");
        let parsed = parsed.with_timezone(&Utc);
        assert!(
            parsed >= before - chrono::Duration::seconds(60)
                && parsed <= after + chrono::Duration::seconds(60),
            "timestamp {} must be close to now",
            event.timestamp
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_respects_explicit_timestamp_and_category(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let result = tool
            .execute(json!({
                "fact": "entrené fuerza",
                "category": "sport",
                "timestamp": "2026-02-01T07:30:00+00:00"
            }))
            .await?;
        assert!(result.success);

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].category, "sport");
        assert_eq!(events[0].timestamp, "2026-02-01T07:30:00+00:00");
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_empty_fact_is_invalid_and_inserts_nothing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        for fact in ["", "   ", "\t\n"] {
            let err = tool.execute(json!({ "fact": fact })).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)));
        }

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert!(events.is_empty(), "an invalid fact must not insert a row");
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_rejects_unknown_category() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let err = tool
            .execute(json!({ "fact": "algo", "category": "random" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert!(
            events.is_empty(),
            "an invalid category must not insert a row"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_rejects_non_iso_timestamp_and_inserts_nothing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let err = tool
            .execute(json!({ "fact": "algo", "timestamp": "no-es-una-fecha" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert!(
            events.is_empty(),
            "an invalid timestamp must not insert a row"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_accepts_valid_rfc3339_timestamp(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let result = tool
            .execute(json!({
                "fact": "entrené fuerza",
                "timestamp": "2026-02-01T07:30:00+00:00"
            }))
            .await?;
        assert!(result.success);

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].timestamp, "2026-02-01T07:30:00+00:00");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 1.8 / tools — timeline_delete_event
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_delete_event_removes_existing_row() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(&db, "keep", "2026-01-01T00:00:00+00:00", "work", "keep").await;
        insert_event(&db, "gone", "2026-01-02T00:00:00+00:00", "work", "gone").await;

        let tool = TimelineDeleteEventTool::new(db.clone());
        let result = tool.execute(json!({ "id": "gone" })).await?;
        assert!(result.success);

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "keep");
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_event_missing_id_is_not_an_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = setup_db().await?;
        let tool = TimelineDeleteEventTool::new(db);

        let result = tool.execute(json!({ "id": "nope" })).await?;
        assert!(result.success);
        let message = result.message.unwrap_or_default();
        assert!(
            message.contains("No existe") || message.to_lowercase().contains("no existe"),
            "the message must say the id does not exist, got: {message}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_event_empty_id_is_invalid() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineDeleteEventTool::new(db);

        for id in ["", "   "] {
            let err = tool.execute(json!({ "id": id })).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 1.9 / permisos
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_and_add_permissions_are_no_confirm() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = setup_db().await?;
        let get = TimelineGetEventsTool::new(db.clone());
        let add = TimelineAddEventTool::new(db);

        assert_eq!(get.permission(&json!({})), Permission::NoConfirm);
        assert_eq!(add.permission(&json!({})), Permission::NoConfirm);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_permission_is_explicit_approval() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = setup_db().await?;
        let tool = TimelineDeleteEventTool::new(db);

        assert_eq!(
            tool.permission(&json!({})),
            Permission::ExplicitApproval,
            "must require explicit approval even with no args"
        );
        assert_eq!(
            tool.permission(&json!({ "id": "x" })),
            Permission::ExplicitApproval,
            "must require explicit approval with args too"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Esquema y descripciones
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_three_tools_expose_object_schema_and_spanish_descriptions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let get = TimelineGetEventsTool::new(db.clone());
        let add = TimelineAddEventTool::new(db.clone());
        let del = TimelineDeleteEventTool::new(db);

        for (name, params, description) in [
            (get.name(), get.parameters(), get.description()),
            (add.name(), add.parameters(), add.description()),
            (del.name(), del.parameters(), del.description()),
        ] {
            assert_eq!(params["type"], "object", "{name} schema must be an object");
            assert!(
                !description.is_empty(),
                "{name} must carry a non-empty description"
            );
            assert!(
                description.contains("hecho") || description.contains("timeline"),
                "{name} description must be Spanish and mention the timeline, got: {description}"
            );
        }

        // The mandatory arguments are documented in the descriptions.
        assert!(del.description().contains("obligatorio"));
        assert!(add.description().contains("obligatorio"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 1.10 / tools — el lugar en `timeline_add_event` y `timeline_get_events`
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_add_event_stores_the_location() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let result = tool
            .execute(json!({
                "fact": "Comí pulpo",
                "location": "Pulpería Terra Meiga"
            }))
            .await?;
        assert!(result.success);

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].location.as_deref(),
            Some("Pulpería Terra Meiga"),
            "the event must keep the location it was given"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_trims_the_location() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let result = tool
            .execute(json!({
                "fact": "Paseé en bici",
                "location": "  vía verde  "
            }))
            .await?;
        assert!(result.success);

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].location.as_deref(),
            Some("vía verde"),
            "the location must be trimmed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_blank_location_is_stored_as_null(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        for (fact, location) in [("hecho uno", ""), ("hecho dos", "   ")] {
            let result = tool
                .execute(json!({ "fact": fact, "location": location }))
                .await?;
            assert!(
                result.success,
                "a blank location must not fail the call for {fact:?}"
            );
        }

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 2, "both facts must still be stored");
        for event in &events {
            assert_eq!(
                event.location, None,
                "a blank location must be stored as NULL, got {:?} for {:?}",
                event.location, event.fact
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_discards_an_overlong_location() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let too_long = "x".repeat(121);
        let exactly_max = "y".repeat(120);

        tool.execute(json!({ "fact": "lugar de más", "location": too_long }))
            .await?;
        tool.execute(json!({ "fact": "lugar justo", "location": exactly_max }))
            .await?;

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 2, "both facts must still be stored");

        let over = events
            .iter()
            .find(|e| e.fact == "lugar de más")
            .expect("the overlong-location fact must be stored");
        assert_eq!(
            over.location, None,
            "a location over 120 characters must be discarded"
        );

        let exact = events
            .iter()
            .find(|e| e.fact == "lugar justo")
            .expect("the exact-length-location fact must be stored");
        assert_eq!(
            exact.location.as_deref(),
            Some(exactly_max.as_str()),
            "a location of exactly 120 characters must be kept"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_reports_an_existing_fact_without_inserting(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db.clone());

        let first = tool
            .execute(json!({
                "fact": "Cené mochis",
                "timestamp": "2026-10-11T20:21:00+00:00"
            }))
            .await?;
        assert!(first.success);
        assert!(
            first.data.get("id").is_some(),
            "the first call must return the created id"
        );

        let second = tool
            .execute(json!({
                "fact": "Cené mochis",
                "timestamp": "2026-10-11T21:00:00+00:00"
            }))
            .await?;
        assert!(second.success, "an existing fact must not be an error");
        assert!(
            second.data.get("id").is_none(),
            "an ignored insert must not return a new id, got: {:?}",
            second.data
        );
        let message = second.message.unwrap_or_default();
        assert!(
            message.contains("ya estaba registrado"),
            "the message must say the fact was already recorded, got: {message}"
        );

        let events = TimelineRepo::list(&db, None, None, None, 10).await?;
        assert_eq!(events.len(), 1, "only the first row must exist");
        Ok(())
    }

    #[tokio::test]
    async fn test_get_events_returns_the_location() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        insert_event(
            &db,
            "with",
            "2026-01-15T08:00:00+00:00",
            "sport",
            "Paseé en bici",
        )
        .await;

        let located = NewTimelineEvent {
            id: "located".to_string(),
            timestamp: "2026-01-15T09:00:00+00:00".to_string(),
            category: "lifestyle".to_string(),
            fact: "Comí pulpo".to_string(),
            source_message_id: None,
            location: Some("Pulpería Terra Meiga".to_string()),
        };
        TimelineRepo::insert_many(&db, &[located]).await?;

        let tool = TimelineGetEventsTool::new(db);
        let result = tool.execute(json!({})).await?;
        assert!(result.success);

        let events = result
            .data
            .as_array()
            .expect("data must be an array of events");

        let with = events
            .iter()
            .find(|e| e["id"] == "located")
            .expect("the located event must be returned");
        let without = events
            .iter()
            .find(|e| e["id"] == "with")
            .expect("the unlocated event must be returned");

        assert_eq!(
            with["location"], "Pulpería Terra Meiga",
            "the located event must carry its place"
        );
        assert!(
            without["location"].is_null(),
            "the unlocated event must serialise its place as null, got: {}",
            without["location"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_add_event_schema_documents_location() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = TimelineAddEventTool::new(db);

        let params = tool.parameters();
        let location = &params["properties"]["location"];
        assert!(
            !location.is_null(),
            "the schema must document a 'location' property, got: {params}"
        );
        assert_eq!(
            location["type"], "string",
            "'location' must be declared as a string"
        );
        assert!(
            tool.description().contains("location"),
            "the description must mention 'location', got: {}",
            tool.description()
        );
        Ok(())
    }
}
