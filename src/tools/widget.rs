use async_trait::async_trait;
use serde_json::Value;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Widget names the backend is willing to ask the client to render.
const ALLOWED_WIDGETS: &[&str] = &["QuickForm", "Checklist"];

/// Name under which this tool is registered and intercepted by the orchestrator.
pub const RENDER_WIDGET_TOOL_NAME: &str = "render_widget";

/// Normalise the optional `data` argument: a missing or non-object value becomes
/// an empty object, while an object is passed through unchanged.
fn normalize_widget_data(args: &Value) -> Value {
    args.get("data")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Tool the LLM invokes to ask the client to render an interactive widget.
///
/// The allowed widget names form a backend allowlist; the frontend verifies
/// them again against its own registry.
pub struct RenderWidgetTool;

impl RenderWidgetTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RenderWidgetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RenderWidgetTool {
    fn name(&self) -> &'static str {
        RENDER_WIDGET_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "Renderiza un widget interactivo en el chat del usuario para recabar una decisión o mostrar un plan accionable."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "widget_name": {
                    "type": "string",
                    "enum": ALLOWED_WIDGETS,
                    "description": "Nombre del widget a renderizar."
                },
                "data": {
                    "type": "object",
                    "description": "Datos que el widget necesita para renderizarse."
                }
            },
            "required": ["widget_name"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let widget_name = args
            .get("widget_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments("'widget_name' is required and must be a string".into())
            })?;

        if !ALLOWED_WIDGETS.contains(&widget_name) {
            return Err(ToolError::InvalidArguments(format!(
                "widget '{widget_name}' is not allowed; expected one of {ALLOWED_WIDGETS:?}"
            )));
        }

        // `data` is optional: a missing or non-object value normalises to `{}`.
        let normalized_data = normalize_widget_data(&args);

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "rendered": true,
                "widget_name": widget_name,
                "data": normalized_data,
            }),
            message: Some(format!("Widget '{widget_name}' requested")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_render_widget_allowed_name_succeeds() {
        let tool = RenderWidgetTool::new();
        let data = serde_json::json!({"title": "Elige", "items": []});
        let result = tool
            .execute(serde_json::json!({
                "widget_name": "Checklist",
                "data": data.clone(),
            }))
            .await
            .expect("a permitted widget name with object data must succeed");

        assert!(result.success);
        assert_eq!(result.data["rendered"], serde_json::json!(true));
        assert_eq!(result.data["widget_name"], "Checklist");
        assert_eq!(
            result.data["data"], data,
            "an object `data` must be passed through unchanged"
        );
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_render_widget_unknown_name_is_invalid_arguments() {
        let tool = RenderWidgetTool::new();
        let result = tool
            .execute(serde_json::json!({"widget_name": "SystemMonitor"}))
            .await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "an unknown widget name must be rejected with InvalidArguments, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_render_widget_missing_name_is_invalid_arguments() {
        let tool = RenderWidgetTool::new();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "a missing widget name must be rejected with InvalidArguments, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_render_widget_missing_data_normalizes_to_empty_object() {
        let tool = RenderWidgetTool::new();
        let result = tool
            .execute(serde_json::json!({"widget_name": "QuickForm"}))
            .await
            .expect("missing data must normalize to an empty object and succeed");
        assert!(result.success);
        assert_eq!(
            result.data["data"],
            serde_json::json!({}),
            "missing data must be normalized to an empty object"
        );
    }

    #[tokio::test]
    async fn test_render_widget_non_object_data_normalizes_to_empty_object() {
        let tool = RenderWidgetTool::new();
        let result = tool
            .execute(serde_json::json!({"widget_name": "QuickForm", "data": "not-an-object"}))
            .await
            .expect("non-object data must normalize to an empty object and succeed");
        assert!(result.success);
        assert_eq!(
            result.data["data"],
            serde_json::json!({}),
            "non-object data must be normalized to an empty object"
        );
    }

    #[tokio::test]
    async fn test_render_widget_requires_no_confirmation() {
        let tool = RenderWidgetTool::new();
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
    }

    /// `name()` is asserted together with a behavioural expectation, so the test
    /// only passes when the tool actually accepts a permitted widget.
    #[tokio::test]
    async fn test_render_widget_name_and_allowed_execution() {
        let tool = RenderWidgetTool::new();
        assert_eq!(tool.name(), "render_widget");
        let result = tool
            .execute(serde_json::json!({"widget_name": "Checklist", "data": {}}))
            .await;
        assert!(
            result.is_ok(),
            "a permitted widget name must execute successfully"
        );
    }
}
