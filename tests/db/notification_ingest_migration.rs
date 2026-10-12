//! Migration tests for notification ingest:
//! `migrations/20261012000002_notification_ingest.sql`. The migration is
//! purely additive: it creates the `notifications` and `ingest_keys` tables and
//! their two indexes with `IF NOT EXISTS`, so re-executing the whole body is
//! safe. It never rebuilds a table and adds no column to an existing one, so
//! there is no trailing `ALTER TABLE` (the one statement SQLite cannot re-run).

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

/// Read the notification-ingest migration SQL from disk.
fn notification_ingest_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261012000002_notification_ingest.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// A struct mirroring one row of `pragma_table_info`.
#[derive(sqlx::FromRow)]
struct ColumnInfo {
    name: String,
    r#type: String,
    notnull: bool,
    dflt_value: Option<String>,
    pk: i64,
}

async fn columns(pool: &SqlitePool, table: &str) -> std::collections::BTreeMap<String, ColumnInfo> {
    let sql = format!(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('{table}')"
    );
    let rows: Vec<ColumnInfo> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .unwrap();
    rows.into_iter().map(|c| (c.name.clone(), c)).collect()
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name = ?1")
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ─── Requirement: la tabla `notifications` almacena la notificación ─────────

/// Scenario: la migración crea `notifications` con las columnas exactas.
#[tokio::test]
async fn test_notifications_table_has_exact_columns() {
    let pool = setup().await;

    assert!(
        table_exists(&pool, "notifications").await,
        "'notifications' must exist after migration"
    );

    let cols = columns(&pool, "notifications").await;
    assert_eq!(
        cols.len(),
        8,
        "notifications must have exactly 8 columns, got: {:?}",
        cols.keys().collect::<Vec<_>>()
    );

    let id = cols.get("id").expect("column 'id' must exist");
    assert_eq!(id.r#type.to_uppercase(), "TEXT", "id must be TEXT");
    assert_eq!(id.pk, 1, "id must be PRIMARY KEY");

    for name in ["origin", "author", "message"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(col.notnull, "{name} must be NOT NULL");
    }

    let level = cols.get("level").expect("column 'level' must exist");
    assert_eq!(level.r#type.to_uppercase(), "TEXT");
    assert!(level.notnull, "level must be NOT NULL");
    assert_eq!(
        level.dflt_value.as_deref(),
        Some("'info'"),
        "level must default to 'info'"
    );

    for name in ["metadata", "read_at"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(!col.notnull, "{name} must be nullable");
    }

    let created_at = cols
        .get("created_at")
        .expect("column 'created_at' must exist");
    assert_eq!(created_at.r#type.to_uppercase(), "TEXT");
    assert!(created_at.notnull, "created_at must be NOT NULL");
    assert!(
        created_at
            .dflt_value
            .as_deref()
            .is_some_and(|d| d.contains("datetime")),
        "created_at must default to datetime('now'), got {:?}",
        created_at.dflt_value
    );
}

/// Scenario: las dos tablas nacen vacías.
#[tokio::test]
async fn test_notifications_and_ingest_keys_are_empty_after_migration() {
    let pool = setup().await;

    let notifications: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(notifications, 0, "notifications must be born empty");

    let ingest_keys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ingest_keys, 0, "ingest_keys must be born empty");
}

/// Scenario: el índice de no leídas es parcial y filtra por `read_at IS NULL`.
#[tokio::test]
async fn test_idx_notifications_unread_is_partial() {
    let pool = setup().await;

    let sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_notifications_unread'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    let sql = sql.expect("index 'idx_notifications_unread' must exist");
    let upper = sql.to_uppercase();
    assert!(
        upper.contains("READ_AT IS NULL"),
        "the unread index must be partial (WHERE read_at IS NULL), got: {sql}"
    );
    assert!(
        upper.contains("CREATED_AT"),
        "the unread index must index created_at, got: {sql}"
    );
}

/// Scenario: existe el índice `(origin, created_at)`.
#[tokio::test]
async fn test_idx_notifications_origin_exists() {
    let pool = setup().await;

    let sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_notifications_origin'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    let sql = sql.expect("index 'idx_notifications_origin' must exist");
    let upper = sql.to_uppercase();
    assert!(
        upper.contains("ORIGIN") && upper.contains("CREATED_AT"),
        "the origin index must cover (origin, created_at), got: {sql}"
    );
}

// ─── Requirement: la tabla `ingest_keys` persiste hash y prefijo ────────────

/// Scenario: la migración crea `ingest_keys` con las columnas exactas.
#[tokio::test]
async fn test_ingest_keys_table_has_exact_columns() {
    let pool = setup().await;

    assert!(
        table_exists(&pool, "ingest_keys").await,
        "'ingest_keys' must exist after migration"
    );

    let cols = columns(&pool, "ingest_keys").await;
    assert_eq!(
        cols.len(),
        7,
        "ingest_keys must have exactly 7 columns, got: {:?}",
        cols.keys().collect::<Vec<_>>()
    );

    let id = cols.get("id").expect("column 'id' must exist");
    assert_eq!(id.r#type.to_uppercase(), "TEXT", "id must be TEXT");
    assert_eq!(id.pk, 1, "id must be PRIMARY KEY");

    for name in ["name", "key_hash", "prefix"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(col.notnull, "{name} must be NOT NULL");
    }

    let created_at = cols
        .get("created_at")
        .expect("column 'created_at' must exist");
    assert_eq!(created_at.r#type.to_uppercase(), "TEXT");
    assert!(created_at.notnull, "created_at must be NOT NULL");
    assert!(
        created_at
            .dflt_value
            .as_deref()
            .is_some_and(|d| d.contains("datetime")),
        "created_at must default to datetime('now'), got {:?}",
        created_at.dflt_value
    );

    for name in ["last_used_at", "revoked_at"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(!col.notnull, "{name} must be nullable");
    }
}

/// Scenario: `name` duplicado viola UNIQUE.
#[tokio::test]
async fn test_ingest_keys_name_is_unique() {
    let pool = setup().await;

    sqlx::query("INSERT INTO ingest_keys (id, name, key_hash, prefix) VALUES ('a', 'monitor', 'h1', 'val_aaaaaaaa')")
        .execute(&pool)
        .await
        .unwrap();

    let err = sqlx::query(
        "INSERT INTO ingest_keys (id, name, key_hash, prefix) VALUES ('b', 'monitor', 'h2', 'val_bbbbbbbb')",
    )
    .execute(&pool)
    .await
    .unwrap_err();

    assert!(
        err.to_string().to_uppercase().contains("UNIQUE"),
        "a duplicate name must violate UNIQUE, got: {err}"
    );
}

/// Scenario: `key_hash` duplicado viola UNIQUE.
#[tokio::test]
async fn test_ingest_keys_key_hash_is_unique() {
    let pool = setup().await;

    sqlx::query("INSERT INTO ingest_keys (id, name, key_hash, prefix) VALUES ('a', 'monitor', 'hash', 'val_aaaaaaaa')")
        .execute(&pool)
        .await
        .unwrap();

    let err = sqlx::query(
        "INSERT INTO ingest_keys (id, name, key_hash, prefix) VALUES ('b', 'other', 'hash', 'val_bbbbbbbb')",
    )
    .execute(&pool)
    .await
    .unwrap_err();

    assert!(
        err.to_string().to_uppercase().contains("UNIQUE"),
        "a duplicate key_hash must violate UNIQUE, got: {err}"
    );
}

// ─── Idempotencia ───────────────────────────────────────────────────────────

/// Scenario: la migración es idempotente al ejecutarse dos veces vía el
/// migrador (los índices y tablas no se duplican).
#[tokio::test]
async fn test_notification_ingest_migration_is_idempotent_through_migrator() {
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
    valet::db::schema::run_migrations(&pool).await.unwrap();

    for table in ["notifications", "ingest_keys"] {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1")
                .bind(table)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "table '{table}' must exist exactly once");
    }

    for index in ["idx_notifications_unread", "idx_notifications_origin"] {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name = ?1")
                .bind(index)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "index '{index}' must exist exactly once");
    }

    // The whole body is re-runnable: re-applying it verbatim is a no-op.
    let sql = notification_ingest_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the migration body must be re-runnable whole");
}
