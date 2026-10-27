//! Migration tests for the `zennotes-notes` change:
//! `migrations/20261026000001_zennotes_skill.sql`.
//!
//! The migration is additive and idempotent, using the same «only overwrite
//! empty/NULL» idiom as the other `*_skill`/`*_prompts` migrations. It seeds the
//! ZenNotes connection settings (`zennotes_base_url`, `zennotes_folder`,
//! `zennotes_token`), the skill enablement flag (`ROUTER_SKILL_ZENNOTES_ENABLED`)
//! and the skill's prompt fragment (`SKILL_ZENNOTES_PROMPT`) without clobbering
//! a value the user already set.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

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

/// Read the ZenNotes-skill migration SQL from disk.
fn zennotes_skill_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261026000001_zennotes_skill.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
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

/// The keys the migration is expected to seed.
const SEEDED_KEYS: [&str; 5] = [
    "zennotes_base_url",
    "zennotes_folder",
    "zennotes_token",
    "ROUTER_SKILL_ZENNOTES_ENABLED",
    "SKILL_ZENNOTES_PROMPT",
];

/// Tarea 3.1 — guardián de contenido: la migración nombra las cinco claves con
/// sus defaults esperados.
#[test]
fn test_zennotes_migration_seeds_the_five_keys() {
    let sql = zennotes_skill_migration_sql();
    for key in SEEDED_KEYS {
        assert!(
            sql.contains(key),
            "the migration must seed `{key}`, got SQL:\n{sql}"
        );
    }
    for (key, value) in [
        ("zennotes_base_url", "https://zennotes.territoriolinux.es"),
        ("zennotes_folder", "Valet"),
        ("ROUTER_SKILL_ZENNOTES_ENABLED", "true"),
    ] {
        assert!(
            sql.contains(&format!("VALUES ('{key}', '{value}'")),
            "the migration must seed `{key}` with its default `{value}`, got SQL:\n{sql}"
        );
    }
}

/// Scenario: Los ajustes se siembran con sus defaults.
#[tokio::test]
async fn test_zennotes_migration_seeds_defaults() {
    let pool = setup().await;

    assert_eq!(
        setting_value(&pool, "zennotes_base_url").await,
        "https://zennotes.territoriolinux.es",
        "the base URL must default to the public ZenNotes instance"
    );
    assert_eq!(
        setting_value(&pool, "zennotes_folder").await,
        "Valet",
        "the dedicated folder must default to `Valet`"
    );
    assert_eq!(
        setting_value(&pool, "zennotes_token").await,
        "",
        "the token must be seeded empty (the user fills it in)"
    );
    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_ZENNOTES_ENABLED").await,
        "true",
        "the zennotes skill must be enabled by default"
    );

    let prompt = setting_value(&pool, "SKILL_ZENNOTES_PROMPT").await;
    assert!(!prompt.trim().is_empty(), "the fragment must not be empty");
    assert_eq!(
        prompt.lines().next(),
        Some("# SKILL ACTIVA: ZENNOTES"),
        "the fragment's first line must be its section heading"
    );
}

/// Scenario: La migración respeta un valor ya fijado. Reaplicar el cuerpo de la
/// migración no pisa un valor no vacío del usuario, pero sí rellena uno vacío.
#[tokio::test]
async fn test_zennotes_migration_respects_user_values() {
    let pool = setup().await;

    for (key, value) in [
        ("zennotes_folder", "MiVault"),
        ("zennotes_token", "user-token"),
        ("ROUTER_SKILL_ZENNOTES_ENABLED", "false"),
        ("SKILL_ZENNOTES_PROMPT", "MI PROMPT PERSONALIZADO"),
    ] {
        sqlx::query("UPDATE settings SET value = ?1 WHERE key = ?2")
            .bind(value)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }

    let sql = zennotes_skill_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "zennotes_folder").await,
        "MiVault",
        "a user-chosen folder must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "zennotes_token").await,
        "user-token",
        "a user-set token must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_ZENNOTES_ENABLED").await,
        "false",
        "a disabled-by-the-user skill must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "SKILL_ZENNOTES_PROMPT").await,
        "MI PROMPT PERSONALIZADO",
        "a non-empty custom prompt must be preserved"
    );

    // Un valor vacío, en cambio, se rellena con el de por defecto.
    sqlx::query("UPDATE settings SET value = '' WHERE key = 'zennotes_folder'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "zennotes_folder").await,
        "Valet",
        "an empty value must be backfilled with the default"
    );
}

/// Scenario: reaplicar el cuerpo de la migración es idempotente (una fila por
/// clave y los mismos valores).
#[tokio::test]
async fn test_zennotes_migration_is_idempotent() {
    let pool = setup().await;

    let sql = zennotes_skill_migration_sql();
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();
    }

    for key in SEEDED_KEYS {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "expected exactly one row for key '{key}'");
    }

    assert_eq!(
        setting_value(&pool, "zennotes_folder").await,
        "Valet",
        "re-applying the migration must not change the folder"
    );
    assert_eq!(
        setting_value(&pool, "SKILL_ZENNOTES_PROMPT")
            .await
            .lines()
            .next(),
        Some("# SKILL ACTIVA: ZENNOTES"),
        "re-applying the migration must not change the fragment heading"
    );
}
