//! Migration tests for the `pendientes` notifications fragment:
//! `migrations/20261012000003_pendientes_notifications_prompt.sql`.
//!
//! The migration adds one operating-guidance line about the `notifications`
//! tool (`list_unread` / `mark_read`) to the `SKILL_PENDIENTES_PROMPT`
//! fragment. It follows the same idiom as `20261010000004_timeline_prompts.sql`
//! and the `timeline-fact-integrity` migration: it is idempotent and it only
//! overwrites the value when it is still byte-for-byte the one seeded by
//! `20261008000001_skill_router_tuning.sql`. A user customisation is left
//! untouched, and the fragment's first line stays the skill's `prompt_heading`
//! (`# SKILL ACTIVA: PENDIENTES`).

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Version of `migrations/20261012000003_pendientes_notifications_prompt.sql`.
const PROMPT_VERSION: i64 = 20261012000003;

/// The skill's `prompt_heading` (first line of the fragment).
const HEADING: &str = "# SKILL ACTIVA: PENDIENTES";

/// The distinctive substrings the new operating-guidance line must carry.
const NEW_LINE_NEEDLES: [&str; 2] = ["list_unread", "mark_read"];

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

/// Read the pendientes-notifications migration SQL from disk.
fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261012000003_pendientes_notifications_prompt.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `pendientes-notifications` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left
/// in the state the previous release had: `SKILL_PENDIENTES_PROMPT` still holds
/// the value seeded by `20261008000001`, without the notifications line. The new
/// migration is NOT applied.
async fn pool_before(before: i64) -> SqlitePool {
    // `run_migrations` calls `register_vec_extension()` before anything else;
    // `sqlite3_auto_extension` is process-global but only reaches connections
    // opened *after* it runs. A throwaway pool triggers the registration, so the
    // pool below (whose connections open later) inherits `vec0`.
    let throwaway = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();
    valet::db::schema::run_migrations(&throwaway).await.unwrap();
    drop(throwaway);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrator = sqlx::migrate::Migrator::new(manifest.join("migrations"))
        .await
        .unwrap();
    let mut migrations: Vec<sqlx::migrate::Migration> = migrator.iter().cloned().collect();
    migrations.sort();
    for migration in &migrations {
        if migration.version < before {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "failed to apply migration {} ({}): {e}",
                        migration.version, migration.description
                    )
                });
        }
    }

    pool
}

/// Read a `settings` value, or `None` when the key is absent.
async fn settings_value(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|e| panic!("reading setting '{key}' must not fail: {e}"))
}

/// Assert the fragment keeps its heading first and carries the new line.
fn assert_fragment_contract(value: &str) {
    assert!(
        value.starts_with(HEADING),
        "the fragment must still start with the skill's heading {HEADING:?}; got: {value}"
    );
    for needle in NEW_LINE_NEEDLES {
        assert!(
            value.contains(needle),
            "the fragment must mention {needle:?}; got: {value}"
        );
    }
}

// ── 1: freshly migrated database ────────────────────────────────────────────

/// Scenario: tras migrar, el fragmento contiene la línea nueva y sigue
/// empezando por el encabezado.
#[tokio::test]
async fn test_fragment_contains_the_notifications_line() {
    let pool = setup().await;

    let value = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must exist after migrating");
    assert_fragment_contract(&value);
}

// ── 2: the update respects the user and the empty case ──────────────────────

/// Scenario: la migración actualiza el fragmento todavía sembrado.
#[tokio::test]
async fn test_migration_updates_the_untouched_fragment() {
    let pool = pool_before(PROMPT_VERSION).await;

    // Read the value seeded by `20261008000001` straight from the database;
    // never hardcode it — the assertion is that the migration replaces *that*
    // value.
    let seeded = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must be seeded by 20261008000001");
    assert!(
        !seeded.contains("list_unread"),
        "the pre-migration fragment must not know the notifications line yet; got: {seeded}"
    );

    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let updated = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must still exist after the migration");
    assert_ne!(updated, seeded, "the untouched fragment must be replaced");
    assert_fragment_contract(&updated);
}

/// Scenario: una personalización del usuario no se pisa.
#[tokio::test]
async fn test_migration_respects_a_customized_fragment() {
    let pool = pool_before(PROMPT_VERSION).await;

    sqlx::query(
        "UPDATE settings SET value = 'MI PENDIENTES A MEDIDA' WHERE key = 'SKILL_PENDIENTES_PROMPT'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
            .await
            .as_deref(),
        Some("MI PENDIENTES A MEDIDA"),
        "a customised fragment must be left untouched"
    );
}

/// Scenario: una clave ausente se rellena con el fragmento completo.
#[tokio::test]
async fn test_migration_fills_a_missing_fragment() {
    let pool = pool_before(PROMPT_VERSION).await;

    sqlx::query("DELETE FROM settings WHERE key = 'SKILL_PENDIENTES_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("a missing SKILL_PENDIENTES_PROMPT must be filled by the migration");
    assert_fragment_contract(&value);
}

// ── 3: idempotency ──────────────────────────────────────────────────────────

/// Scenario: re-ejecutar el cuerpo de la migración no duplica la línea.
#[tokio::test]
async fn test_migration_body_is_idempotent() {
    let pool = setup().await;

    let first = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must exist");

    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("re-running the migration body must succeed");

    let second = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must still exist");
    assert_eq!(
        first, second,
        "re-running the migration body must not change the fragment"
    );
    assert_eq!(
        second.matches("- ").count(),
        first.matches("- ").count(),
        "re-running the migration body must not duplicate any line"
    );
}

/// Scenario: el migrador, ejecutado dos veces, deja una sola fila.
#[tokio::test]
async fn test_migration_is_idempotent_through_migrator() {
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

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'SKILL_PENDIENTES_PROMPT'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "the key must exist exactly once");

    let value = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must exist");
    assert_fragment_contract(&value);
    assert_eq!(
        value.matches("list_unread").count(),
        1,
        "the notifications line must not be duplicated"
    );
}
