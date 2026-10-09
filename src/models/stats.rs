use serde::{Deserialize, Serialize};

/// The origin of an LLM request, stored in `llm_requests.kind`.
///
/// Five processes share the `llm_requests` table; `CallKind` tells them apart
/// so chat aggregates can exclude background work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Chat,
    Router,
    Archivist,
    Consolidator,
    Collapse,
}

impl CallKind {
    /// The string persisted in the `kind` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            CallKind::Chat => "chat",
            CallKind::Router => "router",
            CallKind::Archivist => "archivist",
            CallKind::Consolidator => "consolidator",
            CallKind::Collapse => "collapse",
        }
    }
}

/// Global aggregate statistics over all LLM requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSummary {
    pub total_calls: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    pub total_cached_tokens: u64,
    pub total_reasoning_tokens: u64,
    pub total_cost: f64,
    pub total_errors: u64,
    pub avg_duration_ms: Option<f64>,
}

/// Aggregate LLM usage for a single background origin (non-chat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundStats {
    pub kind: String,
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub total_errors: u64,
    pub avg_duration_ms: Option<f64>,
}

/// Per-model breakdown of LLM usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model: String,
    pub calls: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_duration_ms: Option<f64>,
    pub total_cached_tokens: u64,
    pub total_reasoning_tokens: u64,
}

/// Per-day time-series of LLM usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayStats {
    pub date: String,
    pub calls: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub total_cached_tokens: u64,
    pub total_reasoning_tokens: u64,
}

/// Count of calls per tool name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub tool: String,
    pub count: u64,
}

/// Row count for a single database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSize {
    pub table: String,
    pub rows: u64,
}

/// Aggregate statistics over episodic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_memories: u64,
    pub total_tokens: u64,
    pub messages_indexed: u64,
    pub messages_total: u64,
}

/// Data from the most recent OpenRouter API call, kept in-memory for
/// observability via GET /api/stats/llm/last-call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastApiCall {
    pub model: String,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cached_tokens: u32,
    pub reasoning_tokens: u32,
    pub cost: f64,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub tool_calls: Option<String>,
    pub created_at: String,
}
