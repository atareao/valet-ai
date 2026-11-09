//! Migration tests for the `wordpress-credentials-settings` change:
//! `migrations/20261110000001_blog_credentials.sql`.
//!
//! The migration is additive and idempotent, with the same «only overwrite
//! empty/NULL» idiom as the sibling integration migrations. It seeds the Blog
//! connection settings (`blog_base_url`, `blog_service_user`, `blog_app_password`)
//! and touches no other setting; the password is seeded **empty**.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Version of `migrations/20261110000001_blog_credentials.sql`.
const BLOG_VERSION: i64 = 20261110000001;

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

/// Read the Blog migration SQL from disk.
fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261110000001_blog_credentials.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `blog_credentials` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left in
/// the state the previous release had: the Blog settings do not exist yet.
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

/// Guardián de contenido: la migración siembra los tres ajustes con sus defaults.
#[test]
fn test_blog_migration_seeds_the_three_settings() {
    let sql = migration_sql();

    for (key, value) in [
        ("blog_base_url", "https://atareao.es"),
        ("blog_service_user", ""),
        ("blog_app_password", ""),
    ] {
        assert!(
            sql.contains(&format!("VALUES ('{key}', '{value}'")),
            "the migration must seed `{key}` with its default `{value}`, got SQL:\n{sql}"
        );
    }
}

/// Scenario: los ajustes se siembran con sus defaults.
#[tokio::test]
async fn test_blog_settings_seeded() {
    let pool = setup().await;

    assert_eq!(
        settings_value(&pool, "blog_base_url").await.as_deref(),
        Some("https://atareao.es"),
        "the base URL must default to the public blog"
    );
    assert_eq!(
        settings_value(&pool, "blog_service_user").await.as_deref(),
        Some(""),
        "the service user must be seeded empty"
    );
    assert_eq!(
        settings_value(&pool, "blog_app_password").await.as_deref(),
        Some(""),
        "the password must be seeded empty (the user fills it in)"
    );
}

/// Scenario: la migración respeta un valor ya fijado.
#[tokio::test]
async fn test_blog_migration_respects_existing_values() {
    let pool = pool_before(BLOG_VERSION).await;

    // El usuario ya tenía una URL base y una contraseña configuradas.
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('blog_base_url', 'https://mi-blog.example')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('blog_app_password', 'existing-secret')",
    )
    .execute(&pool)
    .await
    .unwrap();

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "blog_base_url").await.as_deref(),
        Some("https://mi-blog.example"),
        "a user-set base URL must survive the migration"
    );
    assert_eq!(
        settings_value(&pool, "blog_app_password").await.as_deref(),
        Some("existing-secret"),
        "a user-set password must survive the migration"
    );
}

/// Scenario: la migración es idempotente; re-ejecutarla no cambia nada.
#[tokio::test]
async fn test_blog_migration_is_idempotent() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = 'https://custom.example' WHERE key = 'blog_base_url'")
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "blog_base_url").await.as_deref(),
        Some("https://custom.example"),
        "re-running the migration must not overwrite a non-empty value"
    );
    assert_eq!(
        settings_value(&pool, "blog_app_password").await.as_deref(),
        Some(""),
        "the password stays empty"
    );
}
