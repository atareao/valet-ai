use axum::extract::State;
use axum::Json;

use crate::errors::AppError;
use crate::models::Tool;
use crate::AppState;

/// `GET /api/tools` — read-only catalogue of the registered tools.
///
/// The per-tool enable/disable toggle is retired: skill selection is the unit
/// that filters what the router offers.
pub async fn list_tools(State(state): State<AppState>) -> Result<Json<Vec<Tool>>, AppError> {
    let tools = crate::db::repos::tools::ToolsRepo::list(&state.db).await?;
    Ok(Json(tools))
}
