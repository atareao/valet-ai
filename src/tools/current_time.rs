use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

pub struct CurrentTimeTool {
    pub db: SqlitePool,
}

impl CurrentTimeTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for CurrentTimeTool {
    fn name(&self) -> &'static str {
        "get_current_time"
    }

    fn description(&self) -> &'static str {
        "Obtiene la fecha y la hora actuales del usuario según su zona horaria configurada, en un formato legible."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({"type": "object", "properties": {}})
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let timezone = SettingsRepo::get(&self.db, "timezone")
            .await?
            .unwrap_or_else(|| "Europe/Madrid".to_string());

        let result = crate::tools::time_format::format_time_now(&timezone);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({"time": result}),
            message: Some(result),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup_pool() -> SqlitePool {
        use sqlx::migrate::Migrator;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("failed to create in-memory SQLite pool");
        Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_tool_name_and_description() {
        let pool = setup_pool().await;
        let tool = CurrentTimeTool { db: pool };
        assert_eq!(tool.name(), "get_current_time");
        assert!(tool.description().contains("hora"));
    }

    #[tokio::test]
    async fn test_tool_parameters_is_empty_object() {
        let pool = setup_pool().await;
        let tool = CurrentTimeTool { db: pool };
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert_eq!(params["properties"], serde_json::json!({}));
    }

    #[tokio::test]
    async fn test_tool_permission() {
        let pool = setup_pool().await;
        let tool = CurrentTimeTool { db: pool };
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
    }

    #[tokio::test]
    async fn test_execute_with_no_timezone_set() {
        // When no timezone setting exists, it should fall back to Europe/Madrid
        // and return a valid time string.
        let pool = setup_pool().await;
        let tool = CurrentTimeTool { db: pool };
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.success);
        assert!(result.data["time"].as_str().unwrap().starts_with("Hoy es "));
        assert!(result.message.is_some());
    }

    // -----------------------------------------------------------------------
    // Language guard — description in Spanish
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_current_time_description_is_spanish() {
        let pool = setup_pool().await;
        let tool = CurrentTimeTool { db: pool };
        let desc = tool.description();
        assert!(
            desc.contains("hora") && desc.contains("fecha"),
            "get_current_time description must be in Spanish (must contain 'hora' and 'fecha'), got: {desc}"
        );
    }
}
