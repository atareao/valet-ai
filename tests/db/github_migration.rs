//! Migration tests for the `github-integration` change:
//! `migrations/20261111000001_github.sql`.
//!
//! The migration is additive and idempotent, with the same «only overwrite
//! empty/NULL» idiom as the sibling integration migrations. It seeds the GitHub
//! connection settings (`github_default_repo`, `github_default_labels`) and the
//! `SKILL_GITHUB_PROMPT` fragment; the token is **not** seeded (sensitive).

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Version of `migrations/20261111000001_github.sql`.
const GITHUB_VERSION: i64 = 20261111000001;

/// The skill's `prompt_heading` (first line of the fragment).
const HEADING: &str = "# SKILL ACTIVA: GITHUB";

/// A database with every migration applied (`run_migrations()`).
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

/// Read the GitHub migration SQL from disk.
fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261111000001_github.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `github` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left in
/// the state the previous release had: the GitHub settings do not exist yet.
async fn pool_before(before: i64) -> SqlitePool {
    // `run_migrations` calls `register_vec_extension()` before anything else;
    // `sqlite3_auto_extension` is process-global but only reaches connections
    // opened *after* it runs. A throwaway pool triggers the registration.
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

/// Guardián de contenido: la migración siembra los ajustes con sus defaults y
/// **no** siembra el token.
#[test]
fn test_github_migration_seeds_the_settings() {
    let sql = migration_sql();

    assert!(
        sql.contains("VALUES ('github_default_repo', 'atareao/valet-ai'"),
        "the migration must seed `github_default_repo` with its default, got SQL:\n{sql}"
    );
    assert!(
        sql.contains("VALUES ('github_default_labels', ''"),
        "the migration must seed `github_default_labels` empty, got SQL:\n{sql}"
    );
    assert!(
        sql.contains("VALUES ('SKILL_GITHUB_PROMPT'"),
        "the migration must seed `SKILL_GITHUB_PROMPT`, got SQL:\n{sql}"
    );
    assert!(
        !sql.contains("'github_token'"),
        "the migration must NOT seed the sensitive `github_token`"
    );
}

/// Scenario: los ajustes se siembran con sus defaults.
#[tokio::test]
async fn test_github_settings_seeded() {
    let pool = setup().await;

    assert_eq!(
        settings_value(&pool, "github_default_repo")
            .await
            .as_deref(),
        Some("atareao/valet-ai"),
        "the default repo must be seeded"
    );
    assert_eq!(
        settings_value(&pool, "github_default_labels")
            .await
            .as_deref(),
        Some(""),
        "the default labels must be seeded empty"
    );
    let token = settings_value(&pool, "github_token").await;
    assert!(
        token.unwrap_or_default().is_empty(),
        "the token must be empty (the user fills it in)"
    );
}

/// Scenario: la migración respeta un valor ya fijado.
#[tokio::test]
async fn test_github_migration_respects_existing_values() {
    let pool = pool_before(GITHUB_VERSION).await;

    sqlx::query("INSERT INTO settings (key, value) VALUES ('github_default_repo', 'otro/repo')")
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "github_default_repo")
            .await
            .as_deref(),
        Some("otro/repo"),
        "a user-set default repo must survive the migration"
    );
}

/// Scenario: El fragmento sembrado tiene su encabezado.
#[tokio::test]
async fn test_github_fragment_has_its_heading() {
    let pool = setup().await;

    let value = settings_value(&pool, "SKILL_GITHUB_PROMPT")
        .await
        .expect("SKILL_GITHUB_PROMPT must exist after migrating");
    assert!(
        value.lines().next() == Some(HEADING),
        "the fragment must start with its heading {HEADING:?}; got: {value}"
    );
    for needle in ["issue", "repositorio", "labels"] {
        assert!(
            value.contains(needle),
            "the fragment must mention {needle:?}; got: {value}"
        );
    }
}

/// Scenario: Un fragmento editado por el usuario no se pisa.
#[tokio::test]
async fn test_github_migration_respects_a_customized_fragment() {
    let pool = pool_before(GITHUB_VERSION).await;

    sqlx::query(
        "UPDATE settings SET value = 'MI GITHUB A MEDIDA' WHERE key = 'SKILL_GITHUB_PROMPT'",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Standard migration only overwrites empty/NULL, so a non-empty value of the
    // previous release state must be preserved. First seed the fragment as the
    // previous state would have it (absent → the migration should fill it), then
    // re-run to prove idempotency after a user edit.
    apply_migration(&pool).await;
    sqlx::query(
        "UPDATE settings SET value = 'MI GITHUB A MEDIDA' WHERE key = 'SKILL_GITHUB_PROMPT'",
    )
    .execute(&pool)
    .await
    .unwrap();
    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "SKILL_GITHUB_PROMPT")
            .await
            .as_deref(),
        Some("MI GITHUB A MEDIDA"),
        "a customised fragment must be left untouched"
    );
}

/// Scenario: La migración es idempotente.
#[tokio::test]
async fn test_github_migration_is_idempotent() {
    let pool = setup().await;

    let first_repo = settings_value(&pool, "github_default_repo").await;
    let first_labels = settings_value(&pool, "github_default_labels").await;
    let first_prompt = settings_value(&pool, "SKILL_GITHUB_PROMPT").await;

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "github_default_repo").await,
        first_repo
    );
    assert_eq!(
        settings_value(&pool, "github_default_labels").await,
        first_labels
    );
    assert_eq!(
        settings_value(&pool, "SKILL_GITHUB_PROMPT").await,
        first_prompt
    );
}
