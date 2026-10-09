use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::models::Profile;

pub struct ProfilesRepo;

/// Default profile name assigned when a profile is created on first use.
///
/// It is the single source of truth for the default value: `get_or_create`
/// binds it and the orchestrator treats a profile still carrying it as
/// unnamed, so it is never injected as the user's name.
pub const DEFAULT_PROFILE_NAME: &str = "Valet User";

impl ProfilesRepo {
    pub async fn get_or_create(pool: &SqlitePool) -> Result<Profile, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, avatar_url, preferences, created_at, updated_at FROM profiles LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let prefs: String = row.get("preferences");
            return Ok(Profile {
                id: row.get("id"),
                name: row.get("name"),
                avatar_url: row.get("avatar_url"),
                preferences: serde_json::from_str(&prefs).unwrap_or(json!({})),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        // Create default profile
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let default_prefs = json!({}).to_string();

        sqlx::query(
            "INSERT INTO profiles (id, name, avatar_url, preferences, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(DEFAULT_PROFILE_NAME)
        .bind(Option::<String>::None)
        .bind(&default_prefs)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        Ok(Profile {
            id,
            name: DEFAULT_PROFILE_NAME.to_string(),
            avatar_url: None,
            preferences: json!({}),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Fetch the profile whose `id` matches, or `None` if it does not exist.
    ///
    /// This is a pure read: it never inserts or updates any row. An unparseable
    /// `preferences` value falls back to an empty JSON object, matching
    /// [`Self::get_or_create`].
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Profile>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, avatar_url, preferences, created_at, updated_at \
             FROM profiles WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| {
            let prefs: String = row.get("preferences");
            Profile {
                id: row.get("id"),
                name: row.get("name"),
                avatar_url: row.get("avatar_url"),
                preferences: serde_json::from_str(&prefs).unwrap_or(json!({})),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }
        }))
    }

    pub async fn update(
        pool: &SqlitePool,
        name: Option<&str>,
        avatar_url: Option<&str>,
        preferences: Option<&Value>,
    ) -> Result<Profile, sqlx::Error> {
        // Ensure a profile exists first, so we can update by ID
        let existing = Self::get_or_create(pool).await?;
        let now = Utc::now().to_rfc3339();

        let new_name = name.unwrap_or(&existing.name);
        let new_avatar = avatar_url.or(existing.avatar_url.as_deref());
        let new_prefs = preferences
            .map(|v| v.to_string())
            .unwrap_or_else(|| existing.preferences.to_string());

        sqlx::query(
            "UPDATE profiles SET name = ?1, avatar_url = ?2, preferences = ?3, updated_at = ?4 WHERE id = ?5",
        )
        .bind(new_name)
        .bind(new_avatar)
        .bind(&new_prefs)
        .bind(&now)
        .bind(&existing.id)
        .execute(pool)
        .await?;

        Ok(Profile {
            id: existing.id,
            name: new_name.to_string(),
            avatar_url: new_avatar.map(|s| s.to_string()),
            preferences: preferences.cloned().unwrap_or(existing.preferences),
            created_at: existing.created_at,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        Ok(pool)
    }

    #[tokio::test]
    async fn test_get_or_create_creates_default() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let profile = ProfilesRepo::get_or_create(&pool).await.unwrap();
        assert_eq!(profile.name, "Valet User");
        assert!(profile.avatar_url.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_get_or_create_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let p1 = ProfilesRepo::get_or_create(&pool).await.unwrap();
        let p2 = ProfilesRepo::get_or_create(&pool).await.unwrap();
        assert_eq!(p1.id, p2.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_profile() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let profile = ProfilesRepo::update(
            &pool,
            Some("New Name"),
            None,
            Some(&json!({"theme": "dark"})),
        )
        .await?;
        assert_eq!(profile.name, "New Name");
        assert_eq!(profile.preferences["theme"], "dark");

        Ok(())
    }

    #[tokio::test]
    async fn test_update_profile_partial_no_name() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let profile = ProfilesRepo::update(&pool, None, None, Some(&json!({"lang": "es"}))).await?;
        assert_eq!(profile.name, "Valet User");
        assert_eq!(profile.preferences["lang"], "es");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_by_id_returns_some_for_existing() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES ('p1','Lorenzo','{}')")
            .execute(&pool)
            .await?;

        let profile = ProfilesRepo::get_by_id(&pool, "p1").await.unwrap();

        let profile = profile.expect("an existing profile must be returned");
        assert_eq!(profile.name, "Lorenzo");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_by_id_returns_none_without_inserting(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;

        let profile = ProfilesRepo::get_by_id(&pool, "nope").await.unwrap();
        assert!(profile.is_none(), "a missing profile must yield None");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profiles")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 0, "get_by_id must not insert a row");

        Ok(())
    }
}
