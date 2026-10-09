//! Capa C (persistent-memory) core.
//!
//! Pure, side-effect-free logic for the stable user state: the schema/version
//! validation and the Rust-owned `updated_at` rule. The persistence itself
//! lives in [`crate::db::repos::persistent_memory`]; the consolidation worker
//! (a later block) composes both.

/// The id of the single logical row of the persistent-memory table.
pub const GLOBAL_STATE_ID: &str = "global_state";

/// The only accepted `schema_version`.
pub const CURRENT_SCHEMA_VERSION: i64 = 1;

/// Default token budget for the persistent state when the
/// `PERSISTENT_MEMORY_BUDGET_TOKENS` setting is missing or unparseable.
pub const PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT: usize = 800;

/// The fixed, closed set of allowed top-level payload keys. The LLM cannot add
/// sections: anything outside this set is discarded.
pub const ALLOWED_TOP_LEVEL_KEYS: [&str; 3] = ["schema_version", "user_profile", "system_rules"];

/// Reasons a candidate persistent-memory payload is rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    #[error("payload is not a JSON object")]
    NotAnObject,
    #[error("schema_version is missing or not an integer")]
    InvalidSchemaVersion,
    #[error("unsupported schema_version {0} (expected {CURRENT_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(i64),
    #[error("key '{0}' has an invalid shape")]
    InvalidShape(String),
}

/// Validate a candidate payload against the fixed, versioned schema and return
/// the **normalized** state.
///
/// - `schema_version` must be the integer `1`; missing, non-integer or any
///   other version is rejected.
/// - Only the keys in [`ALLOWED_TOP_LEVEL_KEYS`] survive; unknown top-level
///   keys are discarded (the LLM cannot invent sections).
/// - `user_profile`, when present, must be an object; `system_rules`, when
///   present, must be an array of strings.
///
/// The result is never truncated by count: no section is capped by size.
pub fn validate_payload(candidate: &serde_json::Value) -> Result<serde_json::Value, SchemaError> {
    let object = candidate.as_object().ok_or(SchemaError::NotAnObject)?;

    let version = match object.get("schema_version") {
        Some(serde_json::Value::Number(number)) => {
            number.as_i64().ok_or(SchemaError::InvalidSchemaVersion)?
        }
        Some(_) => return Err(SchemaError::InvalidSchemaVersion),
        None => return Err(SchemaError::InvalidSchemaVersion),
    };
    if version != CURRENT_SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedSchemaVersion(version));
    }

    // Rebuild from the closed set of allowed keys: unknown top-level keys are
    // dropped and the section set is fixed.
    let mut normalized = serde_json::Map::new();
    normalized.insert(
        "schema_version".to_string(),
        serde_json::Value::from(CURRENT_SCHEMA_VERSION),
    );

    if let Some(profile) = object.get("user_profile") {
        if !profile.is_object() {
            return Err(SchemaError::InvalidShape("user_profile".into()));
        }
        normalized.insert("user_profile".to_string(), profile.clone());
    }

    if let Some(rules) = object.get("system_rules") {
        let array = rules
            .as_array()
            .ok_or_else(|| SchemaError::InvalidShape("system_rules".into()))?;
        if !array.iter().all(serde_json::Value::is_string) {
            return Err(SchemaError::InvalidShape("system_rules".into()));
        }
        normalized.insert("system_rules".to_string(), rules.clone());
    }

    Ok(serde_json::Value::Object(normalized))
}

/// Stable content hash of a payload, **excluding** the `updated_at` key.
///
/// Used to decide whether the state changed between consolidations: an
/// LLM-supplied date must not count as content.
pub fn content_hash(payload: &serde_json::Value) -> String {
    let mut canonical = payload.clone();
    if let Some(object) = canonical.as_object_mut() {
        object.remove("updated_at");
    }

    // `serde_json`'s default object map is a `BTreeMap`, so serialization is
    // key-sorted and deterministic for a given logical value.
    let serialized = serde_json::to_string(&canonical).unwrap_or_default();

    // FNV-1a (64-bit): a fast, non-cryptographic hash used **only** for change
    // detection between consolidations. It must not be used for security.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in serialized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Resolve the `updated_at` of the next state (Rust owns this value).
///
/// If `previous` exists and its payload has the same content hash as
/// `candidate` (ignoring the timestamp), the previous `updated_at` is kept.
/// Otherwise — changed content, unreadable previous payload, or a new state —
/// `now` is sealed. The candidate's own date is never read.
pub fn resolve_updated_at(
    previous: Option<&crate::models::PersistentMemory>,
    candidate: &serde_json::Value,
    now: &str,
) -> String {
    if let Some(previous) = previous {
        if let Ok(previous_payload) = serde_json::from_str::<serde_json::Value>(&previous.payload) {
            if content_hash(&previous_payload) == content_hash(candidate) {
                return previous.updated_at.clone();
            }
        }
    }
    now.to_string()
}

/// Compact (minified) JSON serialization of a payload.
///
/// This is the exact form the state is measured in and injected as. It is
/// always valid JSON, because it comes from `serde_json` serialization.
pub fn minified_json(payload: &serde_json::Value) -> String {
    serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string())
}

/// Token count of the state, measured over its minified JSON form.
///
/// Uses the project's own deterministic JSON estimator
/// ([`crate::token_estimate::estimate_json_tokens`]): it walks the minified
/// state as JSON, so it does not undercount long, spaceless string values the
/// way a prose heuristic would. This is the measure the budget and the absolute
/// ceiling are enforced against. Chat-message measurement is unrelated and is
/// not affected.
pub fn payload_token_count(payload: &serde_json::Value) -> usize {
    let minified = minified_json(payload);
    crate::token_estimate::estimate_json_tokens(&minified)
}

/// Whether the state carries no meaningful content.
///
/// A state is empty when neither `user_profile` has entries nor `system_rules`
/// has rules (missing/empty section ⇒ empty). An empty state is never injected
/// and leaves no trace.
pub fn is_empty_state(payload: &serde_json::Value) -> bool {
    let Some(object) = payload.as_object() else {
        return true;
    };

    let profile_empty = object
        .get("user_profile")
        .and_then(serde_json::Value::as_object)
        .is_none_or(|p| p.is_empty());
    let rules_empty = object
        .get("system_rules")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|r| r.is_empty());

    profile_empty && rules_empty
}

/// The absolute ceiling: twice the configured budget. Above it, a consolidated
/// state is refused (the previous one is kept) rather than truncated.
pub fn absolute_ceiling(budget_tokens: usize) -> usize {
    budget_tokens.saturating_mul(2)
}

/// Read the persistent-memory token budget from `settings`.
///
/// Missing key or unparseable value fall back to
/// [`PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT`]. A real database error is
/// distinguished and warned about, then also falls back. Shared by the
/// consolidation worker and the HTTP surface so both enforce the same budget.
pub async fn read_budget(pool: &sqlx::SqlitePool) -> usize {
    match crate::db::repos::settings::SettingsRepo::get(pool, "PERSISTENT_MEMORY_BUDGET_TOKENS")
        .await
    {
        Ok(Some(value)) => value.trim().parse::<usize>().unwrap_or_else(|_| {
            tracing::warn!(
                value = %value,
                default = PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT,
                "PERSISTENT_MEMORY_BUDGET_TOKENS is not a valid usize; using the default"
            );
            PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT
        }),
        Ok(None) => PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT,
        Err(e) => {
            tracing::warn!(
                error = %e,
                default = PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT,
                "failed to read PERSISTENT_MEMORY_BUDGET_TOKENS; using the default"
            );
            PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT
        }
    }
}

/// How a state that went through a single compression relates to the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionOutcome {
    /// Fits the budget: store it as is.
    Store,
    /// Fits under the ceiling but still exceeds the budget: store it and warn.
    StoreWithWarning,
    /// Exceeds the absolute ceiling: refuse the write and keep the previous
    /// state.
    Reject,
}

/// Classify a compressed state against the budget and the absolute ceiling.
pub fn evaluate_compressed(compressed_tokens: usize, budget_tokens: usize) -> CompressionOutcome {
    if compressed_tokens > absolute_ceiling(budget_tokens) {
        CompressionOutcome::Reject
    } else if compressed_tokens > budget_tokens {
        CompressionOutcome::StoreWithWarning
    } else {
        CompressionOutcome::Store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PersistentMemory;
    use crate::token_estimate::estimate_json_tokens;
    use serde_json::json;

    fn prev(payload: serde_json::Value, updated_at: &str) -> PersistentMemory {
        PersistentMemory {
            id: GLOBAL_STATE_ID.to_string(),
            payload: serde_json::to_string(&payload).unwrap(),
            updated_at: updated_at.to_string(),
        }
    }

    // ─── Schema: shape and version ──────────────────────────────────────────

    /// The default budget is 800.
    #[test]
    fn budget_default_is_800() {
        assert_eq!(PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT, 800);
    }

    /// A valid version-1 payload is accepted without shape changes.
    #[test]
    fn valid_payload_v1_is_accepted_unchanged() {
        let candidate = json!({
            "schema_version": 1,
            "user_profile": {"city": "Madrid", "likes": ["té"]},
            "system_rules": ["sé breve", "no inventes"]
        });

        let normalized = validate_payload(&candidate).expect("valid payload must be accepted");
        assert_eq!(normalized, candidate);
    }

    /// A missing or non-integer `schema_version` is rejected.
    #[test]
    fn missing_or_non_integer_version_is_rejected() {
        assert_eq!(
            validate_payload(&json!({"user_profile": {}})),
            Err(SchemaError::InvalidSchemaVersion),
            "missing version must be rejected"
        );
        assert_eq!(
            validate_payload(&json!({"schema_version": "1"})),
            Err(SchemaError::InvalidSchemaVersion),
            "string version must be rejected"
        );
        assert_eq!(
            validate_payload(&json!({"schema_version": 1.5})),
            Err(SchemaError::InvalidSchemaVersion),
            "non-integer version must be rejected"
        );
    }

    /// A version other than `1` is rejected (previous state is preserved by the
    /// caller: this function writes nothing).
    #[test]
    fn other_version_is_rejected() {
        assert_eq!(
            validate_payload(&json!({"schema_version": 2, "user_profile": {}})),
            Err(SchemaError::UnsupportedSchemaVersion(2))
        );
    }

    /// A non-object payload is rejected.
    #[test]
    fn non_object_payload_is_rejected() {
        assert_eq!(
            validate_payload(&json!([1, 2, 3])),
            Err(SchemaError::NotAnObject)
        );
        assert_eq!(
            validate_payload(&json!("nope")),
            Err(SchemaError::NotAnObject)
        );
    }

    /// Unknown top-level keys are discarded; the rest is preserved.
    #[test]
    fn unknown_top_level_keys_are_discarded() {
        let candidate = json!({
            "schema_version": 1,
            "scratchpad": "no permitido",
            "user_profile": {"name": "Ada"},
            "system_rules": ["sé breve"]
        });

        let normalized = validate_payload(&candidate).expect("accepted");
        let object = normalized.as_object().expect("object");
        assert!(
            !object.contains_key("scratchpad"),
            "unknown key must be discarded"
        );
        assert_eq!(object.get("schema_version"), Some(&json!(1)));
        assert_eq!(object.get("user_profile"), Some(&json!({"name": "Ada"})));
        assert_eq!(object.get("system_rules"), Some(&json!(["sé breve"])));

        // The closed set is exactly the allowed keys.
        for key in object.keys() {
            assert!(
                ALLOWED_TOP_LEVEL_KEYS.contains(&key.as_str()),
                "unexpected surviving key: {key}"
            );
        }
    }

    /// A permitted key with the wrong type is rejected.
    #[test]
    fn wrong_shapes_are_rejected() {
        assert_eq!(
            validate_payload(&json!({"schema_version": 1, "user_profile": "nope"})),
            Err(SchemaError::InvalidShape("user_profile".into()))
        );
        assert_eq!(
            validate_payload(&json!({"schema_version": 1, "system_rules": {"a": 1}})),
            Err(SchemaError::InvalidShape("system_rules".into()))
        );
        assert_eq!(
            validate_payload(&json!({"schema_version": 1, "system_rules": ["ok", 7]})),
            Err(SchemaError::InvalidShape("system_rules".into())),
            "system_rules must be an array of strings"
        );
    }

    /// D3 — there are no count caps: no section is truncated or dropped,
    /// however many entries it has.
    #[test]
    fn no_count_caps_on_any_section() {
        let rules: Vec<serde_json::Value> = (0..200).map(|i| json!(format!("regla {i}"))).collect();
        let mut profile = serde_json::Map::new();
        for i in 0..100 {
            profile.insert(format!("hecho_{i}"), json!(i));
        }
        let candidate = json!({
            "schema_version": 1,
            "user_profile": profile,
            "system_rules": rules,
        });

        let normalized = validate_payload(&candidate).expect("accepted");
        let object = normalized.as_object().unwrap();
        assert_eq!(
            object
                .get("system_rules")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(200),
            "no rule may be dropped by a count cap"
        );
        assert_eq!(
            object
                .get("user_profile")
                .and_then(|v| v.as_object())
                .map(|m| m.len()),
            Some(100),
            "no profile entry may be dropped by a count cap"
        );
    }

    // ─── Timestamp: content hash and resolve_updated_at ─────────────────────

    /// The content hash ignores the `updated_at` key and is order-independent.
    #[test]
    fn content_hash_excludes_timestamp_and_ignores_key_order() {
        let without = json!({"schema_version": 1, "user_profile": {"a": 1}});
        let with_date = json!({
            "updated_at": "1999-01-01T00:00:00Z",
            "schema_version": 1,
            "user_profile": {"a": 1}
        });
        assert_eq!(
            content_hash(&without),
            content_hash(&with_date),
            "the timestamp must not count as content"
        );

        let reordered = json!({"user_profile": {"a": 1}, "schema_version": 1});
        assert_eq!(
            content_hash(&without),
            content_hash(&reordered),
            "key order must not change the hash"
        );

        let changed = json!({"schema_version": 1, "user_profile": {"a": 2}});
        assert_ne!(
            content_hash(&without),
            content_hash(&changed),
            "different content must hash differently"
        );
    }

    /// A new state seals `now`.
    #[test]
    fn new_state_seals_now() {
        let candidate = json!({"schema_version": 1, "user_profile": {}});
        assert_eq!(
            resolve_updated_at(None, &candidate, "2026-10-03T00:00:00Z"),
            "2026-10-03T00:00:00Z"
        );
    }

    /// Unchanged content keeps the previous `updated_at`.
    #[test]
    fn unchanged_content_keeps_previous_timestamp() {
        let previous = prev(
            json!({"schema_version": 1, "user_profile": {"a": 1}}),
            "2026-09-01T10:00:00Z",
        );
        let candidate = json!({"schema_version": 1, "user_profile": {"a": 1}});

        assert_eq!(
            resolve_updated_at(Some(&previous), &candidate, "2026-10-03T00:00:00Z"),
            "2026-09-01T10:00:00Z"
        );
    }

    /// Changed content seals `now`.
    #[test]
    fn changed_content_seals_now() {
        let previous = prev(
            json!({"schema_version": 1, "user_profile": {"a": 1}}),
            "2026-09-01T10:00:00Z",
        );
        let candidate = json!({"schema_version": 1, "user_profile": {"a": 2}});

        assert_eq!(
            resolve_updated_at(Some(&previous), &candidate, "2026-10-03T00:00:00Z"),
            "2026-10-03T00:00:00Z"
        );
    }

    /// The date the LLM proposes is ignored: after validation it is not part of
    /// the state, so an otherwise-identical state keeps its previous timestamp
    /// and a new one seals `now`.
    #[test]
    fn llm_supplied_date_is_ignored() {
        let now = "2026-10-03T00:00:00Z";
        let llm_candidate = json!({
            "schema_version": 1,
            "user_profile": {"a": 1},
            "updated_at": "1999-01-01T00:00:00Z"
        });
        let validated = validate_payload(&llm_candidate).expect("accepted");
        assert!(
            validated.get("updated_at").is_none(),
            "the LLM date must not survive validation"
        );

        // Same content as the previous state → previous timestamp is kept,
        // never the LLM's date.
        let previous = prev(
            json!({"schema_version": 1, "user_profile": {"a": 1}}),
            "2026-09-01T10:00:00Z",
        );
        assert_eq!(
            resolve_updated_at(Some(&previous), &validated, now),
            "2026-09-01T10:00:00Z"
        );

        // New state → `now`, still not the LLM's date.
        assert_eq!(resolve_updated_at(None, &validated, now), now);
    }

    // ─── Bloque 4: presupuesto, techo y JSON minificado ─────────────────────

    /// `minified_json` is the compact serialization of the payload and always
    /// re-parses as valid JSON.
    #[test]
    fn minified_json_is_compact_and_valid() {
        let value = json!({"schema_version": 1, "user_profile": {"a": 1}});
        assert_eq!(
            minified_json(&value),
            r#"{"schema_version":1,"user_profile":{"a":1}}"#
        );

        let reparsed: serde_json::Value =
            serde_json::from_str(&minified_json(&value)).expect("valid JSON");
        assert_eq!(reparsed, value);
    }

    /// The token count is measured over the minified JSON with the JSON
    /// estimator: `payload_token_count` is exactly `estimate_json_tokens` of the
    /// minified form, for small and spaceless-blob states alike.
    #[test]
    fn payload_token_count_uses_the_json_estimator_on_the_minified_json() {
        let values = [
            json!({"schema_version": 1, "user_profile": {"a": 1}}),
            json!({"schema_version": 1, "user_profile": {"blob": "a".repeat(4000)}}),
            json!({"schema_version": 1, "system_rules": ["sé breve", "no inventes"]}),
        ];

        for value in values {
            let minified = minified_json(&value);
            assert_eq!(
                payload_token_count(&value),
                estimate_json_tokens(&minified),
                "Layer C must measure with estimate_json_tokens over the minified JSON"
            );
        }

        assert!(payload_token_count(&json!({"schema_version": 1, "user_profile": {"a": 1}})) > 0);

        // The old metric (markdown heuristic floored at chars/4) must no longer
        // be what Layer C uses: for a spaceless blob the estimator differs.
        let blob = json!({"schema_version": 1, "user_profile": {"blob": "a".repeat(4000)}});
        let blob_minified = minified_json(&blob);
        let old_metric = crate::models::message::estimate_markdown_tokens_heuristic(&blob_minified)
            .max(blob_minified.chars().count() / 4);
        assert_ne!(
            payload_token_count(&blob),
            old_metric,
            "the markdown heuristic must not be used for Layer C"
        );
    }

    /// `is_empty_state` is true only when neither `user_profile` nor
    /// `system_rules` carry anything.
    #[test]
    fn empty_state_detection() {
        assert!(is_empty_state(&json!({"schema_version": 1})));
        assert!(is_empty_state(
            &json!({"schema_version": 1, "user_profile": {}, "system_rules": []})
        ));
        assert!(!is_empty_state(
            &json!({"schema_version": 1, "user_profile": {"city": "Madrid"}})
        ));
        assert!(!is_empty_state(
            &json!({"schema_version": 1, "system_rules": ["sé breve"]})
        ));
    }

    /// The absolute ceiling is twice the budget.
    #[test]
    fn absolute_ceiling_is_twice_the_budget() {
        assert_eq!(absolute_ceiling(500), 1000);
        assert_eq!(absolute_ceiling(0), 0);
    }

    // ─── Bloque 1.1: lectura compartida del presupuesto ──────────────────────

    async fn budget_db() -> sqlx::SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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

    /// A configured value is read as-is.
    #[tokio::test]
    async fn read_budget_returns_the_configured_value() {
        let pool = budget_db().await;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "PERSISTENT_MEMORY_BUDGET_TOKENS",
            "800",
        )
        .await
        .expect("set");

        assert_eq!(read_budget(&pool).await, 800);
    }

    /// A missing key falls back to the default.
    #[tokio::test]
    async fn read_budget_falls_back_when_missing() {
        let pool = budget_db().await;
        crate::db::repos::settings::SettingsRepo::delete(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS")
            .await
            .expect("delete");

        assert_eq!(
            read_budget(&pool).await,
            PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT
        );
    }

    /// An unparseable value falls back to the default.
    #[tokio::test]
    async fn read_budget_falls_back_when_unparseable() {
        let pool = budget_db().await;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "PERSISTENT_MEMORY_BUDGET_TOKENS",
            "not-a-number",
        )
        .await
        .expect("set");

        assert_eq!(
            read_budget(&pool).await,
            PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT
        );
    }

    /// After a (single) compression: within budget ⇒ store; over budget but
    /// under the ceiling ⇒ store with warning; over the ceiling ⇒ reject.
    #[test]
    fn compression_outcome_classifies_by_budget_and_ceiling() {
        assert_eq!(evaluate_compressed(400, 500), CompressionOutcome::Store);
        assert_eq!(evaluate_compressed(500, 500), CompressionOutcome::Store);
        assert_eq!(
            evaluate_compressed(800, 500),
            CompressionOutcome::StoreWithWarning
        );
        assert_eq!(
            evaluate_compressed(1000, 500),
            CompressionOutcome::StoreWithWarning
        );
        assert_eq!(evaluate_compressed(1001, 500), CompressionOutcome::Reject);
    }
}
