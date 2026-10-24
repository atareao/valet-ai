use crate::db::repos::stats::StatsRepo;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::interval;

/// Run a periodic cleanup worker that purges old `llm_requests` records
/// based on the configured retention days.
///
/// The worker runs on an hourly interval. In each cycle it:
/// 1. Reads `stats_retention_days` from the settings table.
/// 2. Calls `StatsRepo::purge_old(pool, days)` to delete records older
///    than the retention period.
/// 3. Logs the number of deleted records.
///
/// Errors are logged as warnings and the loop continues.
pub async fn run_cleanup_worker(pool: SqlitePool) {
    let mut ticker = interval(Duration::from_secs(3600));
    loop {
        ticker.tick().await;
        match StatsRepo::get_retention_days(&pool).await {
            Ok(days) => match StatsRepo::purge_old(&pool, days).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!(
                            deleted = count,
                            retention_days = days,
                            "Stats cleanup: purged old llm_requests",
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Stats cleanup: purge_old failed");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Stats cleanup: failed to read retention days");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Helper: create an in-memory database with migrations and a default profile.
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

    /// Convenience: insert a single LLM request row with a specific `created_at`.
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

    // ── Test 1: purge_old removes records older than retention days ──────

    #[tokio::test]
    async fn test_cleanup_purges_old_data() {
        let pool = setup().await;

        // Fechas relativas al reloj: 45 y 5 días atrás, alrededor de la
        // retención de 30 días (antes eran literales con fecha fija).
        let old_date = (chrono::Utc::now() - chrono::Duration::days(45))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let recent_date = (chrono::Utc::now() - chrono::Duration::days(5))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();

        // Insert 5 old records (45 days ago)
        for i in 0..5 {
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
                Some(old_date.as_str()),
            )
            .await;
        }

        // Insert 3 recent records (5 days ago)
        for i in 0..3 {
            let id = format!("recent-{i}");
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
                Some(recent_date.as_str()),
            )
            .await;
        }

        // Verify total before purge
        let total_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total_before, 8, "should have 8 records before purge");

        // Act: purge with 30 days retention
        let deleted = StatsRepo::purge_old(&pool, 30).await.unwrap();

        // Assert: only the 5 old records (45 days ago) should be deleted
        assert_eq!(deleted, 5, "should purge exactly 5 old records");

        // Assert: only 3 recent records remain
        let total_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            total_after, 3,
            "should have 3 records remaining after purge"
        );
    }

    // ── Test 2: purge_old on empty table ─────────────────────────────────

    #[tokio::test]
    async fn test_cleanup_no_data() {
        let pool = setup().await;

        // Act: purge with 30 days retention on empty table
        let deleted = StatsRepo::purge_old(&pool, 30).await.unwrap();

        // Assert: no records were deleted
        assert_eq!(deleted, 0, "should purge 0 records from empty table");
    }
}
