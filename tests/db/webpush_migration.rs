//! Migration tests for WebPush notifications:
//! `migrations/20261012000001_webpush_notifications.sql`. The migration is
//! additive: it creates `push_subscriptions`, adds the nullable
//! `reminders.notified_at` column, seeds the three WebPush settings with the
//! same "only overwrite empty/NULL" idiom as `20261010000007_rss_skill.sql` and
//! creates the `(status, notified_at)` index the worker filters on. It never
//! rebuilds a table. It is re-runnable except for the trailing `ALTER TABLE`,
//! which SQLite cannot re-run; in production the `Migrator` runs it once.

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

/// Read the WebPush migration SQL from disk.
fn webpush_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261012000001_webpush_notifications.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// The re-runnable prefix of the migration: everything before the trailing
/// `ALTER TABLE ... ADD COLUMN`, which SQLite cannot re-run. Re-executing only
/// this prefix is honest — it succeeds — while the full body fails on the
/// `ALTER` (see `test_webpush_migration_respects_user_values`).
fn webpush_migration_sql_without_alter() -> String {
    let full = webpush_migration_sql();
    let marker = "ALTER TABLE reminders ADD COLUMN notified_at TEXT;";
    let (prefix, _) = full
        .split_once(marker)
        .unwrap_or_else(|| panic!("the migration must contain the ALTER marker: {marker}"));
    prefix.to_string()
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

/// Read a settings value, panicking if the key is missing.
async fn setting_value(pool: &SqlitePool, key: &str) -> String {
    setting_value_opt(pool, key)
        .await
        .unwrap_or_else(|| panic!("setting '{key}' must exist"))
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

// ─── Requirement: la migración crea la tabla `push_subscriptions` ───────────

/// Scenario: la migración crea la tabla, con las columnas exactas.
#[tokio::test]
async fn test_push_subscriptions_table_has_exact_columns() {
    let pool = setup().await;

    let exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='push_subscriptions'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(exists, "'push_subscriptions' must exist after migration");

    let cols = columns(&pool, "push_subscriptions").await;

    let id = cols.get("id").expect("column 'id' must exist");
    assert_eq!(id.r#type.to_uppercase(), "TEXT", "id must be TEXT");
    assert_eq!(id.pk, 1, "id must be PRIMARY KEY");

    let user_id = cols.get("user_id").expect("column 'user_id' must exist");
    assert_eq!(user_id.r#type.to_uppercase(), "TEXT");
    assert!(user_id.notnull, "user_id must be NOT NULL");
    assert_eq!(
        user_id.dflt_value.as_deref(),
        Some("'default'"),
        "user_id must default to 'default'"
    );

    let endpoint = cols.get("endpoint").expect("column 'endpoint' must exist");
    assert_eq!(endpoint.r#type.to_uppercase(), "TEXT");
    assert!(endpoint.notnull, "endpoint must be NOT NULL");
    assert_eq!(endpoint.pk, 0, "endpoint must not be the primary key");

    for name in ["p256dh", "auth"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT");
        assert!(col.notnull, "{name} must be NOT NULL");
    }

    let ua = cols
        .get("user_agent")
        .expect("column 'user_agent' must exist");
    assert!(!ua.notnull, "user_agent must be nullable");

    for name in ["created_at", "updated_at"] {
        let col = cols
            .get(name)
            .unwrap_or_else(|| panic!("column '{name}' must exist"));
        assert_eq!(col.r#type.to_uppercase(), "TEXT");
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

/// Scenario: un `endpoint` duplicado viola `UNIQUE`.
#[tokio::test]
async fn test_push_subscriptions_endpoint_is_unique() {
    let pool = setup().await;

    sqlx::query(
        "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth)
         VALUES ('a', 'https://example.invalid/push/1', 'p', 'a')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth)
         VALUES ('b', 'https://example.invalid/push/1', 'p2', 'a2')",
    )
    .execute(&pool)
    .await
    .unwrap_err();

    assert!(
        err.to_string().to_uppercase().contains("UNIQUE"),
        "a duplicate endpoint must violate UNIQUE, got: {err}"
    );
}

/// Scenario: el `user_id` por defecto es `'default'`.
#[tokio::test]
async fn test_push_subscriptions_user_id_defaults_to_default() {
    let pool = setup().await;

    sqlx::query(
        "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth)
         VALUES ('a', 'https://example.invalid/push/1', 'p', 'a')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let user_id: String =
        sqlx::query_scalar("SELECT user_id FROM push_subscriptions WHERE id = 'a'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(user_id, "default");
}

// ─── Requirement: `reminders` registra cuándo se notificó ───────────────────

/// Scenario: la columna existe y es nullable.
#[tokio::test]
async fn test_reminders_notified_at_is_nullable() {
    let pool = setup().await;

    let cols = columns(&pool, "reminders").await;
    let col = cols
        .get("notified_at")
        .expect("column 'notified_at' must exist in reminders");
    assert_eq!(
        col.r#type.to_uppercase(),
        "TEXT",
        "notified_at must be TEXT"
    );
    assert!(!col.notnull, "notified_at must be nullable");
}

/// Scenario: un recordatorio recién creado no está notificado.
#[tokio::test]
async fn test_fresh_reminder_notified_at_is_null() {
    let pool = setup().await;

    sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES ('p', 'P', '{}')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO reminders (id, profile_id, text, datetime, status, created_at)
         VALUES ('r', 'p', 'texto', '2026-10-12T10:00:00Z', 'pending', '2026-10-12T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let notified_at: Option<String> =
        sqlx::query_scalar("SELECT notified_at FROM reminders WHERE id = 'r'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notified_at, None, "a fresh reminder must not be notified");
}

// ─── Requirement: la migración siembra los ajustes de WebPush ───────────────

/// Scenario: tras migrar existen las tres claves con sus valores.
#[tokio::test]
async fn test_webpush_settings_seeded() {
    let pool = setup().await;

    assert_eq!(
        setting_value_opt(&pool, "push_vapid_public_key").await,
        Some(String::new()),
        "push_vapid_public_key must be seeded empty"
    );
    assert_eq!(
        setting_value_opt(&pool, "push_vapid_private_key").await,
        Some(String::new()),
        "push_vapid_private_key must be seeded empty"
    );
    assert_eq!(
        setting_value(&pool, "push_reminder_grace_hours").await,
        "24",
        "push_reminder_grace_hours must default to '24'"
    );
}

/// Scenario: una segunda ejecución no sobrescribe un valor personalizado (y
/// rellena un valor vacío o solo-espacios con el default).
#[tokio::test]
async fn test_webpush_migration_respects_user_values() {
    let pool = setup().await;

    for (key, value) in [
        ("push_vapid_public_key", "MI_PUBLICA"),
        ("push_vapid_private_key", "MI_PRIVADA"),
        ("push_reminder_grace_hours", "48"),
    ] {
        sqlx::query("UPDATE settings SET value = ?1 WHERE key = ?2")
            .bind(value)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Re-run the migration body. The seeding statements before the trailing
    // `ALTER TABLE ... ADD COLUMN` run again; the `ALTER` itself is the one
    // statement SQLite cannot re-run (no `ADD COLUMN IF NOT EXISTS`), so the
    // body fails on a duplicate-column error. That is the documented
    // limitation, and we assert it rather than swallowing it: in production the
    // `Migrator` applies this file exactly once.
    let sql = webpush_migration_sql();
    let err = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect_err("re-running the body must fail on the trailing ALTER");
    assert!(
        err.to_string().to_lowercase().contains("duplicate column"),
        "the re-run must fail because of the ALTER, got: {err}"
    );

    assert_eq!(
        setting_value(&pool, "push_vapid_public_key").await,
        "MI_PUBLICA",
        "a non-empty custom public key must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "push_vapid_private_key").await,
        "MI_PRIVADA",
        "a non-empty custom private key must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "push_reminder_grace_hours").await,
        "48",
        "a non-empty custom grace window must be preserved"
    );

    // An empty value is backfilled with the default. Re-run only the
    // re-runnable prefix (without the ALTER) so no error is swallowed.
    let prefix = webpush_migration_sql_without_alter();
    sqlx::query("UPDATE settings SET value = '' WHERE key = 'push_reminder_grace_hours'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(prefix.as_str()))
        .execute(&pool)
        .await
        .expect("the re-runnable prefix must succeed");
    assert_eq!(
        setting_value(&pool, "push_reminder_grace_hours").await,
        "24",
        "an empty grace window must be backfilled with the default"
    );

    // A whitespace-only value also counts as empty (the rest of the code trims
    // before deciding), so it too is backfilled instead of staying broken.
    sqlx::query("UPDATE settings SET value = '   ' WHERE key = 'push_reminder_grace_hours'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(prefix.as_str()))
        .execute(&pool)
        .await
        .expect("the re-runnable prefix must succeed");
    assert_eq!(
        setting_value(&pool, "push_reminder_grace_hours").await,
        "24",
        "a whitespace-only grace window must be treated as empty and backfilled"
    );
}

/// Scenario: la migración crea el índice que la consulta del worker necesita.
#[tokio::test]
async fn test_reminders_status_notified_at_index_exists() {
    let pool = setup().await;

    let exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_reminders_status_notified_at'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        exists,
        "the worker's (status, notified_at) index must exist after migration"
    );
}

/// Scenario: la migración es idempotente al ejecutarse de nuevo (vía el
/// migrador, que la salta) y mantiene una fila por clave.
#[tokio::test]
async fn test_webpush_migration_is_idempotent_through_migrator() {
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

    for key in [
        "push_vapid_public_key",
        "push_vapid_private_key",
        "push_reminder_grace_hours",
    ] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "expected exactly one row for key '{key}'");
    }

    let tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='push_subscriptions'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 1, "the table must exist exactly once");
}
