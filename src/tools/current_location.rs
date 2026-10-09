use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::tools::geo_utils;
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

pub struct CurrentLocationTool {
    #[allow(dead_code)]
    db: SqlitePool,
}

impl CurrentLocationTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for CurrentLocationTool {
    fn name(&self) -> &'static str {
        "get_current_location"
    }

    fn description(&self) -> &'static str {
        "Obtiene la ubicación actual del usuario a partir de sus coordenadas guardadas. Devuelve la dirección con calle, ciudad y región, además de las propias coordenadas."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let latitude = SettingsRepo::get(&self.db, "latitude").await?;
        let longitude = SettingsRepo::get(&self.db, "longitude").await?;

        let result = match (latitude, longitude) {
            (Some(lat_str), Some(lon_str)) => {
                let lat: f64 = lat_str.parse().map_err(|e| {
                    ToolError::ExecutionError(format!("Invalid latitude value: {}", e))
                })?;
                let lon: f64 = lon_str.parse().map_err(|e| {
                    ToolError::ExecutionError(format!("Invalid longitude value: {}", e))
                })?;

                match geo_utils::reverse_geocode(lat, lon).await {
                    Some(address) => format!("{} ({:.4}, {:.4}).", address, lat, lon),
                    None => format!("({:.4}, {:.4}).", lat, lon),
                }
            }
            _ => "Ubicación no configurada.".to_string(),
        };

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({"location": result}),
            message: Some(result),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
    async fn test_tool_name() {
        let pool = setup_pool().await;
        let tool = CurrentLocationTool::new(pool);
        assert_eq!(tool.name(), "get_current_location");
    }

    #[tokio::test]
    async fn test_tool_description_not_empty() {
        let pool = setup_pool().await;
        let tool = CurrentLocationTool::new(pool);
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_tool_parameters_empty_object() {
        let pool = setup_pool().await;
        let tool = CurrentLocationTool::new(pool);
        let params = tool.parameters();
        assert_eq!(params, json!({"type": "object", "properties": {}}));
    }

    #[tokio::test]
    async fn test_tool_permission_no_confirm() {
        let pool = setup_pool().await;
        let tool = CurrentLocationTool::new(pool);
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
    }

    // -----------------------------------------------------------------------
    // Language guard — description in Spanish
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_current_location_description_is_spanish() {
        let pool = setup_pool().await;
        let tool = CurrentLocationTool::new(pool);
        let desc = tool.description();
        assert!(
            desc.contains("ubicación"),
            "get_current_location description must be in Spanish (must contain 'ubicación'), got: {desc}"
        );
    }
}
