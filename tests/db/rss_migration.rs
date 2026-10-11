//! Migration tests for the `rss` skill seed:
//! `migrations/20261010000007_rss_skill.sql`. The migration seeds the
//! skill's enablement flag, its prompt fragment and the Miniflux connection
//! settings, using the same "only overwrite empty/NULL" idiom as the other
//! `*_skill`/`*_prompts` migrations.

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

/// Read the rss-skill migration SQL from disk.
fn rss_skill_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261010000007_rss_skill.sql");
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

/// 1) After migrating, the four rss-skill keys carry their defaults.
#[tokio::test]
async fn test_rss_migration_seeds_defaults() {
    let pool = setup().await;

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_RSS_ENABLED").await,
        "true",
        "the rss skill must be enabled by default"
    );

    let prompt = setting_value(&pool, "SKILL_RSS_PROMPT").await;
    assert!(!prompt.trim().is_empty(), "the fragment must not be empty");
    assert_eq!(
        prompt.lines().next(),
        Some("# SKILL ACTIVA: RSS"),
        "the fragment's first line must be its section heading"
    );

    assert_eq!(
        setting_value(&pool, "miniflux_base_url").await,
        "https://miniflux.territoriolinux.es",
        "the miniflux base url must point at the default host"
    );

    let api_key = setting_value_opt(&pool, "miniflux_api_key").await;
    assert!(
        api_key.is_some(),
        "miniflux_api_key must exist after migration"
    );
    assert_eq!(
        api_key.as_deref(),
        Some(""),
        "miniflux_api_key must be seeded empty (the user fills it in)"
    );
}

/// 2) Applying the migration twice keeps the same values and one row per key.
#[tokio::test]
async fn test_rss_migration_is_idempotent() {
    let pool = setup().await;

    let sql = rss_skill_migration_sql();
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();
    }

    for key in [
        "ROUTER_SKILL_RSS_ENABLED",
        "SKILL_RSS_PROMPT",
        "miniflux_base_url",
        "miniflux_api_key",
    ] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "expected exactly one row for key '{key}'");
    }

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_RSS_ENABLED").await,
        "true",
        "re-applying the migration must not change the enablement flag"
    );
    assert_eq!(
        setting_value(&pool, "miniflux_base_url").await,
        "https://miniflux.territoriolinux.es",
        "re-applying the migration must not change the base url"
    );
}

/// 3) A user-written value survives a re-run of the migration body; an empty
/// value is backfilled with the default.
#[tokio::test]
async fn test_rss_migration_respects_user_values() {
    let pool = setup().await;

    for (key, value) in [
        ("ROUTER_SKILL_RSS_ENABLED", "false"),
        ("SKILL_RSS_PROMPT", "MI PROMPT PERSONALIZADO"),
        ("miniflux_base_url", "https://miniflux.mi-dominio.example"),
        ("miniflux_api_key", "mi-token-personal"),
    ] {
        sqlx::query("UPDATE settings SET value = ?1 WHERE key = ?2")
            .bind(value)
            .bind(key)
            .execute(&pool)
            .await
            .unwrap();
    }

    let sql = rss_skill_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_RSS_ENABLED").await,
        "false",
        "a disabled-by-the-user skill must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "SKILL_RSS_PROMPT").await,
        "MI PROMPT PERSONALIZADO",
        "a non-empty custom prompt must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "miniflux_base_url").await,
        "https://miniflux.mi-dominio.example",
        "a non-empty custom base url must be preserved"
    );
    assert_eq!(
        setting_value(&pool, "miniflux_api_key").await,
        "mi-token-personal",
        "a user-provided api key must be preserved"
    );

    // An empty value, on the other hand, is backfilled with the default.
    sqlx::query("UPDATE settings SET value = '' WHERE key = 'miniflux_base_url'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "miniflux_base_url").await,
        "https://miniflux.territoriolinux.es",
        "an empty value must be backfilled with the default"
    );
}

/// 4) The migration only seeds its own keys: the email skill's settings are
/// left untouched.
#[tokio::test]
async fn test_rss_migration_does_not_touch_others() {
    let pool = setup().await;

    assert!(
        !setting_value(&pool, "SKILL_EMAIL_PROMPT").await.is_empty(),
        "SKILL_EMAIL_PROMPT must survive the rss migration"
    );
    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_EMAIL_ENABLED").await,
        "true",
        "ROUTER_SKILL_EMAIL_ENABLED must survive the rss migration"
    );
}
