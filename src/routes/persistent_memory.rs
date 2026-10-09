//! HTTP surface for the persistent memory (Capa C): read, validated write with
//! budget/ceiling enforcement and optimistic concurrency, and clearing.
//!
//! All the domain logic lives in [`crate::persistent_memory`]; the persistence
//! in [`crate::db::repos::persistent_memory`]. This module only composes them
//! and maps the outcomes to HTTP statuses.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::repos::persistent_memory::PersistentMemoryRepo;
use crate::errors::AppError;
use crate::persistent_memory::{
    absolute_ceiling, evaluate_compressed, is_empty_state, minified_json, payload_token_count,
    read_budget, resolve_updated_at, validate_payload, CompressionOutcome,
};
use crate::AppState;

/// Read response: the state plus its size bounds. No `warning` key.
#[derive(Debug, Serialize)]
pub struct PersistentMemoryStateResponse {
    /// The parsed state, or `null` when there is no row.
    pub payload: Option<Value>,
    /// The Rust-owned mark, or `null` when there is no row.
    pub updated_at: Option<String>,
    /// Tokens of the minified state (0 when empty).
    pub token_count: usize,
    /// The configured budget (`PERSISTENT_MEMORY_BUDGET_TOKENS`, default 500).
    pub budget_tokens: usize,
    /// The absolute ceiling: twice the budget.
    pub ceiling_tokens: usize,
    /// Whether the state carries no `user_profile` entries nor `system_rules`.
    pub is_empty: bool,
}

/// Write response: same shape as the read, plus the optional size warning.
#[derive(Debug, Serialize)]
pub struct PersistentMemoryWriteResponse {
    pub payload: Option<Value>,
    pub updated_at: Option<String>,
    pub token_count: usize,
    pub budget_tokens: usize,
    pub ceiling_tokens: usize,
    pub is_empty: bool,
    /// Non-empty when the state was stored above the budget but under the
    /// ceiling; `null` otherwise.
    pub warning: Option<String>,
}

/// `PUT` body.
#[derive(Debug, Deserialize)]
pub struct UpdatePersistentMemoryRequest {
    /// Candidate state, validated against the versioned schema.
    pub payload: Value,
    /// Optional optimistic-concurrency token. Distinguishes three cases:
    /// omitted (`None`), explicit `null` (`Some(None)`) and a string
    /// (`Some(Some(s))`).
    #[serde(default, deserialize_with = "deserialize_present_optional")]
    pub expected_updated_at: Option<Option<String>>,
}

/// Deserialize only when the field is present: `null` yields `Some(None)`,
/// while an absent field keeps the `Default` (`None`) thanks to `#[serde(default)]`.
fn deserialize_present_optional<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

/// Read the state. Absence of a row is a valid empty state; reading never
/// creates it. A stored payload that does not parse is an invariant violation
/// and is reported as `500`, never silently treated as empty.
pub async fn get_persistent_memory(
    State(state): State<AppState>,
) -> Result<Json<PersistentMemoryStateResponse>, AppError> {
    let budget_tokens = read_budget(&state.db).await;
    let row = PersistentMemoryRepo::get(&state.db).await?;

    let (payload, token_count, is_empty) = match &row {
        Some(entry) => {
            let parsed: Value = serde_json::from_str(&entry.payload).map_err(|e| {
                AppError::Internal(format!(
                    "the stored persistent-memory payload is not valid JSON: {e}"
                ))
            })?;
            let token_count = payload_token_count(&parsed);
            let is_empty = is_empty_state(&parsed);
            (Some(parsed), token_count, is_empty)
        }
        None => (None, 0, true),
    };

    Ok(Json(PersistentMemoryStateResponse {
        payload,
        updated_at: row.map(|entry| entry.updated_at),
        token_count,
        budget_tokens,
        ceiling_tokens: absolute_ceiling(budget_tokens),
        is_empty,
    }))
}

/// Write the state: validate the schema, classify its size against the same
/// budget/ceiling rule as the consolidation worker, and persist with a
/// Rust-resolved `updated_at`. Optimistic concurrency is enforced atomically in
/// the write statement itself. No LLM.
pub async fn update_persistent_memory(
    State(state): State<AppState>,
    Json(body): Json<UpdatePersistentMemoryRequest>,
) -> Result<Json<PersistentMemoryWriteResponse>, AppError> {
    // 1. Validate before touching anything. Rejection writes nothing.
    let normalized = validate_payload(&body.payload)
        .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

    // `previous` is read ONLY to resolve the content-hash `updated_at`; it is
    // never the concurrency guard (that lives in the SQL `WHERE`).
    let previous = PersistentMemoryRepo::get(&state.db).await?;

    // 2. Measure and enforce the size bounds with the same classifier the
    //    consolidation worker uses. A rejection keeps the previous state:
    //    nothing has been written yet.
    let budget_tokens = read_budget(&state.db).await;
    let ceiling_tokens = absolute_ceiling(budget_tokens);
    let token_count = payload_token_count(&normalized);

    let warning = match evaluate_compressed(token_count, budget_tokens) {
        CompressionOutcome::Reject => {
            return Err(AppError::UnprocessableEntity(format!(
                "the persistent state uses {token_count} tokens, above the absolute ceiling of {ceiling_tokens} (budget {budget_tokens}); it was not saved"
            )));
        }
        CompressionOutcome::StoreWithWarning => Some(format!(
            "the persistent state uses {token_count} tokens, above the budget of {budget_tokens}; it was saved with a warning"
        )),
        CompressionOutcome::Store => None,
    };

    // 3. Rust owns `updated_at`; the client's date is ignored.
    let now = chrono::Utc::now().to_rfc3339();
    let updated_at = resolve_updated_at(previous.as_ref(), &normalized, &now);
    let payload_text = minified_json(&normalized);

    // 4. Atomic optimistic concurrency: the guard is part of the write
    //    statement itself, so a consolidation on the same pool cannot slip in
    //    between the check and the write (no TOCTOU window).
    let conflict = match &body.expected_updated_at {
        // Omitted: unconditional write.
        None => {
            PersistentMemoryRepo::upsert(&state.db, &payload_text, &updated_at).await?;
            false
        }
        // Explicit `null`: expected no row ⇒ insert only if still absent.
        Some(None) => {
            PersistentMemoryRepo::insert_if_absent(&state.db, &payload_text, &updated_at).await?
                == 0
        }
        // Expected a concrete mark ⇒ update only if it still matches.
        Some(Some(expected)) => {
            PersistentMemoryRepo::update_if_mark(&state.db, &payload_text, &updated_at, expected)
                .await?
                == 0
        }
    };

    if conflict {
        return Err(AppError::Conflict(
            "the persistent state changed since it was last read".to_string(),
        ));
    }

    let is_empty = is_empty_state(&normalized);
    Ok(Json(PersistentMemoryWriteResponse {
        payload: Some(normalized),
        updated_at: Some(updated_at),
        token_count,
        budget_tokens,
        ceiling_tokens,
        is_empty,
        warning,
    }))
}

/// Clear the state. Idempotent: clearing an absent state also succeeds.
pub async fn delete_persistent_memory(
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    PersistentMemoryRepo::delete(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
