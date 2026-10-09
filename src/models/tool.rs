use serde::{Deserialize, Serialize};

/// A registered tool, as exposed by `GET /api/tools`.
///
/// The per-tool `enabled` flag is gone: the selectable unit is now the skill
/// (see `orchestrator::skills`), so the tool catalogue is a plain read-only
/// listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
}
