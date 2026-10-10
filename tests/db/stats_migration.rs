use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

async fn setup() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();
    valet::db::schema::run_migrations(&pool).await.unwrap();
    pool
}

/// Asserts that `run_migrations` creates the `llm_requests` table
/// with the correct schema (all columns, constraints, defaults).
#[tokio::test]
async fn test_migrations_creates_llm_requests_table() {
    let pool = setup().await;

    // Verify table exists
    let has_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='llm_requests'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        has_table,
        "Expected 'llm_requests' table to exist after migration"
    );

    // Verify all expected columns exist with correct types
    #[derive(sqlx::FromRow)]
    struct ColumnInfo {
        name: String,
        notnull: bool,
        dflt_value: Option<String>,
        pk: bool,
    }

    let columns: Vec<ColumnInfo> =
        sqlx::query_as("SELECT * FROM pragma_table_info('llm_requests')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let col_map: std::collections::HashMap<&str, &ColumnInfo> =
        columns.iter().map(|c| (c.name.as_str(), c)).collect();

    // -- Primary key --
    assert!(col_map.contains_key("id"), "Column 'id' missing");
    assert!(col_map["id"].pk, "'id' should be PRIMARY KEY");

    // -- NOT NULL columns --
    for col in &[
        "model",
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "cached_tokens",
        "reasoning_tokens",
        "is_byok",
        "cache_hit",
        "status",
    ] {
        assert!(col_map.contains_key(col), "Column '{col}' missing");
        assert!(col_map[col].notnull, "Column '{col}' should be NOT NULL");
    }

    // -- Nullable columns --
    for col in &[
        "provider",
        "profile_id",
        "duration_ms",
        "error_message",
        "tool_calls",
    ] {
        assert!(col_map.contains_key(col), "Column '{col}' missing");
        assert!(!col_map[col].notnull, "Column '{col}' should be nullable");
    }

    // -- Default values --
    assert_eq!(
        col_map["prompt_tokens"].dflt_value.as_deref(),
        Some("0"),
        "prompt_tokens default should be 0"
    );
    assert_eq!(
        col_map["completion_tokens"].dflt_value.as_deref(),
        Some("0"),
        "completion_tokens default should be 0"
    );
    assert_eq!(
        col_map["total_tokens"].dflt_value.as_deref(),
        Some("0"),
        "total_tokens default should be 0"
    );
    assert_eq!(
        col_map["cached_tokens"].dflt_value.as_deref(),
        Some("0"),
        "cached_tokens default should be 0"
    );
    assert_eq!(
        col_map["reasoning_tokens"].dflt_value.as_deref(),
        Some("0"),
        "reasoning_tokens default should be 0"
    );
    assert_eq!(
        col_map["cost"].dflt_value.as_deref(),
        Some("0.0"),
        "cost default should be 0.0"
    );
    assert_eq!(
        col_map["is_byok"].dflt_value.as_deref(),
        Some("0"),
        "is_byok default should be 0"
    );
    assert_eq!(
        col_map["cache_hit"].dflt_value.as_deref(),
        Some("0"),
        "cache_hit default should be 0"
    );
    assert_eq!(
        col_map["status"].dflt_value.as_deref(),
        Some("'success'"),
        "status default should be 'success'"
    );

    // -- Status CHECK constraint --
    // SQLite stores CHECK constraints in the schema, but accessing them requires
    // parsing sqlite_master.sql. We verify by doing a round-trip insert.
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO llm_requests (id, model, status) VALUES (?1, ?2, 'success')")
        .bind(&id)
        .bind("gpt-4o")
        .execute(&pool)
        .await
        .expect("INSERT with status='success' should work");

    // Verify invalid status is rejected
    let result = sqlx::query(
        "INSERT INTO llm_requests (id, model, status) VALUES (?1, ?2, 'invalid_status')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind("gpt-4o")
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "INSERT with invalid status should be rejected by CHECK constraint"
    );

    // -- Foreign key: profile_id REFERENCES profiles(id) --
    let result = sqlx::query(
        "INSERT INTO llm_requests (id, model, profile_id) VALUES (?1, ?2, 'nonexistent')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind("gpt-4o")
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "INSERT with nonexistent profile_id should fail FK constraint"
    );
}

/// Asserts that running the migration twice is safe (idempotent).
#[tokio::test]
async fn test_migration_is_idempotent_for_llm_requests() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

    // First call
    valet::db::schema::run_migrations(&pool).await.unwrap();

    // Second call — should not error
    valet::db::schema::run_migrations(&pool).await.unwrap();

    // Table still exists and is usable
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        count, 0,
        "Table should be empty and readable after idempotent migration"
    );
}

// ── Tests that depend on StatsRepo ─────────────────────────────────────────

/// Verifies that a row inserted via StatsRepo matches the schema.
#[tokio::test]
async fn test_stats_repo_inserts_llm_request() {
    let pool = setup().await;

    // 1. Verify the repo summary handles an empty table
    let summary = valet::db::repos::stats::StatsRepo::summary(&pool)
        .await
        .unwrap();
    assert_eq!(summary.total_calls, 0, "empty table should report 0 calls");

    // 2. Insert a row with explicit values via raw query (StatsRepo is
    //    read-only / administrative — insert is done elsewhere, but verify
    //    the summary picks it up)
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO llm_requests (id, model, prompt_tokens, completion_tokens, total_tokens, cached_tokens, reasoning_tokens, cost, status)
         VALUES (?1, 'gpt-4o', 100, 50, 150, 10, 5, 0.015, 'success')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Verify the row is readable via summary
    let summary = valet::db::repos::stats::StatsRepo::summary(&pool)
        .await
        .unwrap();
    assert_eq!(summary.total_calls, 1);
    assert_eq!(summary.total_prompt_tokens, 100);
    assert_eq!(summary.total_completion_tokens, 50);
    assert_eq!(summary.total_tokens, 150);
    assert_eq!(summary.total_cached_tokens, 10);
    assert_eq!(summary.total_reasoning_tokens, 5);
    assert!((summary.total_cost - 0.015).abs() < f64::EPSILON);
    assert_eq!(summary.total_errors, 0);

    // 4. Verify by_model returns one row
    let models = valet::db::repos::stats::StatsRepo::by_model(&pool)
        .await
        .unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model, "gpt-4o");
    assert_eq!(models[0].calls, 1);

    // 5. Verify export_csv includes the row
    let csv = valet::db::repos::stats::StatsRepo::export_csv(&pool)
        .await
        .unwrap();
    assert!(csv.contains(&id), "CSV should contain the inserted row id");
    assert!(csv.contains("gpt-4o"), "CSV should contain the model name");
}

/// Asserts the `kind` column added by `20261009000001_llm_requests_kind.sql`:
/// `TEXT`, `NOT NULL`, default `'chat'`, with a `CHECK` over the five origins.
#[tokio::test]
async fn test_llm_requests_kind_column_schema() {
    let pool = setup().await;

    #[derive(sqlx::FromRow)]
    struct ColumnInfo {
        name: String,
        r#type: String,
        notnull: bool,
        dflt_value: Option<String>,
    }

    let columns: Vec<ColumnInfo> =
        sqlx::query_as("SELECT * FROM pragma_table_info('llm_requests')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let kind = columns
        .iter()
        .find(|c| c.name == "kind")
        .expect("llm_requests must have a 'kind' column");

    assert_eq!(kind.r#type, "TEXT", "'kind' must be TEXT");
    assert!(kind.notnull, "'kind' must be NOT NULL");
    assert_eq!(
        kind.dflt_value.as_deref(),
        Some("'chat'"),
        "'kind' default must be 'chat'"
    );

    // The CHECK constraint is stored verbatim in the table SQL.
    let table_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='llm_requests'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    for value in [
        "chat",
        "router",
        "archivist",
        "consolidator",
        "collapse",
        "timeline",
    ] {
        assert!(
            table_sql.contains(value),
            "CHECK constraint must allow '{value}'; table SQL was: {table_sql}"
        );
    }

    // Round-trip: an allowed origin is accepted…
    sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', 'router')")
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .expect("kind='router' must be accepted by the CHECK constraint");

    // …and a bogus origin is rejected.
    let invalid = sqlx::query(
        "INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', 'invalid_kind')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&pool)
    .await;

    assert!(
        invalid.is_err(),
        "kind='invalid_kind' must be rejected by the CHECK constraint"
    );
}
