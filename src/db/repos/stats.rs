use sqlx::Row;
use sqlx::SqlitePool;

use crate::models::stats::{
    BackgroundStats, CallKind, DayStats, MemoryStats, ModelStats, StatsSummary, TableSize,
    ToolStats,
};

/// Canary threshold for a suspiciously empty successful call.
///
/// A call recorded as `status = 'success'` whose completion produced at most
/// this many tokens is treated as a likely empty response (`content = null`)
/// masquerading as a success. The measured failure returns exactly 1 token, so
/// the threshold is 1: `<= 1` catches it without flagging legitimately short
/// structured outputs (a valid extractor JSON never drops to a single token).
const SUSPICIOUS_MAX_COMPLETION_TOKENS: i64 = 1;

/// Canonical non-chat origins, in the order the API presents them.
///
/// The order is deliberate, not alphabetical; it is fixed here in Rust because
/// the database cannot know it.
const CANONICAL_BACKGROUND_KINDS: [&str; 5] = [
    "router",
    "archivist",
    "consolidator",
    "collapse",
    "timeline",
];

/// Repository for LLM usage statistics and administrative operations.
pub struct StatsRepo;

impl StatsRepo {
    /// Global aggregate summary over all LLM requests.
    pub async fn summary(pool: &SqlitePool) -> Result<StatsSummary, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)                                               AS total_calls,
                COALESCE(SUM(prompt_tokens), 0)                        AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)                    AS total_completion_tokens,
                COALESCE(SUM(total_tokens), 0)                         AS total_tokens,
                COALESCE(SUM(cached_tokens), 0)                        AS total_cached_tokens,
                COALESCE(SUM(reasoning_tokens), 0)                     AS total_reasoning_tokens,
                COALESCE(SUM(cost), 0.0)                               AS total_cost,
                COALESCE(SUM(CASE WHEN status != 'success' THEN 1 ELSE 0 END), 0) AS total_errors,
                AVG(duration_ms)                                       AS avg_duration_ms
            FROM llm_requests
            WHERE kind = 'chat'
            "#,
        )
        .fetch_one(pool)
        .await?;

        Ok(StatsSummary {
            total_calls: row.get::<i64, _>(0) as u64,
            total_prompt_tokens: row.get::<i64, _>(1) as u64,
            total_completion_tokens: row.get::<i64, _>(2) as u64,
            total_tokens: row.get::<i64, _>(3) as u64,
            total_cached_tokens: row.get::<i64, _>(4) as u64,
            total_reasoning_tokens: row.get::<i64, _>(5) as u64,
            total_cost: row.get::<f64, _>(6),
            total_errors: row.get::<i64, _>(7) as u64,
            avg_duration_ms: row.get::<Option<f64>, _>(8),
        })
    }

    /// Per-model breakdown, ordered by total cost descending.
    ///
    /// Aggregates **every origin** — chat and background roles alike — so a
    /// model that only serves background work (e.g. the semantic model used by
    /// the consolidator) is not invisible. The `days` window filters
    /// `created_at >= datetime('now', '-N days')`.
    ///
    /// A window smaller than one day is clamped to one day (`days.max(1)`), so
    /// `days = 0` means "at least the last 24 hours" instead of collapsing to
    /// `created_at >= now` and silently discarding everything. The clamp lives
    /// here, at the repository boundary, so every caller is protected.
    pub async fn by_model(pool: &SqlitePool, days: u32) -> Result<Vec<ModelStats>, sqlx::Error> {
        let days = days.max(1);
        let rows = sqlx::query(
            r#"
            SELECT
                model,
                COUNT(*)                        AS calls,
                SUM(total_tokens)               AS total_tokens,
                SUM(cost)                       AS total_cost,
                AVG(duration_ms)                AS avg_duration_ms,
                SUM(cached_tokens)              AS total_cached_tokens,
                SUM(reasoning_tokens)           AS total_reasoning_tokens
            FROM llm_requests
            WHERE created_at >= datetime('now', '-' || ?1 || ' days')
            GROUP BY model
            ORDER BY total_cost DESC
            "#,
        )
        .bind(days as i64)
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|r| ModelStats {
                model: r.get(0),
                calls: r.get::<i64, _>(1) as u64,
                total_tokens: r.get::<i64, _>(2) as u64,
                total_cost: r.get::<f64, _>(3),
                avg_duration_ms: r.get::<Option<f64>, _>(4),
                total_cached_tokens: r.get::<i64, _>(5) as u64,
                total_reasoning_tokens: r.get::<i64, _>(6) as u64,
            })
            .collect();

        Ok(stats)
    }

    /// Daily time series for the last `days` days, ordered by date ascending.
    pub async fn by_day(pool: &SqlitePool, days: u32) -> Result<Vec<DayStats>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                DATE(created_at)                AS date,
                COUNT(*)                        AS calls,
                SUM(total_tokens)               AS total_tokens,
                SUM(cost)                       AS total_cost,
                SUM(cached_tokens)              AS total_cached_tokens,
                SUM(reasoning_tokens)           AS total_reasoning_tokens
            FROM llm_requests
            WHERE created_at >= datetime('now', '-' || ?1 || ' days')
              AND kind = 'chat'
            GROUP BY DATE(created_at)
            ORDER BY date ASC
            "#,
        )
        .bind(days as i64)
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|r| DayStats {
                date: r.get(0),
                calls: r.get::<i64, _>(1) as u64,
                total_tokens: r.get::<i64, _>(2) as u64,
                total_cost: r.get::<f64, _>(3),
                total_cached_tokens: r.get::<i64, _>(4) as u64,
                total_reasoning_tokens: r.get::<i64, _>(5) as u64,
            })
            .collect();

        Ok(stats)
    }

    /// Frequency of tool calls across all LLM requests.
    ///
    /// Parses the `tool_calls` JSON column (an array of objects with a `name`
    /// field, e.g. `[{"name":"get_weather"}, {"name":"search_web"}]`).
    pub async fn tools_summary(pool: &SqlitePool) -> Result<Vec<ToolStats>, sqlx::Error> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT tool_calls FROM llm_requests WHERE tool_calls IS NOT NULL AND kind = 'chat'",
        )
        .fetch_all(pool)
        .await?;

        let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

        for json_str in &rows {
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                for val in &arr {
                    if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                        *counts.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut result: Vec<ToolStats> = counts
            .into_iter()
            .map(|(tool, count)| ToolStats { tool, count })
            .collect();

        // Sort by count descending for consistent output
        result.sort_by_key(|b| std::cmp::Reverse(b.count));

        Ok(result)
    }

    /// Row counts for all domain tables.
    ///
    /// Includes the primary content tables (excluding FTS virtual tables and
    /// internal helper tables).
    pub async fn db_sizes(pool: &SqlitePool) -> Result<Vec<TableSize>, sqlx::Error> {
        let tables = [
            "events",
            "llm_requests",
            "memory",
            "message_embeddings",
            "messages",
            "notes",
            "profiles",
            "reminders",
            "settings",
            "tasks",
            "timeline_events",
            "tools",
        ];

        let mut sizes = Vec::with_capacity(tables.len());

        for table in &tables {
            let sql = format!("SELECT COUNT(*) FROM \"{table}\"");
            // SAFETY: `table` comes from the hard-coded `tables` array above.
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(pool)
                .await?;
            sizes.push(TableSize {
                table: table.to_string(),
                rows: count as u64,
            });
        }

        Ok(sizes)
    }

    /// Export all LLM request records as a CSV string.
    ///
    /// The CSV includes a header row followed by one row per record.
    pub async fn export_csv(pool: &SqlitePool) -> Result<String, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, model, provider, profile_id,
                prompt_tokens, completion_tokens, total_tokens,
                cached_tokens, reasoning_tokens,
                cost, is_byok, duration_ms, cache_hit,
                status, error_message, tool_calls, kind, created_at
            FROM llm_requests
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let mut csv = String::from(
            "id,model,provider,profile_id,prompt_tokens,completion_tokens,total_tokens,",
        );
        csv.push_str("cached_tokens,reasoning_tokens,cost,is_byok,duration_ms,cache_hit,");
        csv.push_str("status,error_message,tool_calls,kind,created_at\n");

        for r in &rows {
            // Helper to quote a value for CSV
            let quote = |s: &str| {
                if s.contains(',') || s.contains('"') || s.contains('\n') {
                    format!("\"{}\"", s.replace('"', "\"\""))
                } else {
                    s.to_string()
                }
            };

            let id: String = r.get(0);
            let model: String = r.get(1);
            let provider: Option<String> = r.get(2);
            let profile_id: Option<String> = r.get(3);
            let prompt_tokens: i64 = r.get(4);
            let completion_tokens: i64 = r.get(5);
            let total_tokens: i64 = r.get(6);
            let cached_tokens: i64 = r.get(7);
            let reasoning_tokens: i64 = r.get(8);
            let cost: f64 = r.get(9);
            let is_byok: bool = r.get::<i64, _>(10) != 0;
            let duration_ms: Option<i64> = r.get(11);
            let cache_hit: bool = r.get::<i64, _>(12) != 0;
            let status: String = r.get(13);
            let error_message: Option<String> = r.get(14);
            let tool_calls: Option<String> = r.get(15);
            let kind: String = r.get(16);
            let created_at: String = r.get(17);

            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                quote(&id),
                quote(&model),
                provider.as_deref().unwrap_or(""),
                profile_id.as_deref().unwrap_or(""),
                prompt_tokens,
                completion_tokens,
                total_tokens,
                cached_tokens,
                reasoning_tokens,
                cost,
                if is_byok { 1 } else { 0 },
                duration_ms.map_or_else(String::new, |v| v.to_string()),
                if cache_hit { 1 } else { 0 },
                quote(&status),
                quote(error_message.as_deref().unwrap_or("")),
                quote(tool_calls.as_deref().unwrap_or("")),
                quote(&kind),
                quote(&created_at),
            ));
        }

        Ok(csv)
    }

    /// Record an LLM request in the `llm_requests` table.
    ///
    /// Inserts a row with the given parameters. If `created_at` is `None`,
    /// the database will assign `datetime('now')` automatically via the
    /// column default. `kind` records the origin of the call.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_request(
        pool: &SqlitePool,
        kind: CallKind,
        id: &str,
        model: &str,
        profile_id: Option<&str>,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        cached_tokens: i64,
        reasoning_tokens: i64,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        error_message: Option<&str>,
        tool_calls: Option<&str>,
        created_at: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO llm_requests (id, model, provider, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, is_byok, duration_ms, cache_hit, status, error_message, tool_calls, kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, COALESCE(?18, datetime('now')))",
        )
        .bind(id)
        .bind(model)
        .bind(Option::<String>::None) // provider
        .bind(profile_id)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(cached_tokens)
        .bind(reasoning_tokens)
        .bind(cost)
        .bind(0i64) // is_byok
        .bind(duration_ms)
        .bind(0i64) // cache_hit
        .bind(status)
        .bind(error_message)
        .bind(tool_calls)
        .bind(kind.as_str())
        .bind(created_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Aggregate LLM usage per (non-chat origin, model) pair.
    ///
    /// Returns one [`BackgroundStats`] per pair of a canonical background
    /// origin (`router`, `archivist`, `consolidator`, `collapse`, `timeline`)
    /// and the model that served it. A canonical origin with no rows is still
    /// reported — counters at zero, `model: None` — so a role that never runs
    /// stays visible. Rows are ordered by canonical origin and, within an
    /// origin, by total cost descending. The `days` window filters
    /// `created_at >= datetime('now', '-N days')`.
    ///
    /// A window smaller than one day is clamped to one day (`days.max(1)`), so
    /// `days = 0` means "at least the last 24 hours" instead of collapsing to
    /// `created_at >= now` and silently discarding everything. The clamp lives
    /// here, at the repository boundary, so every caller is protected.
    pub async fn background_summary(
        pool: &SqlitePool,
        days: u32,
    ) -> Result<Vec<BackgroundStats>, sqlx::Error> {
        let days = days.max(1);
        let rows = sqlx::query(
            r#"
            SELECT
                kind,
                model,
                COUNT(*)                                        AS calls,
                COALESCE(SUM(prompt_tokens), 0)                 AS input_tokens,
                COALESCE(SUM(completion_tokens), 0)             AS output_tokens,
                COALESCE(SUM(total_tokens), 0)                  AS total_tokens,
                COALESCE(SUM(cost), 0.0)                        AS total_cost,
                COALESCE(SUM(CASE WHEN status != 'success' THEN 1 ELSE 0 END), 0) AS total_errors,
                COALESCE(SUM(CASE WHEN status = 'success' AND completion_tokens <= ?2 THEN 1 ELSE 0 END), 0) AS suspicious_calls,
                AVG(duration_ms)                                AS avg_duration_ms,
                MAX(created_at)                                 AS last_call_at
            FROM llm_requests
            WHERE kind != 'chat'
              AND created_at >= datetime('now', '-' || ?1 || ' days')
            GROUP BY kind, model
            "#,
        )
        .bind(days as i64)
        .bind(SUSPICIOUS_MAX_COMPLETION_TOKENS)
        .fetch_all(pool)
        .await?;

        // Group the real pairs by canonical origin, discarding any origin that
        // is not canonical, then emit them in canonical order — filling an
        // origin with no rows with a single zeroed, model-less entry.
        let mut by_kind: Vec<Vec<BackgroundStats>> = CANONICAL_BACKGROUND_KINDS
            .iter()
            .map(|_| Vec::new())
            .collect();

        for row in &rows {
            let kind: String = row.get(0);
            let Some(idx) = CANONICAL_BACKGROUND_KINDS.iter().position(|k| *k == kind) else {
                continue;
            };
            by_kind[idx].push(BackgroundStats {
                kind,
                model: row.get::<Option<String>, _>(1),
                calls: row.get::<i64, _>(2) as u64,
                input_tokens: row.get::<i64, _>(3) as u64,
                output_tokens: row.get::<i64, _>(4) as u64,
                total_tokens: row.get::<i64, _>(5) as u64,
                total_cost: row.get::<f64, _>(6),
                total_errors: row.get::<i64, _>(7) as u64,
                suspicious_calls: row.get::<i64, _>(8) as u64,
                avg_duration_ms: row.get::<Option<f64>, _>(9),
                last_call_at: row.get::<Option<String>, _>(10),
            });
        }

        let mut stats = Vec::with_capacity(CANONICAL_BACKGROUND_KINDS.len());
        for (idx, kind) in CANONICAL_BACKGROUND_KINDS.iter().enumerate() {
            let mut pairs = std::mem::take(&mut by_kind[idx]);
            if pairs.is_empty() {
                // The origin never ran: keep it visible at zero.
                stats.push(BackgroundStats {
                    kind: (*kind).to_string(),
                    model: None,
                    calls: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                    total_cost: 0.0,
                    total_errors: 0,
                    suspicious_calls: 0,
                    avg_duration_ms: None,
                    last_call_at: None,
                });
            } else {
                pairs.sort_by(|a, b| {
                    b.total_cost
                        .partial_cmp(&a.total_cost)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                stats.append(&mut pairs);
            }
        }

        Ok(stats)
    }

    /// Purge LLM request records older than `days` days.
    ///
    /// Returns the number of deleted rows.
    pub async fn purge_old(pool: &SqlitePool, days: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM llm_requests WHERE created_at < datetime('now', '-' || ?1 || ' days')",
        )
        .bind(days as i64)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Read the retention days setting.
    ///
    /// Returns the value of the `stats_retention_days` setting if present,
    /// otherwise returns the default of 30.
    pub async fn get_retention_days(pool: &SqlitePool) -> Result<u32, sqlx::Error> {
        let row: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'stats_retention_days'")
                .fetch_optional(pool)
                .await?;

        match row {
            Some(val) => val.parse::<u32>().or(Ok(30)),
            None => Ok(30),
        }
    }

    /// Save the retention days setting.
    pub async fn set_retention_days(pool: &SqlitePool, days: u32) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES ('stats_retention_days', ?1, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(days.to_string())
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Aggregate statistics over episodic memory.
    pub async fn memory_summary(pool: &SqlitePool) -> Result<MemoryStats, sqlx::Error> {
        let total_memories: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory")
            .fetch_one(pool)
            .await?;
        let total_tokens: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(tokens_count), 0) FROM memory")
                .fetch_one(pool)
                .await?;
        let messages_indexed: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE is_indexed = 1")
                .fetch_one(pool)
                .await?;
        let messages_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
            .fetch_one(pool)
            .await?;

        Ok(MemoryStats {
            total_memories: total_memories as u64,
            total_tokens: total_tokens as u64,
            messages_indexed: messages_indexed as u64,
            messages_total: messages_total as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Helper to create an in-memory database with all migrations applied and a
    /// default profile inserted (needed for FK constraints on llm_requests).
    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("failed to create in-memory pool");

        crate::db::schema::run_migrations(&pool)
            .await
            .expect("failed to run migrations");

        // Seed a default profile for FK references
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await
        .expect("failed to seed profile");

        pool
    }

    /// Convenience: insert a single LLM request row with the given parameters.
    #[allow(clippy::too_many_arguments)]
    async fn insert_request(
        pool: &SqlitePool,
        id: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        cached_tokens: i64,
        reasoning_tokens: i64,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        error_message: Option<&str>,
        tool_calls: Option<&str>,
        created_at: Option<&str>,
    ) {
        let q = sqlx::query(
            "INSERT INTO llm_requests (id, model, provider, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, is_byok, duration_ms, cache_hit, status, error_message, tool_calls, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, COALESCE(?17, datetime('now')))",
        )
        .bind(id)
        .bind(model)
        .bind(Option::<String>::None) // provider
        .bind("profile-1")
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(cached_tokens)
        .bind(reasoning_tokens)
        .bind(cost)
        .bind(0i64) // is_byok
        .bind(duration_ms)
        .bind(0i64) // cache_hit
        .bind(status)
        .bind(error_message)
        .bind(tool_calls)
        .bind(created_at);

        q.execute(pool).await.unwrap();
    }

    /// Convenience: insert a single LLM request row with an explicit `kind`.
    #[allow(clippy::too_many_arguments)]
    async fn insert_request_with_kind(
        pool: &SqlitePool,
        id: &str,
        kind: &str,
        model: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        tool_calls: Option<&str>,
        created_at: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO llm_requests (id, model, provider, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, is_byok, duration_ms, cache_hit, status, error_message, tool_calls, kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, COALESCE(?18, datetime('now')))",
        )
        .bind(id)
        .bind(model)
        .bind(Option::<String>::None) // provider
        .bind("profile-1")
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(0i64) // cached_tokens
        .bind(0i64) // reasoning_tokens
        .bind(cost)
        .bind(0i64) // is_byok
        .bind(duration_ms)
        .bind(0i64) // cache_hit
        .bind(status)
        .bind(Option::<String>::None) // error_message
        .bind(tool_calls)
        .bind(kind)
        .bind(created_at)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Like [`setup`] but with SQLite foreign-key enforcement enabled, matching
    /// production (`db::init_db` sets `foreign_keys(true)`).
    async fn setup_with_fk() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true)
                    .create_if_missing(true),
            )
            .await
            .expect("failed to create in-memory pool");

        crate::db::schema::run_migrations(&pool)
            .await
            .expect("failed to run migrations");

        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await
        .expect("failed to seed profile");

        pool
    }

    // ── record_request tests ────────────────────────────────────────────

    /// A system operation (worker) records stats with `profile_id = NULL`.
    /// With foreign keys enabled, a NULL must not fail the FK check and the
    /// row must actually be inserted.
    #[tokio::test]
    async fn test_record_request_with_null_profile_id() {
        let pool = setup_with_fk().await;

        StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-null",
            "gpt-4o",
            None,
            100,
            50,
            150,
            0,
            0,
            0.0,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await
        .expect("recording with profile_id = NULL must not violate the FK");

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests WHERE id = 'req-null'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "the row must actually be inserted");

        let profile: Option<String> =
            sqlx::query_scalar("SELECT profile_id FROM llm_requests WHERE id = 'req-null'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(profile.is_none(), "profile_id must be NULL");
    }

    /// A bogus literal such as `"background"` (which does not exist in
    /// `profiles`) must be rejected when foreign keys are enabled — proving the
    /// bug the workers used to hit.
    #[tokio::test]
    async fn test_record_request_with_unknown_profile_id_violates_fk() {
        let pool = setup_with_fk().await;

        let result = StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-bad",
            "gpt-4o",
            Some("background"),
            0,
            0,
            0,
            0,
            0,
            0.0,
            None,
            "error",
            None,
            None,
            None,
        )
        .await;

        assert!(
            result.is_err(),
            "an unknown profile_id must violate the FK when foreign keys are on"
        );
    }

    #[tokio::test]
    async fn test_record_request_inserts_row() {
        let pool = setup().await;

        StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-1",
            "gpt-4o",
            Some("profile-1"),
            100,
            50,
            150,
            0,
            0,
            0.0,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let row = sqlx::query("SELECT id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, duration_ms, status, error_message, tool_calls, created_at FROM llm_requests WHERE id = 'req-1'")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row.get::<String, _>(0), "req-1");
        assert_eq!(row.get::<String, _>(1), "gpt-4o");
        assert_eq!(row.get::<String, _>(2), "profile-1");
        assert_eq!(row.get::<i64, _>(3), 100);
        assert_eq!(row.get::<i64, _>(4), 50);
        assert_eq!(row.get::<i64, _>(5), 150);
        assert_eq!(row.get::<i64, _>(6), 0);
        assert_eq!(row.get::<i64, _>(7), 0);
        assert!((row.get::<f64, _>(8) - 0.0).abs() < f64::EPSILON);
        assert_eq!(row.get::<Option<i64>, _>(9), Some(200));
        assert_eq!(row.get::<String, _>(10), "success");
        assert!(row.get::<Option<String>, _>(11).is_none());
        assert!(row.get::<Option<String>, _>(12).is_none());
        assert!(row.get::<String, _>(13).len() >= 19); // created_at not null, ISO format
    }

    #[tokio::test]
    async fn test_record_request_with_tool_calls() {
        let pool = setup().await;

        let tool_calls_json = r#"{"model":"gpt-4o","tool_calls":[{"name":"get_weather"}]}"#;

        StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-tc",
            "gpt-4o",
            Some("profile-1"),
            100,
            50,
            150,
            0,
            0,
            0.01,
            None,
            "success",
            None,
            Some(tool_calls_json),
            None,
        )
        .await
        .unwrap();

        let saved: Option<String> =
            sqlx::query_scalar("SELECT tool_calls FROM llm_requests WHERE id = 'req-tc'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(saved, Some(tool_calls_json.to_string()));
    }

    #[tokio::test]
    async fn test_record_request_with_error() {
        let pool = setup().await;

        StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-err",
            "gpt-4o",
            Some("profile-1"),
            100,
            50,
            150,
            0,
            0,
            0.0,
            None,
            "error",
            Some("timeout"),
            None,
            None,
        )
        .await
        .unwrap();

        let row =
            sqlx::query("SELECT status, error_message FROM llm_requests WHERE id = 'req-err'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(row.get::<String, _>(0), "error");
        assert_eq!(row.get::<Option<String>, _>(1), Some("timeout".to_string()));
    }

    #[tokio::test]
    async fn test_record_request_auto_created_at() {
        let pool = setup().await;

        StatsRepo::record_request(
            &pool,
            CallKind::Chat,
            "req-auto",
            "gpt-4o",
            Some("profile-1"),
            100,
            50,
            150,
            0,
            0,
            0.0,
            None,
            "success",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let created_at: String =
            sqlx::query_scalar("SELECT created_at FROM llm_requests WHERE id = 'req-auto'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert!(
            !created_at.is_empty(),
            "created_at should be auto-assigned and non-empty"
        );
    }

    // ── 1. summary con datos variados ─────────────────────────────────────

    #[tokio::test]
    async fn test_summary_with_mixed_data() {
        let pool = setup().await;

        // Two successful requests
        insert_request(
            &pool,
            "req-1",
            "gpt-4o",
            100,
            50,
            150,
            10,
            10,
            0.01,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &pool,
            "req-2",
            "gpt-4o",
            200,
            100,
            300,
            20,
            10,
            0.02,
            Some(300),
            "success",
            None,
            None,
            None,
        )
        .await;
        // One error
        insert_request(
            &pool,
            "req-3",
            "claude-3",
            0,
            0,
            0,
            0,
            0,
            0.0,
            None,
            "error",
            Some("timeout"),
            None,
            None,
        )
        .await;

        let s = StatsRepo::summary(&pool).await.unwrap();

        assert_eq!(s.total_calls, 3);
        assert_eq!(s.total_prompt_tokens, 300);
        assert_eq!(s.total_completion_tokens, 150);
        assert_eq!(s.total_tokens, 450);
        assert_eq!(s.total_cached_tokens, 30);
        assert_eq!(s.total_reasoning_tokens, 20);
        assert!((s.total_cost - 0.03).abs() < f64::EPSILON);
        assert_eq!(s.total_errors, 1);
        // avg_duration_ms = (200 + 300 + NULL) / 2 = 250.0
        assert!((s.avg_duration_ms.unwrap() - 250.0).abs() < f64::EPSILON);
    }

    // ── 2. summary vacío ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_summary_empty() {
        let pool = setup().await;

        let s = StatsRepo::summary(&pool).await.unwrap();

        assert_eq!(s.total_calls, 0);
        assert_eq!(s.total_tokens, 0);
        assert_eq!(s.total_cost, 0.0);
        assert_eq!(s.total_errors, 0);
        assert!(s.avg_duration_ms.is_none());
    }

    // ── 3. by_model ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_by_model() {
        let pool = setup().await;

        // 2 calls gpt-4o (cost 0.02 + 0.01 = 0.03), 1 call claude-3 (cost 0.04)
        insert_request(
            &pool,
            "r1",
            "gpt-4o",
            100,
            50,
            150,
            10,
            5,
            0.02,
            Some(200),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &pool,
            "r2",
            "gpt-4o",
            50,
            25,
            75,
            5,
            3,
            0.01,
            Some(100),
            "success",
            None,
            None,
            None,
        )
        .await;
        insert_request(
            &pool,
            "r3",
            "claude-3",
            200,
            100,
            300,
            20,
            10,
            0.04,
            Some(400),
            "success",
            None,
            None,
            None,
        )
        .await;

        let models = StatsRepo::by_model(&pool, 30).await.unwrap();

        assert_eq!(models.len(), 2);
        // Ordered by total_cost DESC → claude-3 first (0.04) then gpt-4o (0.03)
        assert_eq!(models[0].model, "claude-3");
        assert_eq!(models[0].calls, 1);
        assert_eq!(models[0].total_cost, 0.04);
        assert_eq!(models[1].model, "gpt-4o");
        assert_eq!(models[1].calls, 2);
        assert!((models[1].total_cost - 0.03).abs() < f64::EPSILON);
    }

    // ── 4. by_day ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_by_day() {
        let pool = setup().await;

        // Insert one request per day for 7 days, ending yesterday
        for i in 1..=7 {
            let date = format!("2026-09-{:02}T10:00:00", 18 + i); // Sep 19..Sep 25
            let id = format!("day-req-{i}");
            insert_request(
                &pool,
                &id,
                "gpt-4o",
                100,
                50,
                150,
                10,
                5,
                0.01,
                Some(100),
                "success",
                None,
                None,
                Some(&date),
            )
            .await;
        }

        // by_day with days=30 should include all 7
        let days = StatsRepo::by_day(&pool, 30).await.unwrap();
        assert_eq!(days.len(), 7);
        // Ordered ascending
        for i in 0..days.len() - 1 {
            assert!(days[i].date <= days[i + 1].date);
        }
    }

    // ── 5. tools_summary ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_tools_summary() {
        let pool = setup().await;

        // 3 calls with get_weather
        for i in 0..3 {
            insert_request(
                &pool,
                &format!("tw-{i}"),
                "gpt-4o",
                100,
                50,
                150,
                0,
                0,
                0.01,
                None,
                "success",
                None,
                Some(r#"[{"name":"get_weather"}]"#),
                None,
            )
            .await;
        }
        // 2 calls with search_web
        for i in 0..2 {
            insert_request(
                &pool,
                &format!("ts-{i}"),
                "gpt-4o",
                100,
                50,
                150,
                0,
                0,
                0.01,
                None,
                "success",
                None,
                Some(r#"[{"name":"search_web"}]"#),
                None,
            )
            .await;
        }
        // 1 null tool_calls (should be ignored)
        insert_request(
            &pool, "t-null", "gpt-4o", 100, 50, 150, 0, 0, 0.01, None, "success", None, None, None,
        )
        .await;

        let tools = StatsRepo::tools_summary(&pool).await.unwrap();

        assert_eq!(tools.len(), 2);
        // Sorted by count desc: get_weather=3, search_web=2
        assert_eq!(tools[0].tool, "get_weather");
        assert_eq!(tools[0].count, 3);
        assert_eq!(tools[1].tool, "search_web");
        assert_eq!(tools[1].count, 2);
    }

    // ── 6. db_sizes ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_db_sizes() {
        let pool = setup().await;

        // Insert into various tables
        sqlx::query("INSERT INTO messages (id, role, content) VALUES ('m1', 'user', 'Hello')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO messages (id, role, content) VALUES ('m2', 'assistant', 'Hi')")
            .execute(&pool)
            .await
            .unwrap();

        let sizes = StatsRepo::db_sizes(&pool).await.unwrap();

        let messages_size = sizes.iter().find(|t| t.table == "messages").unwrap();
        assert_eq!(messages_size.rows, 2);

        let profiles_size = sizes.iter().find(|t| t.table == "profiles").unwrap();
        assert_eq!(profiles_size.rows, 1);
    }

    /// `db_sizes` must include the `timeline_events` table with its row count.
    #[tokio::test]
    async fn test_db_sizes_includes_timeline_events() {
        let pool = setup().await;

        sqlx::query(
            "INSERT INTO timeline_events (id, timestamp, category, fact) VALUES ('t1', '2026-10-10T09:00:00', 'sport', 'Ran 5km')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO timeline_events (id, timestamp, category, fact) VALUES ('t2', '2026-10-10T10:00:00', 'lifestyle', 'Bought groceries')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let sizes = StatsRepo::db_sizes(&pool).await.unwrap();

        let timeline_size = sizes
            .iter()
            .find(|t| t.table == "timeline_events")
            .expect("db_sizes must include the 'timeline_events' table");
        assert_eq!(timeline_size.rows, 2);
    }

    // ── 7. export_csv ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_export_csv() {
        let pool = setup().await;

        insert_request(
            &pool,
            "csv-1",
            "gpt-4o",
            100,
            50,
            150,
            10,
            5,
            0.01,
            Some(200),
            "success",
            None,
            Some(r#"[{"name":"get_weather"}]"#),
            Some("2026-09-24T10:00:00"),
        )
        .await;
        insert_request(
            &pool,
            "csv-2",
            "claude-3",
            200,
            100,
            300,
            20,
            10,
            0.02,
            None,
            "error",
            Some("timeout"),
            None,
            Some("2026-09-25T12:00:00"),
        )
        .await;

        let csv = StatsRepo::export_csv(&pool).await.unwrap();

        // Should have a header and 2 data lines
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert!(
            lines.len() >= 3,
            "expected header + 2 rows, got {}",
            lines.len()
        );

        let header = lines[0];
        assert!(header.starts_with("id,"));
        assert!(header.contains("model"));
        assert!(header.contains("status"));

        // Check first data row
        let row1 = lines[1];
        assert!(row1.starts_with("csv-1,"));
        assert!(row1.contains("gpt-4o"));

        // Check second row
        let row2 = lines[2];
        assert!(row2.starts_with("csv-2,"));
        assert!(row2.contains("claude-3"));
        assert!(row2.contains("timeout"));
    }

    // ── 8. purge_old ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_purge_old() {
        let pool = setup().await;

        // 10 old records (60 days ago)
        for i in 0..10 {
            let id = format!("old-{i}");
            insert_request(
                &pool,
                &id,
                "gpt-4o",
                10,
                5,
                15,
                0,
                0,
                0.001,
                None,
                "success",
                None,
                None,
                Some("2026-07-01T00:00:00"),
            )
            .await;
        }

        // 10 new records (today)
        for i in 0..10 {
            let id = format!("new-{i}");
            insert_request(
                &pool, &id, "gpt-4o", 10, 5, 15, 0, 0, 0.001, None, "success", None, None, None,
            )
            .await;
        }

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 20);

        // Purge records older than 30 days → should remove the 10 old ones
        let deleted = StatsRepo::purge_old(&pool, 30).await.unwrap();
        assert_eq!(deleted, 10);

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 10);
    }

    // ── 9. get_retention_days ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_retention_days_with_setting() {
        let pool = setup().await;

        // Insert setting
        sqlx::query("INSERT INTO settings (key, value) VALUES ('stats_retention_days', '45')")
            .execute(&pool)
            .await
            .unwrap();

        let days = StatsRepo::get_retention_days(&pool).await.unwrap();
        assert_eq!(days, 45);
    }

    #[tokio::test]
    async fn test_get_retention_days_default() {
        let pool = setup().await;

        // No setting inserted → should default to 30
        let days = StatsRepo::get_retention_days(&pool).await.unwrap();
        assert_eq!(days, 30);
    }

    // ── 10. set_retention_days ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_set_retention_days() {
        let pool = setup().await;

        StatsRepo::set_retention_days(&pool, 60).await.unwrap();

        let days = StatsRepo::get_retention_days(&pool).await.unwrap();
        assert_eq!(days, 60);

        // Update existing
        StatsRepo::set_retention_days(&pool, 90).await.unwrap();
        let days = StatsRepo::get_retention_days(&pool).await.unwrap();
        assert_eq!(days, 90);
    }

    // ── 11. memory_summary ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_memory_summary_empty() {
        let pool = setup().await;

        let s = StatsRepo::memory_summary(&pool).await.unwrap();
        assert_eq!(s.total_memories, 0);
        assert_eq!(s.total_tokens, 0);
        assert_eq!(s.messages_indexed, 0);
        assert_eq!(s.messages_total, 0);
    }

    #[tokio::test]
    async fn test_memory_summary_with_data() {
        let pool = setup().await;

        // Insert memories
        sqlx::query("INSERT INTO memory (id, content, tokens_count) VALUES ('m1', 'mem1', 100)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memory (id, content, tokens_count) VALUES ('m2', 'mem2', 200)")
            .execute(&pool)
            .await
            .unwrap();

        // Insert messages (one indexed)
        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('msg1', 'user', 'hello', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('msg2', 'assistant', 'hi', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, role, content, is_indexed) VALUES ('msg3', 'user', 'howdy', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let s = StatsRepo::memory_summary(&pool).await.unwrap();
        assert_eq!(s.total_memories, 2);
        assert_eq!(s.total_tokens, 300);
        assert_eq!(s.messages_indexed, 2);
        assert_eq!(s.messages_total, 3);
    }

    // ── 12. record_request persiste el kind ────────────────────────────────

    /// `record_request` must round-trip the origin it was given through the
    /// `kind` column for every [`CallKind`] variant.
    #[tokio::test]
    async fn test_record_request_persists_kind_for_each_origin() {
        let pool = setup().await;

        let kinds = [
            CallKind::Chat,
            CallKind::Router,
            CallKind::Archivist,
            CallKind::Consolidator,
            CallKind::Collapse,
        ];

        for (i, kind) in kinds.into_iter().enumerate() {
            let id = format!("kind-req-{i}");
            StatsRepo::record_request(
                &pool,
                kind,
                &id,
                "gpt-4o",
                Some("profile-1"),
                1,
                1,
                2,
                0,
                0,
                0.0,
                None,
                "success",
                None,
                None,
                None,
            )
            .await
            .unwrap();

            let stored: String = sqlx::query_scalar("SELECT kind FROM llm_requests WHERE id = ?1")
                .bind(&id)
                .fetch_one(&pool)
                .await
                .unwrap();

            assert_eq!(
                stored,
                kind.as_str(),
                "record_request must persist the kind for {kind:?}"
            );
        }
    }

    // ── 13. agregaciones de chat excluyen filas no-chat ────────────────────

    /// With one `chat` row and one `archivist` row, every chat aggregate must
    /// ignore the background row.
    #[tokio::test]
    async fn test_chat_aggregations_exclude_non_chat_rows() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "chat-1",
            "chat",
            "gpt-4o",
            100,
            50,
            150,
            0.01,
            Some(200),
            "success",
            Some(r#"[{"name":"get_weather"}]"#),
            None,
        )
        .await;

        insert_request_with_kind(
            &pool,
            "arch-1",
            "archivist",
            "mistralai/mistral-small-24b-instruct-2501",
            1000,
            500,
            1500,
            0.5,
            Some(300),
            "success",
            Some(r#"[{"name":"background_tool"}]"#),
            None,
        )
        .await;

        // summary ignores the archivist row
        let s = StatsRepo::summary(&pool).await.unwrap();
        assert_eq!(s.total_calls, 1, "summary must exclude non-chat rows");
        assert_eq!(
            s.total_tokens, 150,
            "summary must not count archivist tokens"
        );
        assert!(
            (s.total_cost - 0.01).abs() < f64::EPSILON,
            "summary must not count archivist cost"
        );

        // by_model now aggregates every origin, so both models are present.
        let models = StatsRepo::by_model(&pool, 30).await.unwrap();
        assert_eq!(
            models.len(),
            2,
            "by_model aggregates chat and background models"
        );
        let gpt = models.iter().find(|m| m.model == "gpt-4o").unwrap();
        assert_eq!(gpt.calls, 1);
        let mistral = models
            .iter()
            .find(|m| m.model == "mistralai/mistral-small-24b-instruct-2501")
            .expect("the archivist model must now appear in by_model");
        assert_eq!(mistral.calls, 1);

        // by_day ignores the archivist row
        let days = StatsRepo::by_day(&pool, 30).await.unwrap();
        let total_calls: u64 = days.iter().map(|d| d.calls).sum();
        assert_eq!(total_calls, 1, "by_day must exclude non-chat rows");
        let total_tokens: u64 = days.iter().map(|d| d.total_tokens).sum();
        assert_eq!(total_tokens, 150, "by_day must not count archivist tokens");

        // tools_summary ignores the archivist tool calls
        let tools = StatsRepo::tools_summary(&pool).await.unwrap();
        assert_eq!(
            tools.len(),
            1,
            "tools_summary must exclude non-chat tool calls"
        );
        assert_eq!(tools[0].tool, "get_weather");
        assert_eq!(tools[0].count, 1);
    }

    // ── 14. background_summary ─────────────────────────────────────────────

    /// `background_summary` returns exactly one entry per non-chat origin,
    /// aggregating the rows of each and leaving absent origins at zero.
    #[tokio::test]
    async fn test_background_summary_one_entry_per_origin() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "router-1",
            "router",
            "gpt-4o-mini",
            10,
            5,
            15,
            0.001,
            Some(50),
            "success",
            None,
            None,
        )
        .await;
        insert_request_with_kind(
            &pool,
            "router-2",
            "router",
            "gpt-4o-mini",
            20,
            10,
            30,
            0.002,
            Some(60),
            "success",
            None,
            None,
        )
        .await;
        insert_request_with_kind(
            &pool,
            "collapse-1",
            "collapse",
            "gpt-4o",
            30,
            15,
            45,
            0.003,
            Some(70),
            "error",
            None,
            None,
        )
        .await;
        // A chat row must never show up in the background summary.
        insert_request_with_kind(
            &pool,
            "chat-1",
            "chat",
            "gpt-4o",
            100,
            50,
            150,
            0.01,
            Some(200),
            "success",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 30).await.unwrap();
        assert_eq!(bg.len(), 5, "one entry per non-chat origin");

        let find = |kind: &str| {
            bg.iter()
                .find(|b| b.kind == kind)
                .unwrap_or_else(|| panic!("missing origin '{kind}'"))
        };

        let router = find("router");
        assert_eq!(router.calls, 2);
        assert_eq!(router.input_tokens, 30);
        assert_eq!(router.output_tokens, 15);
        assert_eq!(router.total_tokens, 45);
        assert!((router.total_cost - 0.003).abs() < f64::EPSILON);
        assert_eq!(router.total_errors, 0);
        assert!((router.avg_duration_ms.unwrap() - 55.0).abs() < f64::EPSILON);

        let collapse = find("collapse");
        assert_eq!(collapse.calls, 1);
        assert_eq!(collapse.total_tokens, 45);
        assert_eq!(collapse.total_errors, 1, "the collapse row failed");
        assert!((collapse.avg_duration_ms.unwrap() - 70.0).abs() < f64::EPSILON);

        let archivist = find("archivist");
        assert_eq!(archivist.calls, 0);
        assert_eq!(archivist.total_tokens, 0);
        assert_eq!(archivist.total_cost, 0.0);
        assert!(archivist.avg_duration_ms.is_none());

        let consolidator = find("consolidator");
        assert_eq!(consolidator.calls, 0);
        assert_eq!(consolidator.total_tokens, 0);
        assert!(consolidator.avg_duration_ms.is_none());
    }

    /// With only chat rows the five background origins are still returned, all
    /// at zero.
    #[tokio::test]
    async fn test_background_summary_no_background_calls() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "chat-1",
            "chat",
            "gpt-4o",
            100,
            50,
            150,
            0.01,
            Some(200),
            "success",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 30).await.unwrap();
        assert_eq!(bg.len(), 5, "one entry per non-chat origin");

        for kind in [
            "router",
            "archivist",
            "consolidator",
            "collapse",
            "timeline",
        ] {
            let entry = bg
                .iter()
                .find(|b| b.kind == kind)
                .unwrap_or_else(|| panic!("missing origin '{kind}'"));
            assert_eq!(entry.calls, 0, "origin '{kind}' must have zero calls");
            assert_eq!(entry.total_tokens, 0);
            assert_eq!(entry.total_cost, 0.0);
            assert_eq!(entry.total_errors, 0);
            assert!(entry.avg_duration_ms.is_none());
        }
    }

    /// `background_summary` must aggregate the `timeline` origin like any other
    /// background origin, counting calls and errors.
    #[tokio::test]
    async fn test_background_summary_counts_timeline_origin() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "timeline-1",
            "timeline",
            "gpt-4o-mini",
            10,
            5,
            15,
            0.001,
            Some(50),
            "success",
            None,
            None,
        )
        .await;
        insert_request_with_kind(
            &pool,
            "timeline-2",
            "timeline",
            "gpt-4o-mini",
            20,
            10,
            30,
            0.002,
            Some(60),
            "error",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 30).await.unwrap();

        let timeline = bg
            .iter()
            .find(|b| b.kind == "timeline")
            .expect("background_summary must include the 'timeline' origin");
        assert_eq!(timeline.calls, 2);
        assert_eq!(timeline.total_errors, 1);
    }

    // ── 15. export_csv incluye kind ────────────────────────────────────────

    /// `export_csv` must carry the `kind` column in the header and each row.
    #[tokio::test]
    async fn test_export_csv_includes_kind_column() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "csv-kind-1",
            "archivist",
            "gpt-4o",
            100,
            50,
            150,
            0.01,
            Some(200),
            "success",
            None,
            Some("2026-09-24T10:00:00"),
        )
        .await;

        let csv = StatsRepo::export_csv(&pool).await.unwrap();
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert!(
            lines.len() >= 2,
            "expected header + 1 row, got {}",
            lines.len()
        );

        let header: Vec<&str> = lines[0].split(',').collect();
        let kind_idx = header
            .iter()
            .position(|h| *h == "kind")
            .expect("CSV header must include a 'kind' column");

        let row: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(
            row[kind_idx], "archivist",
            "each CSV line must carry its kind value"
        );
    }

    // ── 16. background_summary: eje de modelo ──────────────────────────────

    /// Two models inside the same origin produce two rows, one per
    /// (origin, model) pair, ordered by cost descending within the origin.
    #[tokio::test]
    async fn test_background_summary_two_models_same_kind_two_rows() {
        let pool = setup().await;

        // 2 consolidator calls to deepseek, cost 0.01 each.
        for i in 0..2 {
            insert_request_with_kind(
                &pool,
                &format!("c-ds-{i}"),
                "consolidator",
                "deepseek/deepseek-v4.1-flash",
                100,
                1,
                101,
                0.01,
                Some(100),
                "success",
                None,
                None,
            )
            .await;
        }
        // 3 consolidator calls to qwen, cost 0.02 each.
        for i in 0..3 {
            insert_request_with_kind(
                &pool,
                &format!("c-qw-{i}"),
                "consolidator",
                "qwen/qwen3-235b-a22b-2507",
                200,
                654,
                854,
                0.02,
                Some(200),
                "success",
                None,
                None,
            )
            .await;
        }

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();

        let consolidator: Vec<_> = bg.iter().filter(|b| b.kind == "consolidator").collect();
        assert_eq!(
            consolidator.len(),
            2,
            "two models in one origin must yield two rows"
        );
        // qwen cost = 0.06 > deepseek cost = 0.02 → qwen first.
        assert_eq!(
            consolidator[0].model.as_deref(),
            Some("qwen/qwen3-235b-a22b-2507")
        );
        assert_eq!(consolidator[0].calls, 3);
        assert_eq!(
            consolidator[1].model.as_deref(),
            Some("deepseek/deepseek-v4.1-flash")
        );
        assert_eq!(consolidator[1].calls, 2);
    }

    /// The five canonical origins are always present. Those with no rows keep
    /// zeroed counters, `model: None` and `last_call_at: None`.
    #[tokio::test]
    async fn test_background_summary_zero_fill_and_model_none() {
        let pool = setup().await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        assert_eq!(bg.len(), 5, "one zero-filled entry per canonical origin");

        let kinds: Vec<&str> = bg.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "router",
                "archivist",
                "consolidator",
                "collapse",
                "timeline"
            ]
        );

        for entry in &bg {
            assert!(entry.model.is_none(), "{} must have no model", entry.kind);
            assert_eq!(entry.calls, 0);
            assert_eq!(entry.suspicious_calls, 0);
            assert!(entry.last_call_at.is_none());
        }
    }

    /// The final ordering is by canonical origin, not alphabetically.
    #[tokio::test]
    async fn test_background_summary_canonical_order() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool, "ord-tl", "timeline", "m", 1, 5, 6, 0.1, None, "success", None, None,
        )
        .await;
        insert_request_with_kind(
            &pool, "ord-ro", "router", "m", 1, 5, 6, 0.2, None, "success", None, None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        let kinds: Vec<&str> = bg.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "router",
                "archivist",
                "consolidator",
                "collapse",
                "timeline"
            ]
        );
    }

    /// Successful calls with `completion_tokens <= 1` count as suspicious;
    /// normal successful calls do not.
    #[tokio::test]
    async fn test_background_summary_suspicious_calls() {
        let pool = setup().await;

        for i in 0..4 {
            insert_request_with_kind(
                &pool,
                &format!("sus-{i}"),
                "consolidator",
                "deepseek/deepseek-v4.1-flash",
                100,
                1,
                101,
                0.01,
                Some(10),
                "success",
                None,
                None,
            )
            .await;
        }
        for i in 0..2 {
            insert_request_with_kind(
                &pool,
                &format!("normal-{i}"),
                "consolidator",
                "deepseek/deepseek-v4.1-flash",
                200,
                654,
                854,
                0.02,
                Some(20),
                "success",
                None,
                None,
            )
            .await;
        }

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        let consolidator = bg.iter().find(|b| b.kind == "consolidator").unwrap();
        assert_eq!(consolidator.suspicious_calls, 4);
        assert_eq!(
            consolidator.total_errors, 0,
            "the empty success is invisible"
        );
    }

    /// An error with one token counts as an error but never as suspicious.
    #[tokio::test]
    async fn test_background_summary_error_with_one_token_not_suspicious() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "err-1",
            "consolidator",
            "m",
            100,
            1,
            101,
            0.01,
            None,
            "error",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        let consolidator = bg.iter().find(|b| b.kind == "consolidator").unwrap();
        assert_eq!(consolidator.total_errors, 1);
        assert_eq!(consolidator.suspicious_calls, 0);
    }

    /// A normal successful call is never suspicious.
    #[tokio::test]
    async fn test_background_summary_success_normal_not_suspicious() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "ok-1",
            "consolidator",
            "m",
            100,
            100,
            200,
            0.01,
            None,
            "success",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        let consolidator = bg.iter().find(|b| b.kind == "consolidator").unwrap();
        assert_eq!(consolidator.suspicious_calls, 0);
    }

    /// Chat rows never appear in the background summary, so a one-token chat
    /// success can never be flagged as suspicious.
    #[tokio::test]
    async fn test_background_summary_chat_never_suspicious() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "chat-tiny",
            "chat",
            "m",
            10,
            1,
            11,
            0.0,
            None,
            "success",
            None,
            None,
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        assert!(bg.iter().all(|b| b.kind != "chat"));
        assert!(bg.iter().all(|b| b.suspicious_calls == 0));
    }

    /// `last_call_at` is the maximum `created_at` of the pair, and `None` for
    /// an origin with no rows.
    #[tokio::test]
    async fn test_background_summary_last_call_at() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "lc-a",
            "router",
            "A",
            1,
            1,
            2,
            0.01,
            None,
            "success",
            None,
            Some("2026-10-01T10:00:00"),
        )
        .await;
        insert_request_with_kind(
            &pool,
            "lc-b",
            "router",
            "A",
            1,
            1,
            2,
            0.01,
            None,
            "success",
            None,
            Some("2026-10-05T10:00:00"),
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 30).await.unwrap();
        let router = bg.iter().find(|b| b.kind == "router").unwrap();
        assert_eq!(router.last_call_at.as_deref(), Some("2026-10-05T10:00:00"));

        let archivist = bg.iter().find(|b| b.kind == "archivist").unwrap();
        assert!(archivist.last_call_at.is_none());
    }

    /// The `days` window discards rows older than the cutoff.
    #[tokio::test]
    async fn test_background_summary_days_window() {
        let pool = setup().await;

        // Recent (default `now`).
        insert_request_with_kind(
            &pool, "w-now", "router", "A", 1, 1, 2, 0.01, None, "success", None, None,
        )
        .await;
        // Old (60 days ago).
        insert_request_with_kind(
            &pool,
            "w-old",
            "router",
            "A",
            1,
            1,
            2,
            0.01,
            None,
            "success",
            None,
            Some("2026-08-01T00:00:00"),
        )
        .await;

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();
        let router = bg.iter().find(|b| b.kind == "router").unwrap();
        assert_eq!(router.calls, 1, "rows outside the window must be discarded");
    }

    // ── 17. by_model: agregación por modelo sobre todos los orígenes ───────

    /// A model that only works in a background origin must appear.
    #[tokio::test]
    async fn test_by_model_includes_background_only_model() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "bg-qwen-1",
            "consolidator",
            "qwen/qwen3-235b-a22b-2507",
            100,
            100,
            200,
            0.05,
            None,
            "success",
            None,
            None,
        )
        .await;

        let models = StatsRepo::by_model(&pool, 7).await.unwrap();
        let qwen = models
            .iter()
            .find(|m| m.model == "qwen/qwen3-235b-a22b-2507")
            .expect("a background-only model must appear in by_model");
        assert_eq!(qwen.calls, 1);
        assert_eq!(qwen.total_cost, 0.05);
    }

    /// Chat and background calls of the same model are aggregated together.
    #[tokio::test]
    async fn test_by_model_aggregates_chat_and_background() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool,
            "agg-chat",
            "chat",
            "mistralai/mistral-small-24b-instruct-2501",
            100,
            50,
            150,
            0.01,
            None,
            "success",
            None,
            None,
        )
        .await;
        insert_request_with_kind(
            &pool,
            "agg-arch",
            "archivist",
            "mistralai/mistral-small-24b-instruct-2501",
            100,
            50,
            150,
            0.02,
            None,
            "success",
            None,
            None,
        )
        .await;

        let models = StatsRepo::by_model(&pool, 7).await.unwrap();
        let merged: Vec<_> = models
            .iter()
            .filter(|m| m.model == "mistralai/mistral-small-24b-instruct-2501")
            .collect();
        assert_eq!(merged.len(), 1, "the same model must be one single row");
        assert_eq!(merged[0].calls, 2);
        assert!((merged[0].total_cost - 0.03).abs() < f64::EPSILON);
    }

    /// `by_model` honors the `days` window.
    #[tokio::test]
    async fn test_by_model_days_window() {
        let pool = setup().await;

        insert_request_with_kind(
            &pool, "bm-n1", "chat", "gpt-4o", 100, 50, 150, 0.01, None, "success", None, None,
        )
        .await;
        insert_request_with_kind(
            &pool, "bm-n2", "chat", "gpt-4o", 100, 50, 150, 0.01, None, "success", None, None,
        )
        .await;
        insert_request_with_kind(
            &pool,
            "bm-old",
            "chat",
            "gpt-4o",
            100,
            50,
            150,
            0.01,
            None,
            "success",
            None,
            Some("2026-08-01T00:00:00"),
        )
        .await;

        let models = StatsRepo::by_model(&pool, 7).await.unwrap();
        let gpt = models.iter().find(|m| m.model == "gpt-4o").unwrap();
        assert_eq!(gpt.calls, 2, "the old row must be outside the window");
    }

    // ── 18. days = 0 se trata como una ventana de un día ───────────────────

    /// A window smaller than one day is clamped to one day at the repository
    /// boundary, so `days = 0` means "the last 24 hours", not "nothing". A row
    /// 12 hours old is inside the window; a row 3 days old is outside. The
    /// clamp protects both `background_summary` and `by_model`.
    #[tokio::test]
    async fn test_days_zero_behaves_as_one_day() {
        let pool = setup().await;

        // 12 hours old → inside a one-day window.
        sqlx::query(
            "INSERT INTO llm_requests
                 (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cost, status, kind, created_at)
             VALUES ('z-recent', 'role-model', 'profile-1', 10, 10, 20, 0.01, 'success', 'router', datetime('now', '-12 hours'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // 3 days old → outside a one-day window but inside the default 30.
        sqlx::query(
            "INSERT INTO llm_requests
                 (id, model, profile_id, prompt_tokens, completion_tokens, total_tokens, cost, status, kind, created_at)
             VALUES ('z-old', 'role-model', 'profile-1', 10, 10, 20, 0.99, 'success', 'router', datetime('now', '-3 days'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Sanity: the 3-day-old row is really in the table.
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 2);

        // background_summary with days = 0 (clamped to 1) keeps only the recent row.
        let bg = StatsRepo::background_summary(&pool, 0).await.unwrap();
        let router = bg.iter().find(|b| b.kind == "router").unwrap();
        assert_eq!(
            router.calls, 1,
            "days = 0 must behave as one day, not as an empty window"
        );
        assert!(
            (router.total_cost - 0.01).abs() < f64::EPSILON,
            "only the recent row's cost must be counted"
        );

        // by_model is clamped the same way.
        let models = StatsRepo::by_model(&pool, 0).await.unwrap();
        let model = models
            .iter()
            .find(|m| m.model == "role-model")
            .expect("the recent row's model must survive a days = 0 window");
        assert_eq!(
            model.calls, 1,
            "days = 0 must behave as one day in by_model"
        );
    }

    // ── 19. background_summary descarta kinds no canónicos ─────────────────

    /// A row whose `kind` is not in the canonical list is silently discarded: it
    /// never appears in the output and it neither shifts nor corrupts the five
    /// canonical entries. The table's CHECK would reject `'weird'`, so
    /// check-constraint enforcement is disabled for the two inserts that
    /// deliberately plant the out-of-band row.
    #[tokio::test]
    async fn test_background_summary_discards_non_canonical_kind() {
        let pool = setup().await;

        // One canonical row...
        insert_request_with_kind(
            &pool,
            "canon-1",
            "router",
            "gpt-4o-mini",
            10,
            5,
            15,
            0.001,
            Some(50),
            "success",
            None,
            None,
        )
        .await;

        // ...and one row whose kind is not canonical. `PRAGMA
        // ignore_check_constraints` lets it past the column CHECK; it is
        // re-enabled right after so the rest of the test (and suite) is
        // unaffected.
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&pool)
            .await
            .unwrap();
        insert_request_with_kind(
            &pool,
            "weird-1",
            "weird",
            "gpt-4o",
            100,
            50,
            150,
            9.99,
            Some(10),
            "success",
            None,
            None,
        )
        .await;
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&pool)
            .await
            .unwrap();

        let bg = StatsRepo::background_summary(&pool, 7).await.unwrap();

        assert_eq!(bg.len(), 5, "a non-canonical kind must not add an entry");
        assert!(
            bg.iter().all(|b| b.kind != "weird"),
            "the non-canonical kind must be discarded"
        );
        let kinds: Vec<&str> = bg.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "router",
                "archivist",
                "consolidator",
                "collapse",
                "timeline"
            ],
            "the five canonical entries must be intact and in order"
        );

        // The discarded row must not have leaked into the canonical router entry.
        let router = bg.iter().find(|b| b.kind == "router").unwrap();
        assert_eq!(router.calls, 1, "the discarded row must not add calls");
        assert_eq!(router.total_tokens, 15);
        assert!(
            (router.total_cost - 0.001).abs() < f64::EPSILON,
            "the discarded row's cost must not leak into any canonical entry"
        );
    }
}
