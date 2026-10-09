use async_trait::async_trait;
use serde_json::Value;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Widget names the backend is willing to ask the client to render.
const ALLOWED_WIDGETS: &[&str] = &["QuickForm", "Checklist", "LocationWidget"];

/// Schema of the `data` argument, documented in the tool definition so the model
/// emits the keys each widget actually reads.
const DATA_SCHEMA_DESCRIPTION: &str = "Datos del widget según widget_name. QuickForm: {\"title\": str, \"fields\": [{\"name\": str, \"label\": str, \"type\": text|textarea|number|select|checkbox|slider, \"options\": [str] (solo cuando type es select), \"min\": num, \"max\": num (solo cuando type es slider)}], \"submit_label\": str}. Checklist: {\"title\": str, \"items\": [{\"id\": str, \"label\": str}]}. LocationWidget: {\"title\": str, \"description\"?: str, \"latitude\": num, \"longitude\": num, \"address\"?: str}.";

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
                    "description": DATA_SCHEMA_DESCRIPTION,
                    "anyOf": [
                        {
                            "title": "QuickForm",
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "fields": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string" },
                                            "label": { "type": "string" },
                                            "type": {
                                                "type": "string",
                                                "enum": ["text", "textarea", "number", "select", "checkbox", "slider"]
                                            },
                                            "options": { "type": "array", "items": { "type": "string" } },
                                            "min": { "type": "number" },
                                            "max": { "type": "number" }
                                        }
                                    }
                                },
                                "submit_label": { "type": "string" }
                            }
                        },
                        {
                            "title": "Checklist",
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "items": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "label": { "type": "string" }
                                        }
                                    }
                                }
                            }
                        },
                        {
                            "title": "LocationWidget",
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "description": { "type": "string" },
                                "latitude": { "type": "number" },
                                "longitude": { "type": "number" },
                                "address": { "type": "string" }
                            }
                        }
                    ]
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
    async fn test_render_widget_location_widget_succeeds() {
        let tool = RenderWidgetTool::new();
        let data = serde_json::json!({"title": "X", "latitude": 1.0, "longitude": 2.0});
        let result = tool
            .execute(serde_json::json!({
                "widget_name": "LocationWidget",
                "data": data.clone(),
            }))
            .await
            .expect("a permitted LocationWidget name with object data must succeed");

        assert!(result.success);
        assert_eq!(result.data["widget_name"], "LocationWidget");
        assert_eq!(
            result.data["data"], data,
            "an object `data` must be passed through unchanged"
        );
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

    /// The tool must document the shape of `data` for each allowed widget so the
    /// model emits the correct keys instead of inventing its own.
    #[test]
    fn test_render_widget_parameters_document_data_schema() {
        let params = RenderWidgetTool::new().parameters();

        // `data` is an object refined with a union of widget shapes (`anyOf`),
        // one alternative per allowed widget.
        assert_eq!(
            params["properties"]["data"]["type"],
            serde_json::json!("object"),
            "`data` must remain a plain object"
        );

        // `widget_name` keeps its allowlist unchanged.
        assert_eq!(
            params["properties"]["widget_name"]["enum"],
            serde_json::json!(["QuickForm", "Checklist", "LocationWidget"]),
            "`widget_name` enum must be [\"QuickForm\", \"Checklist\", \"LocationWidget\"]"
        );

        // `data` must carry a description documenting its schema.
        let desc = params["properties"]["data"]["description"]
            .as_str()
            .expect("`data` must carry a description documenting its schema");

        for token in [
            "QuickForm",
            "Checklist",
            "LocationWidget",
            "latitude",
            "longitude",
            "address",
            "fields",
            "name",
            "label",
            "type",
            "options",
            "submit_label",
            "items",
            "id",
            "text",
            "textarea",
            "number",
            "select",
            "checkbox",
            "slider",
        ] {
            assert!(
                desc.contains(token),
                "the `data` description must mention `{token}`; got: {desc}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: `data` stays optional and its schema uses a
    // union of widget shapes (`anyOf`) with one alternative per widget.
    // -----------------------------------------------------------------------

    /// Scenario: data sigue siendo opcional
    #[test]
    fn test_render_widget_data_is_not_required() {
        let params = RenderWidgetTool::new().parameters();

        assert_eq!(
            params["required"],
            serde_json::json!(["widget_name"]),
            "`required` must be exactly [\"widget_name\"], got: {}",
            params["required"]
        );
        let required = params["required"].as_array().unwrap();
        assert!(
            !required.iter().any(|v| v.as_str() == Some("data")),
            "`required` must NOT contain `data`, got: {required:?}"
        );
    }

    /// Scenario: El esquema de data usa una unión de formas por widget
    #[test]
    fn test_render_widget_data_schema_uses_union_of_widget_shapes() {
        let params = RenderWidgetTool::new().parameters();
        let data = &params["properties"]["data"];

        let alternatives = data
            .get("oneOf")
            .or_else(|| data.get("anyOf"))
            .and_then(|v| v.as_array())
            .expect("`data` must define a union of widget shapes via `oneOf` or `anyOf`");

        assert_eq!(
            alternatives.len(),
            ALLOWED_WIDGETS.len(),
            "there must be one alternative per widget ({ALLOWED_WIDGETS:?}), got {} alternatives",
            alternatives.len()
        );

        // Each branch is located by its `title`, and its structure is checked
        // against the keys the corresponding widget actually reads, instead of
        // merely asserting that the widget names appear somewhere in the union.
        let find_branch = |title: &str| {
            alternatives
                .iter()
                .find(|alt| alt["title"].as_str() == Some(title))
                .unwrap_or_else(|| panic!("missing `{title}` branch in `data` union: {data}"))
        };

        let quick_form = find_branch("QuickForm");
        assert!(
            quick_form["properties"]["fields"].is_object(),
            "QuickForm must declare `properties.fields`, got: {quick_form}"
        );

        let checklist = find_branch("Checklist");
        assert!(
            checklist["properties"]["items"].is_object(),
            "Checklist must declare `properties.items`, got: {checklist}"
        );

        let location = find_branch("LocationWidget");
        assert!(
            location["properties"]["latitude"].is_object(),
            "LocationWidget must declare `properties.latitude`, got: {location}"
        );
        assert!(
            location["properties"]["longitude"].is_object(),
            "LocationWidget must declare `properties.longitude`, got: {location}"
        );

        // Every branch must correspond to one of the `widget_name` enum values.
        let enum_values = params["properties"]["widget_name"]["enum"]
            .as_array()
            .expect("`widget_name` must expose an enum");
        for alt in alternatives {
            let title = alt["title"].as_str().unwrap_or("");
            assert!(
                enum_values.iter().any(|v| v.as_str() == Some(title)),
                "union branch `{title}` must match a `widget_name` enum value, got: {enum_values:?}"
            );
        }
    }
}
