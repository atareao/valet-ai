use axum::extract::{Path, State};
use axum::Json;

use crate::errors::AppError;
use crate::models::Tool;
use crate::AppState;

pub async fn list_tools(State(state): State<AppState>) -> Result<Json<Vec<Tool>>, AppError> {
    let tools = crate::db::repos::tools::ToolsRepo::list(&state.db).await?;
    Ok(Json(tools))
}

pub async fn toggle_tool(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Tool>, AppError> {
    let tool = crate::db::repos::tools::ToolsRepo::toggle_enabled(&state.db, &id).await?;
    let tool = tool.ok_or_else(|| AppError::NotFound(format!("Tool {} not found", id)))?;

    // Keep the in-memory registry in sync so a disabled tool is immediately
    // hidden from the prompt and rejected at execution time.
    if let Some(registry) = state.tool_registry.as_ref() {
        match crate::db::repos::tools::ToolsRepo::disabled_names(&state.db).await {
            Ok(disabled) => registry.set_disabled(disabled),
            Err(e) => tracing::warn!("failed to refresh disabled tools: {e}"),
        }
    }

    Ok(Json(tool))
}
