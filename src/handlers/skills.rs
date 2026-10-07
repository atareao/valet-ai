//! Read-only handler for the closed skills catalog.
//!
//! The catalog lives in code (`crate::orchestrator::skills::catalog()`) and is
//! the single source of truth for the router and the evaluation harness. This
//! endpoint exposes it —plus the non-routable core set— so the interface never
//! duplicates the catalog in its own code.

use axum::Json;
use serde_json::{json, Value};

use crate::orchestrator::skills::{catalog, CORE_TOOLS};

/// `GET /api/skills`
///
/// Serialises the closed catalog from [`catalog()`] (id, `prompt_key`,
/// `prompt_heading` and the tools each skill covers, prerequisites included)
/// together with the always-exposed core set.
pub async fn list_skills() -> Json<Value> {
    let skills: Vec<Value> = catalog()
        .iter()
        .map(|spec| {
            json!({
                "id": spec.id,
                "prompt_key": spec.prompt_key,
                "prompt_heading": spec.prompt_heading,
                "tools": spec.tools,
            })
        })
        .collect();

    Json(json!({
        "skills": skills,
        "core_tools": CORE_TOOLS,
    }))
}
