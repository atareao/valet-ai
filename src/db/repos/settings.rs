use sqlx::Row;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub struct SettingsRepo;

impl SettingsRepo {
    /// Get a single setting by key. Returns None if not found.
    pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
        Ok(row.map(|r| r.get(0)))
    }

    /// Set a setting (insert or update). Sets updated_at to current timestamp.
    pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get all settings as a HashMap.
    pub async fn get_all(pool: &SqlitePool) -> Result<HashMap<String, String>, sqlx::Error> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(pool)
            .await?;
        let mut map = HashMap::new();
        for row in rows {
            let k: String = row.get(0);
            let v: String = row.get(1);
            map.insert(k, v);
        }
        Ok(map)
    }

    /// Delete a setting by key.
    pub async fn delete(pool: &SqlitePool, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM settings WHERE key = ?1")
            .bind(key)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Seed default settings values.
    ///
    /// Note: `system_prompt`, `archivist_prompt` and `collapse_prompt` are
    /// seeded by migration `20260929000001_prompts.sql`, not here.
    pub async fn seed_defaults(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")
            .bind("max_window_tokens")
            .bind("10000")
            .execute(pool)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")
            .bind("message_page_size")
            .bind("50")
            .execute(pool)
            .await?;
        // API keys for external services (set via UI, fallback to ENV)
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")
            .bind("google_places_api_key")
            .bind("")
            .execute(pool)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")
            .bind("brave_search_api_key")
            .bind("")
            .execute(pool)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)")
            .bind("openweather_api_key")
            .bind("")
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        SettingsRepo::seed_defaults(&pool).await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn test_get_default_value() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let value = SettingsRepo::get(&pool, "max_window_tokens").await.unwrap();
        assert_eq!(value, Some("10000".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let value = SettingsRepo::get(&pool, "nonexistent").await.unwrap();
        assert_eq!(value, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_set_inserts_new() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        SettingsRepo::set(&pool, "foo", "bar").await.unwrap();
        let value = SettingsRepo::get(&pool, "foo").await.unwrap();
        assert_eq!(value, Some("bar".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_set_updates_existing() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        SettingsRepo::set(&pool, "foo", "bar").await.unwrap();
        SettingsRepo::set(&pool, "foo", "baz").await.unwrap();
        let value = SettingsRepo::get(&pool, "foo").await.unwrap();
        assert_eq!(value, Some("baz".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let all = SettingsRepo::get_all(&pool).await.unwrap();
        assert!(all.contains_key("max_window_tokens"));
        assert!(all.contains_key("system_prompt"));
        assert!(all.contains_key("collapse_prompt"));

        Ok(())
    }

    #[tokio::test]
    async fn test_collapse_prompt_seeded() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let value = SettingsRepo::get(&pool, "collapse_prompt").await.unwrap();
        assert!(
            value.is_some(),
            "collapse_prompt should be seeded after migrations"
        );
        let prompt = value.unwrap();
        assert!(
            !prompt.is_empty(),
            "collapse_prompt should have a non-empty default value"
        );
        assert!(
            prompt.contains("Resume"),
            "Default collapse_prompt should be in Spanish, containing 'Resume'"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        SettingsRepo::set(&pool, "foo", "bar").await.unwrap();
        SettingsRepo::delete(&pool, "foo").await.unwrap();
        let value = SettingsRepo::get(&pool, "foo").await.unwrap();
        assert_eq!(value, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_message_page_size_seeded() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let value = SettingsRepo::get(&pool, "message_page_size").await.unwrap();
        assert_eq!(value, Some("50".to_string()));

        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED phase — generation params settings (seeding + preservation)
    //
    // These tests will FAIL (runtime) because the migration that seeds the
    // twelve `GENERATION_*` keys does not exist yet.
    // -----------------------------------------------------------------------

    /// Locate the generation-params migration by content (the filename is not
    /// fixed) and return its SQL.
    fn generation_migration_sql() -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut found: Option<String> = None;
        for entry in std::fs::read_dir(&dir).expect("migrations directory must exist") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            let content = std::fs::read_to_string(&path).expect("readable migration file");
            if content.contains("GENERATION_CHAT_TEMPERATURE") {
                found = Some(content);
                break;
            }
        }
        found.expect(
            "a migration seeding the GENERATION_* keys (GENERATION_CHAT_TEMPERATURE) must exist",
        )
    }

    /// Scenario: Las doce claves se siembran con sus defaults
    #[tokio::test]
    async fn test_generation_settings_seeded_with_defaults(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;

        let expected = [
            ("GENERATION_CHAT_TEMPERATURE", "0.7"),
            ("GENERATION_CHAT_REASONING", ""),
            ("GENERATION_CHAT_MAX_TOKENS", "4096"),
            ("GENERATION_COLLAPSE_TEMPERATURE", "0.2"),
            ("GENERATION_COLLAPSE_REASONING", "off"),
            ("GENERATION_COLLAPSE_MAX_TOKENS", "1024"),
            ("GENERATION_MEMORY_TEMPERATURE", "0.3"),
            ("GENERATION_MEMORY_REASONING", "off"),
            ("GENERATION_MEMORY_MAX_TOKENS", "1024"),
            ("GENERATION_SEMANTIC_TEMPERATURE", "0.1"),
            ("GENERATION_SEMANTIC_REASONING", "off"),
            ("GENERATION_SEMANTIC_MAX_TOKENS", "2048"),
        ];

        for (key, value) in expected {
            assert_eq!(
                SettingsRepo::get(&pool, key).await.unwrap(),
                Some(value.to_string()),
                "after migrations, settings.{key} must hold its initial value"
            );
        }

        Ok(())
    }

    /// Scenario: Un valor existente se respeta
    #[tokio::test]
    async fn test_generation_setting_existing_value_is_respected(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;

        // A user-customised, non-empty value present before the migration runs.
        SettingsRepo::set(&pool, "GENERATION_CHAT_TEMPERATURE", "0.9").await?;

        // Re-run the generation migration; it must not clobber non-empty values.
        let sql = generation_migration_sql();
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await?;

        assert_eq!(
            SettingsRepo::get(&pool, "GENERATION_CHAT_TEMPERATURE").await?,
            Some("0.9".to_string()),
            "a non-empty pre-existing generation setting must be preserved"
        );

        Ok(())
    }
}
