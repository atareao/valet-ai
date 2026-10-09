use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Row, SqlitePool};

pub struct UnifiedSearchTool {
    db: SqlitePool,
}

impl UnifiedSearchTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    async fn search_table(
        &self,
        fts_table: &str,
        query: &str,
        source: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        // For multi-column FTS tables (events: title, description), specify column 0.
        // For single-column tables, column 0 is the only column.
        let sql = format!(
            "SELECT {} AS source, rank, snippet({}, 0, '<b>', '</b>', '...', 64) AS snippet \
             FROM {} WHERE {} MATCH ?1 ORDER BY rank LIMIT ?2",
            quote(source),
            fts_table,
            fts_table,
            fts_table,
        );
        // SAFETY: `fts_table`/`source` are hard-coded by the caller (`search`),
        // and `quote` escapes the source literal. `query`/`limit` are bound below.
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(query)
            .bind(limit as i64)
            .fetch_all(&self.db)
            .await
            .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for row in rows {
            results.push(serde_json::json!({
                "source": row.try_get::<String, _>(0).map_err(|e| e.to_string())?,
                "rank": row.try_get::<f64, _>(1).map_err(|e| e.to_string())?,
                "snippet": row.try_get::<String, _>(2).map_err(|e| e.to_string())?,
            }));
        }
        Ok(results)
    }

    pub async fn search(
        &self,
        query: &str,
        dimensions: Option<&str>,
        limit: usize,
    ) -> Result<ToolResult, ToolError> {
        let tables: Vec<(&str, &str)> = match dimensions {
            Some("messages") => vec![("messages_fts", "message")],
            Some("notes") => vec![("notes_fts", "note")],
            Some("events") => vec![("events_fts", "event")],
            Some("tasks") => vec![("tasks_fts", "task")],
            _ => vec![
                ("messages_fts", "message"),
                ("notes_fts", "note"),
                ("events_fts", "event"),
                ("tasks_fts", "task"),
            ],
        };

        let mut all_results = Vec::new();
        for (table, source) in tables {
            if let Ok(mut results) = self.search_table(table, query, source, limit).await {
                all_results.append(&mut results);
            }
        }

        // Sort by rank (lower is better) and take top N
        all_results.sort_by(|a, b| {
            let ra = a["rank"].as_f64().unwrap_or(0.0);
            let rb = b["rank"].as_f64().unwrap_or(0.0);
            ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
        });
        all_results.truncate(limit);

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&all_results).unwrap_or_default(),
            message: Some(format!("Found {} results", all_results.len())),
        })
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[async_trait]
impl Tool for UnifiedSearchTool {
    fn name(&self) -> &'static str {
        "unified_search"
    }

    fn description(&self) -> &'static str {
        "Buscar en todas las dimensiones (mensajes, notas, eventos y tareas) a partir de una consulta en lenguaje natural o palabras clave"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Texto a buscar" },
                "dimensions": {
                    "type": "string",
                    "enum": ["messages", "notes", "events", "tasks"],
                    "description": "Limitar a una dimensión específica"
                },
                "limit": { "type": "integer", "description": "Máximo de resultados (default: 10)" }
            },
            "required": ["query"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.is_empty() {
            return Err(ToolError::InvalidArguments("query is required".into()));
        }
        let dimensions = args.get("dimensions").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        self.search(query, dimensions, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Result<UnifiedSearchTool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        // Create FTS triggers
        crate::db::fts::create_fts_triggers(&pool).await.unwrap();
        // Seed a profile
        sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES ('p1', 'Test', '{}')")
            .execute(&pool)
            .await?;
        // Insert a message (FTS trigger will index it)
        sqlx::query(
            "INSERT INTO messages (id, role, content) \
             VALUES ('m1', 'user', 'prueba de búsqueda unificada')",
        )
        .execute(&pool)
        .await?;
        // Insert a note
        sqlx::query(
            "INSERT INTO notes (id, profile_id, content, category) \
             VALUES ('n1', 'p1', 'nota de prueba para búsqueda', 'idea')",
        )
        .execute(&pool)
        .await?;
        // Insert an event
        sqlx::query(
            "INSERT INTO events (id, profile_id, title, description, start_time, end_time) \
             VALUES ('e1', 'p1', 'Evento de prueba', 'descripción del evento', \
             '2025-01-01T10:00:00Z', '2025-01-01T11:00:00Z')",
        )
        .execute(&pool)
        .await?;
        // Insert a task
        sqlx::query(
            "INSERT INTO tasks (id, profile_id, content) \
             VALUES ('t1', 'p1', 'tarea de prueba para buscar')",
        )
        .execute(&pool)
        .await?;

        Ok(UnifiedSearchTool::new(pool))
    }

    #[tokio::test]
    async fn test_search_rejects_empty_query() -> Result<(), Box<dyn std::error::Error>> {
        let tool = setup().await?;
        let result = tool.execute(serde_json::json!({"query": ""})).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_search_finds_results_across_dimensions() -> Result<(), Box<dyn std::error::Error>>
    {
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "prueba", "limit": 10}))
            .await
            .unwrap();
        assert!(result.success);
        let results = result.data.as_array().unwrap();
        assert!(!results.is_empty(), "Should find at least one result");
        Ok(())
    }

    #[tokio::test]
    async fn test_search_results_expose_rank_and_are_sorted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Given several matches with different relevance
        // When unified_search runs
        // Then every result exposes a numeric `rank`
        // And the sequence of `rank` is ascending (lower = more relevant)
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "prueba", "limit": 10}))
            .await
            .unwrap();
        assert!(result.success);
        let results = result.data.as_array().unwrap();
        assert!(!results.is_empty(), "Should find at least one result");

        let ranks: Vec<f64> = results
            .iter()
            .map(|r| {
                r["rank"]
                    .as_f64()
                    .expect("each result must expose a numeric `rank`")
            })
            .collect();

        for pair in ranks.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "results must be sorted ascending by rank, got {ranks:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_search_with_messages_dimension() -> Result<(), Box<dyn std::error::Error>> {
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "búsqueda", "dimensions": "messages", "limit": 5}))
            .await
            .unwrap();
        assert!(result.success);
        Ok(())
    }

    #[tokio::test]
    async fn test_search_with_notes_dimension() -> Result<(), Box<dyn std::error::Error>> {
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "nota", "dimensions": "notes", "limit": 5}))
            .await
            .unwrap();
        assert!(result.success);
        Ok(())
    }

    #[tokio::test]
    async fn test_search_respects_limit() -> Result<(), Box<dyn std::error::Error>> {
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "prueba", "limit": 1}))
            .await
            .unwrap();
        assert!(result.success);
        let results = result.data.as_array().unwrap();
        assert!(results.len() <= 1, "Should respect limit of 1");
        Ok(())
    }

    #[tokio::test]
    async fn test_search_no_match_returns_empty() -> Result<(), Box<dyn std::error::Error>> {
        let tool = setup().await?;
        let result = tool
            .execute(serde_json::json!({"query": "zzzznoexiste", "limit": 10}))
            .await
            .unwrap();
        assert!(result.success);
        let results = result.data.as_array().unwrap();
        assert!(
            results.is_empty(),
            "Should return empty for non-matching query"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: unified_search description in Spanish and it
    // clarifies the query is natural language or keywords.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_unified_search_description_is_spanish() -> Result<(), Box<dyn std::error::Error>>
    {
        let tool = setup().await?;
        let desc = tool.description();
        assert!(
            desc.contains("Buscar"),
            "unified_search description must be in Spanish (must contain 'Buscar'), got: {desc}"
        );
        assert!(
            desc.contains("lenguaje natural") || desc.contains("palabras clave"),
            "unified_search description must clarify the query is in natural language or keywords, got: {desc}"
        );
        Ok(())
    }
}
