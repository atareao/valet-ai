//! Migration tests for the `oximeal-recipe-instructions` change:
//! `migrations/20261109000001_oximeal_recipe_instructions.sql`.
//!
//! The migration is additive and idempotent, with the same «byte-for-byte
//! guard» idiom as `20261104000001_widget_options_contract.sql`: it replaces the
//! `SKILL_MENUS_PROMPT` fragment **only** when it still holds the exact value
//! seeded by `20261107000001_oximeal.sql`, so a user edit survives untouched. It
//! touches no other setting.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Version of `migrations/20261109000001_oximeal_recipe_instructions.sql`.
const PROMPT_VERSION: i64 = 20261109000001;

/// The skill's `prompt_heading` (first line of the fragment).
const HEADING: &str = "# SKILL ACTIVA: MENUS";

/// Substrings the new fragment must carry: the two new fields plus the three
/// mandatory ones it must keep naming.
const NEW_NEEDLES: [&str; 5] = [
    "instructions",
    "prep_time_minutes",
    "name",
    "category",
    "ingredients",
];

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

/// Read the prompt migration SQL from disk.
fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261109000001_oximeal_recipe_instructions.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `oximeal-recipe-instructions` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left in
/// the state the previous release had: `SKILL_MENUS_PROMPT` still holds the value
/// seeded by `20261107000001`, without the instructions guidance. The new
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
    sqlx::query_scalar::<_, Option<String>>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|e| panic!("reading setting '{key}' must not fail: {e}"))
        .flatten()
}

/// Run the migration body against `pool`.
async fn apply_migration(pool: &SqlitePool) {
    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(pool)
        .await
        .expect("the migration body must run");
}

/// Assert the fragment keeps its heading first and carries every needle.
fn assert_fragment_contract(value: &str) {
    assert!(
        value.starts_with(HEADING),
        "the fragment must still start with the skill's heading {HEADING:?}; got: {value}"
    );
    for needle in NEW_NEEDLES {
        assert!(
            value.contains(needle),
            "the fragment must mention {needle:?}; got: {value}"
        );
    }
}

// ── 1: freshly migrated database ────────────────────────────────────────────

/// Scenario: El fragmento sembrado pide las instrucciones y el tiempo.
#[tokio::test]
async fn test_fragment_asks_for_instructions_and_prep_time() {
    let pool = setup().await;

    let value = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must exist after migrating");
    assert_fragment_contract(&value);
}

/// Scenario: El texto por defecto compilado y el sembrado coinciden.
///
/// El fragmento que siembra la migración (embebida en el binario) es exactamente
/// el valor que la base de datos acaba teniendo tras migrar.
#[tokio::test]
async fn test_compiled_migration_literal_matches_the_seeded_value() {
    let pool = setup().await;
    let value = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must exist after migrating");

    let sql = migration_sql();
    assert!(
        sql.contains(&value),
        "the fragment the migration declares must be byte-for-byte the seeded value; got: {value}"
    );
}

// ── 2: the update respects the user ─────────────────────────────────────────

/// Scenario: El fragmento por defecto se actualiza.
#[tokio::test]
async fn test_migration_updates_the_untouched_fragment() {
    let pool = pool_before(PROMPT_VERSION).await;

    // Read the value seeded by `20261107000001` straight from the database;
    // never hardcode it — the assertion is that the migration replaces *that*
    // value.
    let seeded = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must be seeded by 20261107000001");
    assert!(
        !seeded.contains("prep_time_minutes"),
        "the pre-migration fragment must not ask for the prep time yet; got: {seeded}"
    );

    apply_migration(&pool).await;

    let updated = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must still exist after the migration");
    assert_ne!(updated, seeded, "the untouched fragment must be replaced");
    assert_fragment_contract(&updated);
}

/// Scenario: Un fragmento editado por el usuario no se pisa.
#[tokio::test]
async fn test_migration_respects_a_customized_fragment() {
    let pool = pool_before(PROMPT_VERSION).await;

    sqlx::query("UPDATE settings SET value = 'MI MENUS A MEDIDA' WHERE key = 'SKILL_MENUS_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "SKILL_MENUS_PROMPT").await.as_deref(),
        Some("MI MENUS A MEDIDA"),
        "a customised fragment must be left untouched"
    );
}

// ── 3: idempotency ──────────────────────────────────────────────────────────

/// Scenario: La migración es idempotente.
#[tokio::test]
async fn test_migration_body_is_idempotent() {
    let pool = setup().await;

    let first = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must exist");

    apply_migration(&pool).await;

    let second = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must still exist");
    assert_eq!(
        first, second,
        "re-running the migration body must not change the fragment"
    );
}

/// Scenario: El migrador, ejecutado dos veces, deja una sola fila.
#[tokio::test]
async fn test_migration_is_idempotent_through_migrator() {
    let pool = setup().await;
    valet::db::schema::run_migrations(&pool).await.unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = 'SKILL_MENUS_PROMPT'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "the key must exist exactly once");

    let value = settings_value(&pool, "SKILL_MENUS_PROMPT")
        .await
        .expect("SKILL_MENUS_PROMPT must exist");
    assert_fragment_contract(&value);
}
