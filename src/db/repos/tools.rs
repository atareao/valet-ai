use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::llm::provider::ToolDef;
use crate::models::Tool;

pub struct ToolsRepo;

impl ToolsRepo {
    pub async fn list(pool: &SqlitePool) -> Result<Vec<Tool>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, name, description, enabled FROM tools ORDER BY name")
            .fetch_all(pool)
            .await?;

        let items: Vec<Tool> = rows
            .iter()
            .map(|row| Tool {
                id: row.get(0),
                name: row.get(1),
                description: row.get(2),
                enabled: row.get::<bool, _>(3),
            })
            .collect();

        Ok(items)
    }

    pub async fn find_by_name(pool: &SqlitePool, name: &str) -> Result<Option<Tool>, sqlx::Error> {
        let row = sqlx::query("SELECT id, name, description, enabled FROM tools WHERE name = ?1")
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| Tool {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
            enabled: r.get::<bool, _>(3),
        }))
    }

    pub async fn toggle_enabled(pool: &SqlitePool, id: &str) -> Result<Option<Tool>, sqlx::Error> {
        sqlx::query(
            "UPDATE tools SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END WHERE id = ?1",
        )
        .bind(id)
        .execute(pool)
        .await?;

        let row = sqlx::query("SELECT id, name, description, enabled FROM tools WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| Tool {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
            enabled: r.get::<bool, _>(3),
        }))
    }

    /// Reconcile the `tools` table with the given registry definitions.
    ///
    /// Rows whose `name` is not in `defs` are removed (when `defs` is empty the
    /// whole table is cleared). Each definition is upserted preserving the
    /// existing `id` and `enabled` flag — UI toggles reference the `id`, so it
    /// must survive reconciliations.
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
            // conflict the existing id (and enabled flag) is preserved.
            let new_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO tools (id, name, description, enabled) VALUES (?1, ?2, ?3, 1) \
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

    /// Names of tools currently disabled in the database.
    pub async fn disabled_names(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT name FROM tools WHERE enabled = 0")
            .fetch_all(pool)
            .await
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
            "INSERT INTO tools (id, name, description, enabled) \
             VALUES ('legacy-id', 'geo', 'Geolocalización', 1)",
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
    async fn test_sync_preserves_enabled_and_id() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;

        let weather = ToolsRepo::list(&pool)
            .await?
            .into_iter()
            .find(|t| t.name == "weather")
            .unwrap();
        let original_id = weather.id.clone();
        let toggled = ToolsRepo::toggle_enabled(&pool, &weather.id)
            .await?
            .unwrap();
        assert!(!toggled.enabled);

        // Re-sync with a new description: enabled=0 and the id must survive.
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
        assert!(!weather.enabled, "enabled flag must be preserved");
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

    #[tokio::test]
    async fn test_disabled_names() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;
        let weather = ToolsRepo::list(&pool)
            .await?
            .into_iter()
            .find(|t| t.name == "weather")
            .unwrap();
        ToolsRepo::toggle_enabled(&pool, &weather.id).await?;

        let disabled = ToolsRepo::disabled_names(&pool).await?;
        assert_eq!(disabled, vec!["weather".to_string()]);
        Ok(())
    }

    #[tokio::test]
    async fn test_toggle_enabled() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        ToolsRepo::sync_from_registry(&pool, &defs(REGISTRY_NAMES)).await?;
        let tools = ToolsRepo::list(&pool).await?;
        let tool = tools.into_iter().find(|t| t.name == "weather").unwrap();
        assert!(tool.enabled);

        let toggled = ToolsRepo::toggle_enabled(&pool, &tool.id).await?.unwrap();
        assert!(!toggled.enabled);
        Ok(())
    }

    #[tokio::test]
    async fn test_toggle_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let result = ToolsRepo::toggle_enabled(&pool, "nonexistent").await?;
        assert!(result.is_none());
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

        let render = tools
            .iter()
            .find(|t| t.name == "render_widget")
            .expect("the production registry must sync `render_widget` into the tools table");
        assert!(render.enabled, "render_widget must be enabled by default");
        Ok(())
    }

    #[tokio::test]
    async fn test_toggle_render_widget_marks_it_disabled() -> Result<(), Box<dyn std::error::Error>>
    {
        let state = crate::AppState::new_in_memory_empty().await;
        let render = ToolsRepo::list(&state.db)
            .await?
            .into_iter()
            .find(|t| t.name == "render_widget")
            .expect("render_widget must have been synced from the production registry");

        let toggled = ToolsRepo::toggle_enabled(&state.db, &render.id)
            .await?
            .expect("toggle must return the updated tool");
        assert!(!toggled.enabled);

        let disabled = ToolsRepo::disabled_names(&state.db).await?;
        assert!(
            disabled.contains(&"render_widget".to_string()),
            "disabled_names must include render_widget, got {disabled:?}"
        );
        Ok(())
    }
}
