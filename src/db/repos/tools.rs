use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::llm::provider::ToolDef;
use crate::models::Tool;

/// Read and reconcile the `tools` table.
///
/// The table is a plain catalogue of the registered tools: the per-tool
/// `enabled` flag was retired together with the toggle endpoint. Selection is
/// now a skill-level concern (`orchestrator::skills`), so this repository has no
/// enablement logic.
pub struct ToolsRepo;

impl ToolsRepo {
    pub async fn list(pool: &SqlitePool) -> Result<Vec<Tool>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, name, description FROM tools ORDER BY name")
            .fetch_all(pool)
            .await?;

        let items: Vec<Tool> = rows
            .iter()
            .map(|row| Tool {
                id: row.get(0),
                name: row.get(1),
                description: row.get(2),
            })
            .collect();

        Ok(items)
    }

    pub async fn find_by_name(pool: &SqlitePool, name: &str) -> Result<Option<Tool>, sqlx::Error> {
        let row = sqlx::query("SELECT id, name, description FROM tools WHERE name = ?1")
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| Tool {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
        }))
    }

    /// Reconcile the `tools` table with the given registry definitions.
    ///
    /// Rows whose `name` is not in `defs` are removed (when `defs` is empty the
    /// whole table is cleared). Each definition is upserted preserving the
    /// existing `id` — nothing else references it, but keeping it stable keeps
    /// the catalogue's identity across reconciliations.
    pub async fn sync_from_registry(
        pool: &SqlitePool,
        defs: &[ToolDef],
    ) -> Result<(), sqlx::Error> {
        if defs.is_empty() {
            sqlx::query("DELETE FROM tools").execute(pool).await?;
        } else {
            let placeholders = (1..=defs.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("DELETE FROM tools WHERE name NOT IN ({placeholders})");
            // SAFETY: `placeholders` is generated from `defs.len()` only; every
            // name is bound below as a query parameter.
            let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
            for def in defs {
                query = query.bind(&def.name);
            }
            query.execute(pool).await?;
        }

        for def in defs {
            // A fresh id is only used when the row does not exist yet; on
            // conflict the existing id is preserved.
            let new_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO tools (id, name, description) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(name) DO UPDATE SET description = excluded.description",
            )
            .bind(new_id)
            .bind(&def.name)
            .bind(&def.description)
            .execute(pool)
            .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Test-side copy of the production tool catalog. Must mirror the 13 tools
    /// returned by `build_tool_registry` in `src/lib.rs`; keep both in sync.
    const REGISTRY_NAMES: &[&str] = &[
        "weather",
        "geocode",
        "reverse_geocode",
        "search_places",
        "web_search",
        "calendar",
        "tasks",
        "reminders",
        "get_current_time",
        "get_current_location",
        "notes",
        "unified_search",
        "render_widget",
    ];

    async fn setup() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        Ok(pool)
    }

    fn defs(names: &[&str]) -> Vec<ToolDef> {
        names
            .iter()
            .map(|name| ToolDef {
                name: (*name).to_string(),
                description: format!("{name} tool"),
                parameters: serde_json::json!({}),
            })
            .collect()
    }

    #[tokio::test]
    async fn test_list_tools() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;
        let tools = ToolsRepo::list(&pool).await?;
        assert_eq!(tools.len(), 13);
        assert!(tools.iter().any(|t| t.name == "weather"));
        assert!(tools.iter().any(|t| t.name == "unified_search"));
        Ok(())
    }

    #[tokio::test]
    async fn test_sync_adds_new_tools() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;
        let tools = ToolsRepo::list(&pool).await?;
        for name in REGISTRY_NAMES {
            assert!(
                tools.iter().any(|t| t.name == *name),
                "missing synced tool {name}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_sync_removes_obsolete_tools() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        // Seed a legacy row that is not part of the registry.
        sqlx::query(
            "INSERT INTO tools (id, name, description) \
             VALUES ('legacy-id', 'geo', 'Geolocalización')",
        )
        .execute(&pool)
        .await?;

        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;

        let tools = ToolsRepo::list(&pool).await?;
        assert!(
            !tools.iter().any(|t| t.name == "geo"),
            "geo must be removed"
        );
        assert_eq!(tools.len(), 13);
        Ok(())
    }

    #[tokio::test]
    async fn test_sync_preserves_id_and_updates_description(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;

        let weather = ToolsRepo::list(&pool)
            .await?
            .into_iter()
            .find(|t| t.name == "weather")
            .unwrap();
        let original_id = weather.id.clone();

        // Re-sync with a new description: the id must survive.
        let mut updated = defs(REGISTRY_NAMES);
        for def in &mut updated {
            if def.name == "weather" {
                def.description = "updated weather description".to_string();
            }
        }
        ToolsRepo::sync_from_registry(&pool, &updated).await?;

        let weather = ToolsRepo::list(&pool)
            .await?
            .into_iter()
            .find(|t| t.name == "weather")
            .unwrap();
        assert_eq!(weather.id, original_id, "id must be preserved");
        assert_eq!(weather.description, "updated weather description");
        Ok(())
    }

    #[tokio::test]
    async fn test_sync_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let defs = defs(REGISTRY_NAMES);
        ToolsRepo::sync_from_registry(&pool, &defs).await?;
        ToolsRepo::sync_from_registry(&pool, &defs).await?;
        let tools = ToolsRepo::list(&pool).await?;
        assert_eq!(tools.len(), 13);
        Ok(())
    }

    #[tokio::test]
    async fn test_sync_empty_registry_clears_table() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;
        ToolsRepo::sync_from_registry(&pool, &[]).await?;
        assert!(ToolsRepo::list(&pool).await?.is_empty());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // The production registry must sync `render_widget` into the table.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sync_from_production_registry_includes_render_widget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state = crate::AppState::new_in_memory_empty().await;
        let tools = ToolsRepo::list(&state.db).await?;

        assert!(
            tools.iter().any(|t| t.name == "render_widget"),
            "the production registry must sync `render_widget` into the tools table"
        );
        Ok(())
    }
}
