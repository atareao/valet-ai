//! Migration tests for `migrations/20261128000001_mastodon.sql`: ajustes
//! vacíos, prompt con guarda byte a byte y tabla `mastodon_seen`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

const MASTODON_VERSION: i64 = 20261128000001;
const HEADING: &str = "# SKILL ACTIVA: MASTODON";

async fn empty_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory pool")
}

async fn setup() -> SqlitePool {
    let pool = empty_pool().await;
    valet::db::schema::run_migrations(&pool)
        .await
        .expect("migrations");
    pool
}

fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261128000001_mastodon.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("migration missing at {}: {e}", path.display()))
}

async fn pool_before() -> SqlitePool {
    let throwaway = setup().await;
    drop(throwaway);

    let pool = empty_pool().await;
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrator = sqlx::migrate::Migrator::new(manifest.join("migrations"))
        .await
        .expect("migrator");
    let mut migrations: Vec<sqlx::migrate::Migration> = migrator.iter().cloned().collect();
    migrations.sort();
    for migration in migrations {
        if migration.version < MASTODON_VERSION {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("applying migration {}: {e}", migration.version));
        }
    }
    pool
}

async fn settings_value(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|e| panic!("reading '{key}': {e}"))
        .flatten()
}

async fn apply_migration(pool: &SqlitePool) {
    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(pool)
        .await
        .expect("re-executing the migration must succeed");
}

#[test]
fn migration_declares_the_expected_contract() {
    let sql = migration_sql();
    assert!(sql.contains("VALUES ('mastodon_base_url', ''"));
    assert!(sql.contains("VALUES ('mastodon_token', ''"));
    assert!(sql.contains("VALUES ('SKILL_MASTODON_PROMPT'"));
    assert!(sql.contains("CREATE TABLE IF NOT EXISTS mastodon_seen"));
    assert!(sql.contains("notification_id TEXT PRIMARY KEY"));
    assert!(sql.contains("created_at"));
}

#[tokio::test]
async fn first_execution_seeds_settings_prompt_and_dedupe_table() {
    let pool = pool_before().await;
    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "mastodon_base_url").await.as_deref(),
        Some("")
    );
    assert_eq!(
        settings_value(&pool, "mastodon_token").await.as_deref(),
        Some("")
    );
    let prompt = settings_value(&pool, "SKILL_MASTODON_PROMPT")
        .await
        .expect("prompt seeded");
    assert_eq!(prompt.lines().next(), Some(HEADING));
    for needle in ["list_mentions", "reply", "aprobación", "direct"] {
        assert!(prompt.contains(needle), "el prompt debe mencionar {needle}");
    }

    let columns = sqlx::query("PRAGMA table_info(mastodon_seen)")
        .fetch_all(&pool)
        .await
        .expect("pragma");
    assert!(!columns.is_empty(), "la tabla debe existir");
    let table_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'mastodon_seen'",
    )
    .fetch_one(&pool)
    .await
    .expect("definition");
    assert!(table_sql.contains("notification_id TEXT PRIMARY KEY"));
    assert!(table_sql.contains("created_at"));
}

#[tokio::test]
async fn reexecution_preserves_a_user_edited_prompt_byte_for_byte() {
    let pool = setup().await;
    let edited = format!("{HEADING}\n\nTexto cerrado editado por el usuario.\n");
    valet::db::repos::settings::SettingsRepo::set(&pool, "SKILL_MASTODON_PROMPT", &edited)
        .await
        .expect("seed edited prompt");

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "SKILL_MASTODON_PROMPT")
            .await
            .as_deref(),
        Some(edited.as_str())
    );
}

#[tokio::test]
async fn reexecution_preserves_existing_settings_and_rows() {
    let pool = setup().await;
    valet::db::repos::settings::SettingsRepo::set(
        &pool,
        "mastodon_base_url",
        "https://social.example",
    )
    .await
    .expect("seed url");
    sqlx::query(
        "INSERT INTO mastodon_seen (notification_id, status_id, visibility) VALUES ('n1', 's1', 'direct')",
    )
    .execute(&pool)
    .await
    .expect("seed seen row");

    apply_migration(&pool).await;
    valet::db::schema::run_migrations(&pool)
        .await
        .expect("run_migrations is idempotent");

    assert_eq!(
        settings_value(&pool, "mastodon_base_url").await.as_deref(),
        Some("https://social.example")
    );
    let visibility: String =
        sqlx::query_scalar("SELECT visibility FROM mastodon_seen WHERE notification_id = 'n1'")
            .fetch_one(&pool)
            .await
            .expect("row survives");
    assert_eq!(visibility, "direct");
}
