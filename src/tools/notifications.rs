//! Herramienta del agente para la bandeja de notificaciones (`notifications`).
//!
//! Dos operaciones sobre la bandeja: listar las notificaciones **no leídas**
//! (`list_unread`) y marcarlas como leídas (`mark_read`). Ambas delegan en
//! [`NotificationsRepo`], de modo que comparten exactamente la semántica de las
//! rutas homónimas: `list_unread` devuelve las mismas no leídas, más recientes
//! primero, y `mark_read` es idempotente. Leer y marcar como leído es
//! reversible, así que la herramienta no exige confirmación (`NoConfirm`).

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::db::repos::notifications::NotificationsRepo;
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Límite por defecto de `list_unread` cuando el modelo no lo indica.
const DEFAULT_LIMIT: i64 = 50;

/// Cota superior aceptada para `limit` (coincide con el máximo del endpoint).
const MAX_LIMIT: i64 = 200;

pub struct NotificationsTool {
    db: SqlitePool,
}

impl NotificationsTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Lista las notificaciones no leídas, más recientes primero.
    ///
    /// Delega en [`NotificationsRepo::list_unread`], la misma consulta que sirve
    /// a `GET /api/notifications/unread`: solo las que tienen `read_at` NULL.
    async fn list_unread(&self, args: Value) -> Result<ToolResult, ToolError> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(DEFAULT_LIMIT)
            .clamp(1, MAX_LIMIT);

        let (items, total) = NotificationsRepo::list_unread(&self.db, limit, 0).await?;

        Ok(ToolResult {
            success: true,
            data: json!({ "notifications": items, "total": total }),
            message: Some(format!("Hay {total} notificaciones sin leer")),
        })
    }

    /// Marca como leídas las notificaciones indicadas en `ids`.
    ///
    /// Comparte la semántica idempotente de `POST /api/notifications/mark-read`:
    /// solo marca las que estaban sin leer, los ids inexistentes se ignoran y
    /// repetir la llamada devuelve `0`. Exige al menos un `id`.
    async fn mark_read(&self, args: Value) -> Result<ToolResult, ToolError> {
        let ids: Vec<String> = args
            .get("ids")
            .and_then(|v| v.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        if ids.is_empty() {
            return Err(ToolError::InvalidArguments(
                "se requiere al menos un `id` en `ids`".into(),
            ));
        }

        let marked = NotificationsRepo::mark_read(&self.db, &ids).await?;

        Ok(ToolResult {
            success: true,
            data: json!({ "marked": marked }),
            message: Some(format!("{marked} notificaciones marcadas como leídas")),
        })
    }
}

#[async_trait]
impl Tool for NotificationsTool {
    fn name(&self) -> &'static str {
        "notifications"
    }

    fn description(&self) -> &'static str {
        "Notificaciones — avisos entrantes: listar los no leídos y marcarlos como leídos"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["list_unread", "mark_read"],
                    "description": "Acción a realizar. `list_unread` lista los avisos sin leer; `mark_read` exige `ids`."
                },
                "limit": { "type": "integer", "description": "Número máximo de avisos a devolver (por defecto 50, máximo 200)" },
                "ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "IDs de las notificaciones a marcar como leídas"
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("");

        match operation {
            "list_unread" => self.list_unread(args).await,
            "mark_read" => self.mark_read(args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "operación desconocida: {operation}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::notifications::Notification;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        crate::db::schema::run_migrations(&pool).await.unwrap();
        pool
    }

    /// Scenario: la herramienta figura en las definiciones con sus dos
    /// operaciones (`list_unread` y `mark_read`).
    #[tokio::test]
    async fn notifications_figures_in_definitions_with_its_operations() {
        let pool = setup_db().await;
        let registry = crate::build_tool_registry(&pool);

        let def = registry
            .definitions()
            .into_iter()
            .find(|d| d.name == "notifications")
            .expect("`notifications` must be advertised in definitions()");

        let ops = def.parameters["properties"]["operation"]["enum"]
            .as_array()
            .expect("the `operation` parameter must enumerate its operations")
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        assert!(ops.contains(&"list_unread"), "enum must offer list_unread");
        assert!(ops.contains(&"mark_read"), "enum must offer mark_read");
    }

    /// Scenario: el permiso de la herramienta es `NoConfirm`.
    #[tokio::test]
    async fn notifications_permission_is_noconfirm() {
        let pool = setup_db().await;
        let tool = NotificationsTool::new(pool);
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
    }

    /// Scenario: `list_unread` coincide con el endpoint — devuelve las mismas
    /// no leídas y no devuelve las leídas.
    #[tokio::test]
    async fn list_unread_returns_only_the_unread_ones() {
        let pool = setup_db().await;
        let tool = NotificationsTool::new(pool.clone());

        let unread_a = NotificationsRepo::insert(&pool, "monitor", "a", "sin leer A", "info", None)
            .await
            .unwrap();
        let _unread_b =
            NotificationsRepo::insert(&pool, "monitor", "b", "sin leer B", "info", None)
                .await
                .unwrap();
        let read = NotificationsRepo::insert(&pool, "monitor", "c", "ya leída", "info", None)
            .await
            .unwrap();
        NotificationsRepo::mark_read(&pool, std::slice::from_ref(&read))
            .await
            .unwrap();

        let result = tool
            .execute(serde_json::json!({ "operation": "list_unread" }))
            .await
            .unwrap();
        assert!(result.success);

        let items: Vec<Notification> =
            serde_json::from_value(result.data["notifications"].clone()).unwrap();
        assert_eq!(items.len(), 2, "only the two unread notifications");
        assert!(items.iter().all(|n| n.read_at.is_none()));
        assert!(
            items.iter().any(|n| n.id == unread_a),
            "the unread notification must be listed"
        );
        assert!(
            !items.iter().any(|n| n.id == read),
            "the read notification must not be listed"
        );
    }

    /// Scenario: `mark_read` es idempotente también para el agente — la primera
    /// invocación marca 2 y la repetición marca 0.
    #[tokio::test]
    async fn mark_read_is_idempotent_for_the_agent() {
        let pool = setup_db().await;
        let tool = NotificationsTool::new(pool.clone());

        let a = NotificationsRepo::insert(&pool, "monitor", "a", "uno", "info", None)
            .await
            .unwrap();
        let b = NotificationsRepo::insert(&pool, "monitor", "b", "dos", "info", None)
            .await
            .unwrap();

        let first = tool
            .execute(serde_json::json!({ "operation": "mark_read", "ids": [a, b] }))
            .await
            .unwrap();
        assert_eq!(
            first.data["marked"], 2,
            "both unread are marked the first time"
        );

        let again = tool
            .execute(serde_json::json!({ "operation": "mark_read", "ids": [a, b] }))
            .await
            .unwrap();
        assert_eq!(again.data["marked"], 0, "repeating the mark is a no-op");
    }
}
