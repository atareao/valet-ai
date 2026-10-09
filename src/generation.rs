//! Per-role generation parameters (temperature, reasoning, max tokens).
//!
//! The twelve `GENERATION_*` settings are seeded by migration and **read on
//! every LLM call** so that editing them in the UI takes effect without a
//! restart. This module centralises the reading/parsing so the three workers
//! and the orchestrator all behave identically.

use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::llm::provider::{ReasoningEffort, ReasoningSpec};

/// The four generation roles, each backed by its own three settings keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationRole {
    /// The conversational chat (orchestrator).
    Chat,
    /// The message-collapse worker.
    Collapse,
    /// Episodic-memory card extraction ("fichas").
    Memory,
    /// Consolidator and its compression pass.
    Semantic,
}

impl GenerationRole {
    /// The `GENERATION_*` prefix shared by the role's three keys.
    fn prefix(self) -> &'static str {
        match self {
            GenerationRole::Chat => "GENERATION_CHAT",
            GenerationRole::Collapse => "GENERATION_COLLAPSE",
            GenerationRole::Memory => "GENERATION_MEMORY",
            GenerationRole::Semantic => "GENERATION_SEMANTIC",
        }
    }

    /// `(temperature, reasoning, max_tokens)` defaults for the role.
    /// The reasoning default is the raw settings string (`""` means "omit").
    fn defaults(self) -> (f32, &'static str, u32) {
        match self {
            GenerationRole::Chat => (0.7, "", 4096),
            GenerationRole::Collapse => (0.2, "off", 1024),
            GenerationRole::Memory => (0.3, "off", 1024),
            GenerationRole::Semantic => (0.1, "off", 2048),
        }
    }
}

/// Resolved generation parameters for one LLM call.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationParams {
    /// Sampling temperature.
    pub temperature: f32,
    /// Reasoning control; `None` means "do not send the field".
    pub reasoning: Option<ReasoningSpec>,
    /// Maximum tokens for the completion.
    pub max_tokens: u32,
}

/// Read and parse the three generation keys for `role`.
///
/// Reads from `settings` on **every call**. A missing key or an unparseable
/// value falls back to the role default, logging a `tracing::warn!` for the
/// latter (and for database read errors).
pub async fn read_generation_params(pool: &SqlitePool, role: GenerationRole) -> GenerationParams {
    let prefix = role.prefix();
    let (default_temp, default_reasoning, default_max_tokens) = role.defaults();

    let temperature = read_f32(
        pool,
        &format!("{prefix}_TEMPERATURE"),
        default_temp,
        "temperature",
    )
    .await;
    let reasoning = read_reasoning(pool, &format!("{prefix}_REASONING"), default_reasoning).await;
    let max_tokens = read_u32(
        pool,
        &format!("{prefix}_MAX_TOKENS"),
        default_max_tokens,
        "max_tokens",
    )
    .await;

    GenerationParams {
        temperature,
        reasoning,
        max_tokens,
    }
}

/// Read a setting, returning the trimmed value; `None` when absent. A database
/// error is warned about and treated as absent.
async fn read_raw(pool: &SqlitePool, key: &str) -> Option<String> {
    match SettingsRepo::get(pool, key).await {
        Ok(Some(value)) => Some(value.trim().to_string()),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, key = %key, "failed to read generation setting; using default");
            None
        }
    }
}

async fn read_f32(pool: &SqlitePool, key: &str, default: f32, kind: &str) -> f32 {
    match read_raw(pool, key).await {
        Some(raw) => raw.parse::<f32>().unwrap_or_else(|_| {
            tracing::warn!(
                key = %key,
                value = %raw,
                default = default,
                "generation {kind} is not a valid number; using the role default"
            );
            default
        }),
        None => default,
    }
}

async fn read_u32(pool: &SqlitePool, key: &str, default: u32, kind: &str) -> u32 {
    match read_raw(pool, key).await {
        Some(raw) => raw.parse::<u32>().unwrap_or_else(|_| {
            tracing::warn!(
                key = %key,
                value = %raw,
                default = default,
                "generation {kind} is not a valid number; using the role default"
            );
            default
        }),
        None => default,
    }
}

async fn read_reasoning(pool: &SqlitePool, key: &str, default: &str) -> Option<ReasoningSpec> {
    let raw = read_raw(pool, key)
        .await
        .unwrap_or_else(|| default.to_string());

    match raw.as_str() {
        "" => None,
        "off" | "none" => Some(ReasoningSpec::Off),
        "minimal" => Some(ReasoningSpec::Effort(ReasoningEffort::Minimal)),
        "low" => Some(ReasoningSpec::Effort(ReasoningEffort::Low)),
        "medium" => Some(ReasoningSpec::Effort(ReasoningEffort::Medium)),
        "high" => Some(ReasoningSpec::Effort(ReasoningEffort::High)),
        "xhigh" => Some(ReasoningSpec::Effort(ReasoningEffort::XHigh)),
        "max" => Some(ReasoningSpec::Effort(ReasoningEffort::Max)),
        other => {
            tracing::warn!(
                key = %key,
                value = %other,
                "unknown reasoning level; falling back to `off`"
            );
            Some(ReasoningSpec::Off)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        pool
    }

    #[tokio::test]
    async fn chat_defaults() {
        let pool = db().await;
        let params = read_generation_params(&pool, GenerationRole::Chat).await;
        assert_eq!(params.temperature, 0.7);
        assert_eq!(params.reasoning, None);
        assert_eq!(params.max_tokens, 4096);
    }

    #[tokio::test]
    async fn collapse_defaults() {
        let pool = db().await;
        let params = read_generation_params(&pool, GenerationRole::Collapse).await;
        assert_eq!(params.temperature, 0.2);
        assert_eq!(params.reasoning, Some(ReasoningSpec::Off));
        assert_eq!(params.max_tokens, 1024);
    }

    #[tokio::test]
    async fn semantic_defaults() {
        let pool = db().await;
        let params = read_generation_params(&pool, GenerationRole::Semantic).await;
        assert_eq!(params.temperature, 0.1);
        assert_eq!(params.reasoning, Some(ReasoningSpec::Off));
        assert_eq!(params.max_tokens, 2048);
    }

    /// The code-level default for the Semantic role is `off` when the setting is absent.
    #[tokio::test]
    async fn semantic_default_reasoning_is_off_when_missing() {
        let pool = db().await;
        crate::db::repos::settings::SettingsRepo::delete(&pool, "GENERATION_SEMANTIC_REASONING")
            .await
            .expect("delete");
        let params = read_generation_params(&pool, GenerationRole::Semantic).await;
        assert_eq!(params.reasoning, Some(ReasoningSpec::Off));
    }

    #[tokio::test]
    async fn parses_effort_levels() {
        let pool = db().await;
        SettingsRepo::set(&pool, "GENERATION_CHAT_REASONING", "high")
            .await
            .unwrap();
        let params = read_generation_params(&pool, GenerationRole::Chat).await;
        assert_eq!(
            params.reasoning,
            Some(ReasoningSpec::Effort(ReasoningEffort::High))
        );
    }

    #[tokio::test]
    async fn off_and_none_map_to_off() {
        let pool = db().await;
        SettingsRepo::set(&pool, "GENERATION_CHAT_REASONING", "none")
            .await
            .unwrap();
        let params = read_generation_params(&pool, GenerationRole::Chat).await;
        assert_eq!(params.reasoning, Some(ReasoningSpec::Off));
    }

    #[tokio::test]
    async fn invalid_values_fall_back() {
        let pool = db().await;
        SettingsRepo::set(&pool, "GENERATION_MEMORY_TEMPERATURE", "alta")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "GENERATION_MEMORY_REASONING", "super")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "GENERATION_MEMORY_MAX_TOKENS", "muchos")
            .await
            .unwrap();

        let params = read_generation_params(&pool, GenerationRole::Memory).await;
        assert_eq!(params.temperature, 0.3);
        assert_eq!(params.reasoning, Some(ReasoningSpec::Off));
        assert_eq!(params.max_tokens, 1024);
    }

    #[tokio::test]
    async fn missing_keys_fall_back() {
        let pool = db().await;
        SettingsRepo::delete(&pool, "GENERATION_COLLAPSE_MAX_TOKENS")
            .await
            .unwrap();
        let params = read_generation_params(&pool, GenerationRole::Collapse).await;
        assert_eq!(params.max_tokens, 1024);
    }
}
