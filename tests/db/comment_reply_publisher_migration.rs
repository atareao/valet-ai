//! Migration tests for the `comment-reply-publisher` change:
//!
//!   * `migrations/20261022000001_comment_replies.sql` — the `comment_replies`
//!     idempotency register (tasks 1.2);
//!   * `migrations/20261022000002_blog_skill.sql` — the `blog` skill seed
//!     (tasks 1.3).
//!
//! Both are additive and idempotent: `CREATE TABLE IF NOT EXISTS` plus an index
//! with `IF NOT EXISTS`, and a settings seed that only fills an absent or empty
//! value. Neither rebuilds a table.
//!
//! Nota: `tasks.md` 1.2 dice «sus **nueve** columnas», pero el delta aprobado
//! (`specs/blog-comments/spec.md`, requisito de la tabla `comment_replies`)
//! enumera **ocho**. Se sigue el delta, que es el contrato autorizado, y se
//! anota la diferencia en el informe de la tarea.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// The columns the migration must declare, in `pragma_table_info` order.
const EXPECTED_COLUMNS: [&str; 8] = [
    "comment_id",
    "post_id",
    "notification_id",
    "state",
    "wp_comment_id",
    "last_status",
    "created_at",
    "updated_at",
];

/// A brand-new, empty in-memory database.
async fn in_memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap()
}

/// A database with every migration applied (`run_migrations()`).
async fn setup() -> SqlitePool {
    let pool = in_memory_pool().await;
    valet::db::schema::run_migrations(&pool).await.unwrap();
    pool
}

/// Read the `comment_replies` migration SQL from disk.
fn comment_replies_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261022000001_comment_replies.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `comment_replies` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// Read the `blog`-skill migration SQL from disk.
fn blog_skill_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261022000002_blog_skill.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `blog`-skill migration does not exist yet at {}: {e}",
            path.display()
        )
    })
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

/// The columns of `comment_replies`, in `pragma_table_info` order.
async fn columns(pool: &SqlitePool) -> Vec<ColumnInfo> {
    sqlx::query_as(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk \
         FROM pragma_table_info('comment_replies')",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| panic!("'comment_replies' must exist to be inspected: {e}"))
}

/// The description of one column, or a panic naming the missing one.
fn column<'a>(columns: &'a [ColumnInfo], name: &str) -> &'a ColumnInfo {
    columns
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("column '{name}' must exist"))
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name = ?1")
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The `sql` of the table itself, as the schema holds it.
async fn table_sql(pool: &SqlitePool) -> String {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='comment_replies'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("'comment_replies' must exist: {e}"))
}

/// The `rootpage` of a table: it changes when SQLite drops and recreates the
/// table, so an unchanged value is the fingerprint of «no rebuild».
async fn rootpage(pool: &SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT rootpage FROM sqlite_master WHERE type='table' AND name = ?1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("'{name}' must exist: {e}"))
}

/// Every explicit index declared on `comment_replies` (the implicit ones for
/// `UNIQUE`/`PRIMARY KEY` have `sql IS NULL`).
async fn index_sqls(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master \
         WHERE type='index' AND tbl_name='comment_replies' AND sql IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Read a settings value, panicking if the key is missing.
async fn setting_value(pool: &SqlitePool, key: &str) -> String {
    setting_value_opt(pool, key)
        .await
        .unwrap_or_else(|| panic!("setting '{key}' must exist"))
}

/// Read a settings value, returning `None` when the key is absent.
async fn setting_value_opt(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap()
        .flatten()
}

// ─── 1.2 — la tabla, su índice y su reserva atómica ─────────────────────────

/// Scenario: tras migrar existe `comment_replies` con sus columnas exactas y
/// `comment_id` como clave primaria.
#[tokio::test]
async fn test_comment_replies_table_has_exact_columns() {
    let pool = setup().await;

    assert!(
        table_exists(&pool, "comment_replies").await,
        "'comment_replies' must exist after migration"
    );

    let cols = columns(&pool).await;
    assert_eq!(
        cols.len(),
        EXPECTED_COLUMNS.len(),
        "comment_replies must have exactly {} columns, got: {:?}",
        EXPECTED_COLUMNS.len(),
        cols.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(
        cols.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        EXPECTED_COLUMNS.to_vec(),
        "the columns must be declared in this order"
    );

    let comment_id = column(&cols, "comment_id");
    assert_eq!(
        comment_id.r#type.to_uppercase(),
        "TEXT",
        "comment_id must be TEXT"
    );
    assert_eq!(comment_id.pk, 1, "comment_id must be PRIMARY KEY");
    assert!(comment_id.notnull, "comment_id must be NOT NULL");

    let post_id = column(&cols, "post_id");
    assert_eq!(
        post_id.r#type.to_uppercase(),
        "TEXT",
        "post_id must be TEXT"
    );
    assert!(post_id.notnull, "post_id must be NOT NULL");

    for name in ["notification_id", "wp_comment_id"] {
        let col = column(&cols, name);
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(!col.notnull, "{name} must be nullable");
    }

    let state = column(&cols, "state");
    assert_eq!(state.r#type.to_uppercase(), "TEXT", "state must be TEXT");
    assert!(state.notnull, "state must be NOT NULL");

    let last_status = column(&cols, "last_status");
    assert_eq!(
        last_status.r#type.to_uppercase(),
        "INTEGER",
        "last_status must be INTEGER"
    );
    assert!(!last_status.notnull, "last_status must be nullable");

    for name in ["created_at", "updated_at"] {
        let col = column(&cols, name);
        assert_eq!(col.r#type.to_uppercase(), "TEXT", "{name} must be TEXT");
        assert!(col.notnull, "{name} must be NOT NULL");
        assert!(
            col.dflt_value
                .as_deref()
                .is_some_and(|d| d.contains("datetime")),
            "{name} must default to datetime('now'), got {:?}",
            col.dflt_value
        );
    }
}

/// Scenario: `state` admite `reserved`, `published` y `uncertain`, y rechaza
/// cualquier otro valor (el `CHECK` de la tabla es la puerta).
#[tokio::test]
async fn test_comment_replies_state_check_admits_the_three_states() {
    let pool = setup().await;

    for (comment_id, state) in [
        ("901", "reserved"),
        ("902", "published"),
        ("903", "uncertain"),
    ] {
        let inserted = sqlx::query(
            "INSERT INTO comment_replies (comment_id, post_id, state) VALUES (?1, '42', ?2)",
        )
        .bind(comment_id)
        .bind(state)
        .execute(&pool)
        .await;
        assert!(
            inserted.is_ok(),
            "state '{state}' must be admitted by the CHECK: {:?}",
            inserted.err()
        );
    }

    let invalid = sqlx::query(
        "INSERT INTO comment_replies (comment_id, post_id, state) VALUES ('904', '42', 'unknown')",
    )
    .execute(&pool)
    .await;
    assert!(
        invalid.is_err(),
        "state 'unknown' must be rejected by the CHECK"
    );
}

/// Scenario: la migración declara el índice `idx_comment_replies_post` sobre
/// `(post_id, created_at)`.
#[tokio::test]
async fn test_idx_comment_replies_post_exists() {
    let pool = setup().await;

    let sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='index' AND name = 'idx_comment_replies_post'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    let sql = sql.expect("index 'idx_comment_replies_post' must exist");
    let upper = sql.to_uppercase();
    assert!(
        upper.contains("POST_ID") && upper.contains("CREATED_AT"),
        "the index must cover (post_id, created_at), got: {sql}"
    );
}

/// Scenario: un `INSERT ... ON CONFLICT(comment_id) DO NOTHING` repetido no
/// duplica la fila y devuelve **cero** filas afectadas.
#[tokio::test]
async fn test_comment_replies_reserve_is_atomic_and_repeatable() {
    let pool = setup().await;

    let first = sqlx::query(
        "INSERT INTO comment_replies (comment_id, post_id, state) VALUES ('901', '42', 'reserved') \
         ON CONFLICT(comment_id) DO NOTHING",
    )
    .execute(&pool)
    .await
    .expect("the first reserve must succeed");
    assert_eq!(
        first.rows_affected(),
        1,
        "the first reserve must materialise one row"
    );

    let second = sqlx::query(
        "INSERT INTO comment_replies (comment_id, post_id, state) VALUES ('901', '99', 'reserved') \
         ON CONFLICT(comment_id) DO NOTHING",
    )
    .execute(&pool)
    .await
    .expect("the second reserve must not error");
    assert_eq!(
        second.rows_affected(),
        0,
        "a repeated reserve must affect zero rows"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comment_replies")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "there must still be exactly one row for '901'");

    let post_id: String =
        sqlx::query_scalar("SELECT post_id FROM comment_replies WHERE comment_id = '901'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        post_id, "42",
        "the existing row must not be overwritten (DO NOTHING)"
    );
}

/// Scenario: volver a aplicar la migración es idempotente y **no** reconstruye
/// la tabla (su `sql` y su `rootpage` no cambian, y la fila sobrevive).
#[tokio::test]
async fn test_comment_replies_migration_is_idempotent_and_rebuilds_nothing() {
    let pool = setup().await;

    sqlx::query(
        "INSERT INTO comment_replies (comment_id, post_id, state) VALUES ('901', '42', 'published')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql_before = table_sql(&pool).await;
    let indexes_before = index_sqls(&pool).await;
    let rootpage_before = rootpage(&pool, "comment_replies").await;

    // Re-execute the migration body as-is…
    let sql = comment_replies_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("re-executing the migration body must succeed");

    // …and re-run the migrator itself (a no-op on an up-to-date database).
    valet::db::schema::run_migrations(&pool)
        .await
        .expect("re-running the migrations must not fail");

    let tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='comment_replies'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 1, "the table must exist exactly once");

    assert_eq!(
        table_sql(&pool).await,
        sql_before,
        "the table schema must be identical after re-applying the migration"
    );
    assert_eq!(
        index_sqls(&pool).await,
        indexes_before,
        "the indexes must be identical after re-applying the migration"
    );
    assert_eq!(
        rootpage(&pool, "comment_replies").await,
        rootpage_before,
        "the table must NOT have been rebuilt (rootpage changed)"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comment_replies")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "the pre-existing row must survive");
}

// ─── 1.3 — la siembra de la skill `blog` ────────────────────────────────────

/// Scenario: tras migrar existen `ROUTER_SKILL_BLOG_ENABLED` (`'true'`) y
/// `SKILL_BLOG_PROMPT`, cuya primera línea es `# SKILL ACTIVA: BLOG`.
#[tokio::test]
async fn test_blog_skill_migration_seeds_defaults() {
    let pool = setup().await;

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_BLOG_ENABLED").await,
        "true",
        "the blog skill must be enabled by default"
    );

    let prompt = setting_value(&pool, "SKILL_BLOG_PROMPT").await;
    assert!(!prompt.trim().is_empty(), "the fragment must not be empty");
    assert_eq!(
        prompt.lines().next(),
        Some("# SKILL ACTIVA: BLOG"),
        "the fragment's first line must be its section heading"
    );
}

/// Scenario: volver a aplicar el cuerpo de la migración es idempotente (una
/// fila por clave y los mismos valores).
#[tokio::test]
async fn test_blog_skill_migration_is_idempotent() {
    let pool = setup().await;

    let sql = blog_skill_migration_sql();
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();
    }

    for key in ["ROUTER_SKILL_BLOG_ENABLED", "SKILL_BLOG_PROMPT"] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "expected exactly one row for key '{key}'");
    }

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_BLOG_ENABLED").await,
        "true",
        "re-applying the migration must not change the enablement flag"
    );
    assert_eq!(
        setting_value(&pool, "SKILL_BLOG_PROMPT")
            .await
            .lines()
            .next(),
        Some("# SKILL ACTIVA: BLOG"),
        "re-applying the migration must not change the fragment heading"
    );
}

/// Scenario: un valor escrito por el usuario no se sobrescribe; un valor vacío
/// sí se rellena con el de por defecto.
#[tokio::test]
async fn test_blog_skill_migration_respects_user_values() {
    let pool = setup().await;

    for (key, value) in [
        ("ROUTER_SKILL_BLOG_ENABLED", "false"),
        ("SKILL_BLOG_PROMPT", "MI PROMPT PERSONALIZADO"),
    ] {
        sqlx::query("UPDATE settings SET value = ?1 WHERE key = ?2")
            .bind(value)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }

    let sql = blog_skill_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_BLOG_ENABLED").await,
        "false",
        "a disabled-by-the-user skill must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "SKILL_BLOG_PROMPT").await,
        "MI PROMPT PERSONALIZADO",
        "a non-empty custom prompt must be preserved"
    );

    // Un valor vacío, en cambio, se rellena con el de por defecto.
    sqlx::query("UPDATE settings SET value = '' WHERE key = 'ROUTER_SKILL_BLOG_ENABLED'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_BLOG_ENABLED").await,
        "true",
        "an empty value must be backfilled with the default"
    );
}
