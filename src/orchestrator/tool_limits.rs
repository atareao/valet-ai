//! ReAct tool-call guard limits (`REACT_TOOL_*` settings).
//!
//! The four thresholds that govern the ReAct loop's tool-call guards live in
//! `settings` and are **read on every turn** (like the `GENERATION_*` ones) so
//! that editing them in the UI takes effect without a restart. This module
//! centralises the reading/parsing so the orchestrator behaves consistently.
//!
//! A missing key, an unparseable value or a database read error falls back to
//! the code default; the unparseable case also logs a `tracing::warn!`.

use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;

/// Compiled default for [`ToolLimits::fingerprint`].
const DEFAULT_FINGERPRINT_LIMIT: usize = 2;
/// Compiled default for [`ToolLimits::failure`].
const DEFAULT_FAILURE_LIMIT: usize = 3;
/// Compiled default for [`ToolLimits::success`].
const DEFAULT_SUCCESS_LIMIT: usize = 50;
/// Compiled default for [`ToolLimits::total`].
const DEFAULT_TOTAL_LIMIT: usize = 100;

/// Resolved ReAct tool-call guard limits for one turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLimits {
    /// Executions allowed of an identical call (same tool + `operation` + args),
    /// whatever the result. `REACT_TOOL_FINGERPRINT_LIMIT`.
    pub fingerprint: usize,
    /// Consecutive failures allowed per `tool::operation`. `REACT_TOOL_FAILURE_LIMIT`.
    pub failure: usize,
    /// Successful calls allowed per `tool::operation`. `REACT_TOOL_SUCCESS_LIMIT`.
    pub success: usize,
    /// Tool calls allowed in the whole turn. `REACT_TOOL_TOTAL_LIMIT`.
    pub total: usize,
}

/// Read and parse the four `REACT_TOOL_*` guard limits.
///
/// Reads from `settings` on **every call**. A missing key falls back to the
/// code default; an unparseable value does too, logging a `tracing::warn!`.
pub async fn read_tool_limits(pool: &SqlitePool) -> ToolLimits {
    ToolLimits {
        fingerprint: read_limit(
            pool,
            "REACT_TOOL_FINGERPRINT_LIMIT",
            DEFAULT_FINGERPRINT_LIMIT,
            "fingerprint",
        )
        .await,
        failure: read_limit(
            pool,
            "REACT_TOOL_FAILURE_LIMIT",
            DEFAULT_FAILURE_LIMIT,
            "failure",
        )
        .await,
        success: read_limit(
            pool,
            "REACT_TOOL_SUCCESS_LIMIT",
            DEFAULT_SUCCESS_LIMIT,
            "success",
        )
        .await,
        total: read_limit(pool, "REACT_TOOL_TOTAL_LIMIT", DEFAULT_TOTAL_LIMIT, "total").await,
    }
}

/// Read a single limit, falling back to `default` when the key is absent, its
/// value is not a valid `usize`, or the read itself fails (with a `warn!`).
async fn read_limit(pool: &SqlitePool, key: &str, default: usize, kind: &str) -> usize {
    let raw = match SettingsRepo::get(pool, key).await {
        Ok(Some(value)) => value.trim().to_string(),
        Ok(None) => return default,
        Err(e) => {
            tracing::warn!(
                error = %e,
                key = %key,
                "failed to read a react tool guard limit; using the default"
            );
            return default;
        }
    };

    raw.parse::<usize>().unwrap_or_else(|_| {
        tracing::warn!(
            key = %key,
            value = %raw,
            default = default,
            "react tool {kind} limit is not a valid number; using the default"
        );
        default
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::SettingsRepo;
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

    /// Scenario: un umbral ausente cae al default compilado.
    ///
    /// The four keys are not seeded here on purpose: even when they are absent
    /// the reader must return the code defaults.
    #[tokio::test]
    async fn missing_keys_fall_back_to_defaults() {
        let pool = db().await;
        for key in [
            "REACT_TOOL_FINGERPRINT_LIMIT",
            "REACT_TOOL_FAILURE_LIMIT",
            "REACT_TOOL_SUCCESS_LIMIT",
            "REACT_TOOL_TOTAL_LIMIT",
        ] {
            SettingsRepo::delete(&pool, key).await.expect("delete");
        }

        let limits = read_tool_limits(&pool).await;

        assert_eq!(
            limits,
            ToolLimits {
                fingerprint: 2,
                failure: 3,
                success: 50,
                total: 100,
            },
            "an absent limit must fall back to the compiled default"
        );
    }

    /// Scenario: la migración siembra exactamente los defaults compilados.
    ///
    /// Relates the Rust `DEFAULT_*` constants to the SQL migration so a future
    /// divergence between the two fails loudly instead of silently.
    #[tokio::test]
    async fn migration_seeds_the_compiled_defaults() {
        let pool = db().await;

        for (key, default) in [
            ("REACT_TOOL_FINGERPRINT_LIMIT", DEFAULT_FINGERPRINT_LIMIT),
            ("REACT_TOOL_FAILURE_LIMIT", DEFAULT_FAILURE_LIMIT),
            ("REACT_TOOL_SUCCESS_LIMIT", DEFAULT_SUCCESS_LIMIT),
            ("REACT_TOOL_TOTAL_LIMIT", DEFAULT_TOTAL_LIMIT),
        ] {
            assert_eq!(
                SettingsRepo::get(&pool, key).await.expect("get"),
                Some(default.to_string()),
                "settings.{key} seeded by the migration must equal the compiled default"
            );
        }
    }

    /// Scenario: un umbral no parseable cae al default (con `warn!`).
    #[tokio::test]
    async fn unparseable_value_falls_back_to_default() {
        let pool = db().await;
        SettingsRepo::set(&pool, "REACT_TOOL_SUCCESS_LIMIT", "muchos")
            .await
            .expect("set");

        let limits = read_tool_limits(&pool).await;

        assert_eq!(
            limits.success, 50,
            "an unparseable success limit must fall back to the default 50"
        );
    }
}
