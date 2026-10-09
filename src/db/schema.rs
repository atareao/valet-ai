use sqlx::SqlitePool;

/// Run all database migrations using sqlx's embedded migration system.
///
/// Migrations live in the `migrations/` directory. This function resolves
/// the path relative to `CARGO_MANIFEST_DIR` (embedded at compile time) to
/// work reliably regardless of the process's current working directory.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    ensure_vec0_on_pool(pool).await?;

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrations_path = manifest.join("migrations");
    sqlx::migrate::Migrator::new(migrations_path)
        .await?
        .run(pool)
        .await?;
    Ok(())
}

/// Make sure the `sqlite-vec` `vec0` module is available on `pool` before the
/// migration that creates the `vec_memory` virtual table runs.
///
/// `sqlite3_auto_extension` only affects connections opened *after* it is
/// called. [`register_vec_extension`] is idempotent (fenced by a [`Once`]), but
/// a caller may have opened its pool *before* reaching here — as the test
/// helpers do. In that case the pooled connection lacks `vec0`, so we register
/// the extension and detach one stale connection to force the pool to open a
/// fresh one that inherits it.
///
/// [`register_vec_extension`]: crate::db::vec_extension::register_vec_extension
/// [`Once`]: std::sync::Once
async fn ensure_vec0_on_pool(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    crate::db::vec_extension::register_vec_extension();

    // If the extension is already usable on a pooled connection there is
    // nothing to do (this is the production path: `init_db` registers before
    // opening the pool, and this is a no-op).
    if sqlx::query_scalar::<_, String>("SELECT vec_version()")
        .fetch_one(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }

    // The pooled connection(s) predate the registration. Detach one so the
    // pool reopens a connection, which then picks up the auto-extension.
    let conn = pool.acquire().await?;
    drop(conn.detach());
    Ok(())
}

/// Seed default settings into the database.
/// Called after migrations to ensure required settings exist.
pub async fn seed_default_settings(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    crate::db::repos::settings::SettingsRepo::seed_defaults(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

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
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_migrations_does_not_create_conversations() {
        let pool = setup().await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='conversations'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            count, 0,
            "Table 'conversations' should NOT exist after migration"
        );
    }

    #[tokio::test]
    async fn test_migrations_creates_messages_table() {
        let pool = setup().await;

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='messages'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            has_table,
            "Expected 'messages' table to exist after migration"
        );

        // Verify no conversation_id column exists
        let column_names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert!(
            !column_names.contains(&"conversation_id".to_string()),
            "Column 'conversation_id' should NOT exist in messages table"
        );
    }

    /// The six tables belonging to the removed tools (meals, shopping list,
    /// habits and contacts) must NOT exist after migration.
    const REMOVED_TOOLS_TABLES: [&str; 6] = [
        "contacts",
        "contacts_fts",
        "meal_plans",
        "shopping_list",
        "habits",
        "habit_logs",
    ];

    #[tokio::test]
    async fn test_migrations_drop_removed_tools_tables() {
        let pool = setup().await;

        for table in REMOVED_TOOLS_TABLES {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = ?1")
                    .bind(table)
                    .fetch_one(&pool)
                    .await
                    .unwrap();

            assert_eq!(count, 0, "Table '{table}' should NOT exist after migration");
        }
    }

    #[tokio::test]
    async fn test_idempotent_does_not_recreate_removed_tables() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .unwrap();

        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        for table in REMOVED_TOOLS_TABLES {
            assert!(
                !tables.contains(&table.to_string()),
                "Table '{table}' should NOT exist after idempotent migration"
            );
        }
    }

    #[tokio::test]
    async fn test_messages_table_has_location_column() {
        let pool = setup().await;

        let column_names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert!(
            column_names.contains(&"location".to_string()),
            "Column 'location' should exist in messages table"
        );
    }

    #[tokio::test]
    async fn test_messages_table_has_new_columns() {
        let pool = setup().await;

        let column_names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert!(
            column_names.contains(&"tokens_count".to_string()),
            "Column 'tokens_count' should exist in messages table"
        );
        assert!(
            column_names.contains(&"collapsed_content".to_string()),
            "Column 'collapsed_content' should exist in messages table"
        );
        assert!(
            column_names.contains(&"collapsed_tokens_count".to_string()),
            "Column 'collapsed_tokens_count' should exist in messages table"
        );
        assert!(
            column_names.contains(&"is_indexed".to_string()),
            "Column 'is_indexed' should exist in messages table"
        );
        assert!(
            column_names.contains(&"summary_ref".to_string()),
            "Column 'summary_ref' should exist in messages table"
        );
    }

    // ─── Episodic memory migration tests ───────────────────────────────────────

    #[tokio::test]
    async fn test_memory_table_exists_with_columns() {
        let pool = setup().await;

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            has_table,
            "Expected 'memory' table to exist after migration"
        );

        let columns: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('memory')",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let col_map: std::collections::BTreeMap<String, (String, Option<String>, i64)> = columns
            .into_iter()
            .map(|(_cid, name, ty, _notnull, dflt, pk)| (name, (ty, dflt, pk)))
            .collect();

        // id TEXT PRIMARY KEY
        let (ty, _dflt, pk) = col_map.get("id").expect("Column 'id' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "id should be TEXT");
        assert_eq!(*pk, 1, "id should be PRIMARY KEY");

        // content TEXT NOT NULL
        let (ty, _dflt, pk) = col_map
            .get("content")
            .expect("Column 'content' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "content should be TEXT");
        assert_eq!(*pk, 0, "content should not be PK");

        // tokens_count INTEGER NOT NULL DEFAULT 0
        let (ty, dflt, pk) = col_map
            .get("tokens_count")
            .expect("Column 'tokens_count' should exist");
        assert_eq!(
            ty.to_uppercase(),
            "INTEGER",
            "tokens_count should be INTEGER"
        );
        assert_eq!(
            dflt.as_deref(),
            Some("0"),
            "tokens_count should default to 0"
        );
        assert_eq!(*pk, 0, "tokens_count should not be PK");

        // created_at TEXT
        let (ty, _dflt, _pk) = col_map
            .get("created_at")
            .expect("Column 'created_at' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "created_at should be TEXT");

        // metadata TEXT DEFAULT '{}'
        let (ty, dflt, _pk) = col_map
            .get("metadata")
            .expect("Column 'metadata' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "metadata should be TEXT");
        assert_eq!(
            dflt.as_deref(),
            Some("'{}'"),
            "metadata should default to '{{}}'"
        );
    }

    #[tokio::test]
    async fn test_vec_memory_virtual_table_exists() {
        let pool = setup().await;

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='vec_memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            has_table,
            "Expected 'vec_memory' virtual table to exist after migration"
        );

        let ddl: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='vec_memory'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(
            ddl.as_ref().is_some_and(|s| {
                let upper = s.trim().to_uppercase();
                upper.starts_with("CREATE TABLE") || upper.starts_with("CREATE VIRTUAL TABLE")
            }),
            "vec_memory must be a TABLE (regular or virtual)"
        );
    }

    #[tokio::test]
    async fn test_idx_messages_unindexed_exists() {
        let pool = setup().await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_messages_unindexed'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            count, 1,
            "Index 'idx_messages_unindexed' should exist on messages(created_at) WHERE is_indexed = 0"
        );
    }

    #[tokio::test]
    async fn test_idx_messages_summary_ref_exists() {
        let pool = setup().await;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_messages_summary_ref'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            count, 1,
            "Index 'idx_messages_summary_ref' should exist on messages(summary_ref) WHERE summary_ref IS NOT NULL"
        );
    }

    #[tokio::test]
    async fn test_legacy_tables_do_not_exist() {
        let pool = setup().await;

        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        for legacy in &["memories", "memory_embeddings", "memories_fts"] {
            assert!(
                !tables.contains(&legacy.to_string()),
                "Legacy table '{legacy}' should NOT exist after migration"
            );
        }
    }

    #[tokio::test]
    async fn test_memory_migration_is_idempotent() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .unwrap();

        // Run twice – second run must not error
        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        // Verify memory table still looks correct after second run
        let has_memory: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            has_memory,
            "memory table should survive idempotent migration"
        );

        let has_vec: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='vec_memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            has_vec,
            "vec_memory table should survive idempotent migration"
        );
    }

    // ─── 3.1 / 3.3: the vec0 virtual table ───────────────────────────────────

    /// 3.1 — `vec_memory` is a `vec0` virtual table exposing `id` and
    /// `embedding`, with the cosine metric.
    #[tokio::test]
    async fn test_vec_memory_is_virtual_vec0_with_id_and_embedding() {
        let pool = setup().await;

        let sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='vec_memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            sql.to_uppercase().contains("CREATE VIRTUAL TABLE") && sql.contains("vec0"),
            "vec_memory must be a vec0 virtual table, got: {sql}"
        );
        assert!(sql.contains("id"), "vec_memory must declare `id`");
        assert!(
            sql.contains("embedding"),
            "vec_memory must declare `embedding`"
        );
        assert!(
            sql.contains("distance_metric=cosine"),
            "vec_memory must declare the cosine metric, got: {sql}"
        );

        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('vec_memory')")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(
            columns.contains(&"id".to_string()),
            "vec_memory must expose the `id` column"
        );
        assert!(
            columns.contains(&"embedding".to_string()),
            "vec_memory must expose the `embedding` column"
        );
    }

    /// 3.3 — `vec0` rejects a vector whose dimension differs from the one the
    /// table declares, so embedding-dimension drift is impossible structurally.
    #[tokio::test]
    async fn test_vec0_rejects_wrong_dimension_vector() {
        let pool = setup().await;

        let err = sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES ('x', vec_f32(?1))")
            .bind("[1.0, 2.0]")
            .execute(&pool)
            .await
            .unwrap_err();

        let message = err.to_string();
        assert!(
            message.to_lowercase().contains("dimension"),
            "vec0 must reject a vector of the wrong dimension, got: {message}"
        );
    }

    // ─── 6.1: the four episodic-memory knobs in `settings` ───────────────────

    /// Read the memory-settings migration SQL from disk.
    fn memory_settings_migration_sql() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/20261001000002_memory_settings.sql");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
    }

    async fn settings_value(pool: &SqlitePool, key: &str) -> String {
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("setting '{key}' should exist: {e}"))
    }

    /// A freshly migrated database carries the four knobs with their defaults.
    #[tokio::test]
    async fn test_memory_settings_seeded_with_defaults() {
        let pool = setup().await;

        assert_eq!(settings_value(&pool, "MEMORY_HALF_LIFE_DAYS").await, "90");
        assert_eq!(settings_value(&pool, "SIMILARITY_THRESHOLD").await, "0.5");
        assert_eq!(settings_value(&pool, "RAG_BUDGET_TOKENS").await, "800");
        assert_eq!(settings_value(&pool, "MEMORY_KNN_CANDIDATES").await, "20");
    }

    /// A non-empty pre-existing value is respected by the seeding migration
    /// (same "only overwrite empty/NULL" idiom as the prompts migration).
    #[tokio::test]
    async fn test_memory_settings_respects_existing_value() {
        let pool = setup().await;

        sqlx::query("UPDATE settings SET value = '1200' WHERE key = 'RAG_BUDGET_TOKENS'")
            .execute(&pool)
            .await
            .unwrap();

        // Re-run the seeding body (the migration itself is already recorded).
        let sql = memory_settings_migration_sql();
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            settings_value(&pool, "RAG_BUDGET_TOKENS").await,
            "1200",
            "a non-empty custom value must be preserved by the migration"
        );

        // An empty value, on the other hand, is backfilled with the default.
        sqlx::query("UPDATE settings SET value = '' WHERE key = 'RAG_BUDGET_TOKENS'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            settings_value(&pool, "RAG_BUDGET_TOKENS").await,
            "800",
            "an empty value must be backfilled with the default"
        );
    }

    // ─── Bloque 2: Capa C (persistent_memory) migration ──────────────────────

    /// Read the Capa C migration SQL from disk.
    fn persistent_memory_migration_sql() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations/20261002000001_persistent_memory.sql");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
    }

    /// The migration creates `persistent_memory(id TEXT PRIMARY KEY,
    /// payload TEXT NOT NULL, updated_at)`.
    #[tokio::test]
    async fn test_persistent_memory_table_exists_with_columns() {
        let pool = setup().await;

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='persistent_memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            has_table,
            "Expected 'persistent_memory' table after migration"
        );

        let columns: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('persistent_memory')",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let col_map: std::collections::BTreeMap<String, (String, i64, i64)> = columns
            .into_iter()
            .map(|(_cid, name, ty, notnull, _dflt, pk)| (name, (ty, notnull, pk)))
            .collect();

        let (ty, _notnull, pk) = col_map.get("id").expect("Column 'id' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "id should be TEXT");
        assert_eq!(*pk, 1, "id should be PRIMARY KEY");

        let (ty, notnull, pk) = col_map
            .get("payload")
            .expect("Column 'payload' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "payload should be TEXT");
        assert_eq!(*notnull, 1, "payload should be NOT NULL");
        assert_eq!(*pk, 0, "payload should not be PK");

        let (ty, _notnull, _pk) = col_map
            .get("updated_at")
            .expect("Column 'updated_at' should exist");
        assert_eq!(ty.to_uppercase(), "TEXT", "updated_at should be TEXT");
    }

    /// The migration seeds `consolidator_prompt` (with both placeholders) and
    /// `PERSISTENT_MEMORY_BUDGET_TOKENS = 800` (bumped from 500 by the
    /// consolidator-reliability migration).
    #[tokio::test]
    async fn test_persistent_memory_settings_seeded() {
        let pool = setup().await;

        let prompt = settings_value(&pool, "consolidator_prompt").await;
        assert!(
            !prompt.is_empty(),
            "consolidator_prompt should not be empty"
        );
        assert!(
            prompt.contains("{{ ESTADO_ACTUAL }}"),
            "consolidator_prompt should contain the {{ ESTADO_ACTUAL }} placeholder"
        );
        assert!(
            prompt.contains("{{ BLOQUE_DE_MENSAJES }}"),
            "consolidator_prompt should contain the {{ BLOQUE_DE_MENSAJES }} placeholder"
        );

        assert_eq!(
            settings_value(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS").await,
            "800",
            "the budget knob should default to 800"
        );
    }

    /// Re-running the migration body is idempotent, keeps one row per key and a
    /// single table, and never overwrites a non-empty custom value (only
    /// empty/NULL values are backfilled).
    #[tokio::test]
    async fn test_persistent_memory_migration_idempotent_and_respects_existing() {
        let pool = setup().await;

        sqlx::query(
            "UPDATE settings SET value = '999' WHERE key = 'PERSISTENT_MEMORY_BUDGET_TOKENS'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let sql = persistent_memory_migration_sql();
        for _ in 0..2 {
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(&pool)
                .await
                .unwrap();
        }

        // Exactly one table and one row per seeded key.
        let tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='persistent_memory'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(tables, 1, "the table must exist exactly once");

        for key in &["consolidator_prompt", "PERSISTENT_MEMORY_BUDGET_TOKENS"] {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
                .bind(key)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 1, "expected exactly one row for key '{key}'");
        }

        // A non-empty custom value survives; an empty one is backfilled.
        assert_eq!(
            settings_value(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS").await,
            "999",
            "a non-empty custom budget must be preserved"
        );

        sqlx::query("UPDATE settings SET value = '' WHERE key = 'PERSISTENT_MEMORY_BUDGET_TOKENS'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            settings_value(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS").await,
            "500",
            "an empty budget must be backfilled with the default"
        );
    }
}
