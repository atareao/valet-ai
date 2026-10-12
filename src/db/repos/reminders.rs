use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub profile_id: String,
    pub text: String,
    pub datetime: String,
    pub status: String,
    pub created_at: String,
}

pub struct RemindersRepo;

impl RemindersRepo {
    pub async fn create(pool: &SqlitePool, reminder: &Reminder) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO reminders (id, profile_id, text, datetime, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&reminder.id)
        .bind(&reminder.profile_id)
        .bind(&reminder.text)
        .bind(&reminder.datetime)
        .bind(&reminder.status)
        .bind(&reminder.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Reminder>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, profile_id, text, datetime, status, created_at
             FROM reminders WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(Reminder {
                id: r.get("id"),
                profile_id: r.get("profile_id"),
                text: r.get("text"),
                datetime: r.get("datetime"),
                status: r.get("status"),
                created_at: r.get("created_at"),
            })),
            None => Ok(None),
        }
    }

    pub async fn list(
        pool: &SqlitePool,
        profile_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<Reminder>, sqlx::Error> {
        let mut sql = String::from(
            "SELECT id, profile_id, text, datetime, status, created_at
             FROM reminders WHERE profile_id = ?1",
        );

        if status.is_some() {
            sql.push_str(" AND status = ?2");
        }

        sql.push_str(" ORDER BY datetime ASC");

        // SAFETY: `sql` only concatenates literal fragments; values are bound below.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(profile_id);
        if let Some(s) = status {
            query = query.bind(s);
        }

        let rows = query.fetch_all(pool).await?;

        let reminders: Vec<Reminder> = rows
            .iter()
            .map(|r| Reminder {
                id: r.get("id"),
                profile_id: r.get("profile_id"),
                text: r.get("text"),
                datetime: r.get("datetime"),
                status: r.get("status"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(reminders)
    }

    pub async fn dismiss(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE reminders SET status = 'dismissed' WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Snooze a reminder to a new date/time and reset status to pending.
    pub async fn snooze(
        pool: &SqlitePool,
        id: &str,
        new_datetime: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE reminders SET status = 'pending', datetime = ?1 WHERE id = ?2")
            .bind(new_datetime)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM reminders WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Marca un recordatorio como notificado, fijando `notified_at` a la hora
    /// actual. Es lo que impide reenviar el mismo aviso en el ciclo siguiente.
    pub async fn mark_notified(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE reminders SET notified_at = datetime('now') WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;

        // Create profiles table (needed for FK constraint)
        sqlx::query(
            "CREATE TABLE profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                preferences TEXT NOT NULL DEFAULT '{}'
            )",
        )
        .execute(&pool)
        .await?;

        // Create reminders table matching production schema
        sqlx::query(
            "CREATE TABLE reminders (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL REFERENCES profiles(id),
                text TEXT NOT NULL,
                datetime TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                notified_at TEXT
            )",
        )
        .execute(&pool)
        .await?;

        // Insert test profile
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await?;

        Ok(pool)
    }

    fn sample_reminder(id: &str, text: &str, datetime: &str) -> Reminder {
        Reminder {
            id: id.into(),
            profile_id: "profile-1".into(),
            text: text.into(),
            datetime: datetime.into(),
            status: "pending".into(),
            created_at: "2026-09-23T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn test_create_reminder() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        let reminder = sample_reminder("rem-1", "Comprar leche", "2026-09-24T10:00:00Z");
        RemindersRepo::create(&pool, &reminder).await.unwrap();
        let found = RemindersRepo::find_by_id(&pool, "rem-1").await?.unwrap();
        assert_eq!(found.text, "Comprar leche");
        assert_eq!(found.status, "pending");

        Ok(())
    }

    #[tokio::test]
    async fn test_dismiss_reminder() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        RemindersRepo::create(
            &pool,
            &sample_reminder("rem-2", "Reunión", "2026-09-24T15:00:00Z"),
        )
        .await?;
        RemindersRepo::dismiss(&pool, "rem-2").await.unwrap();
        let found = RemindersRepo::find_by_id(&pool, "rem-2").await?.unwrap();
        assert_eq!(found.status, "dismissed");

        Ok(())
    }

    #[tokio::test]
    async fn test_snooze_reminder() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        RemindersRepo::create(
            &pool,
            &sample_reminder("rem-3", "Llamar", "2026-09-24T10:00:00Z"),
        )
        .await?;
        RemindersRepo::snooze(&pool, "rem-3", "2026-09-24T12:00:00Z").await?;
        let found = RemindersRepo::find_by_id(&pool, "rem-3").await?.unwrap();
        assert_eq!(found.status, "pending");
        assert_eq!(found.datetime, "2026-09-24T12:00:00Z");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_reminders_by_status() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        RemindersRepo::create(&pool, &sample_reminder("r1", "A", "2026-09-24T10:00:00Z")).await?;
        RemindersRepo::create(&pool, &sample_reminder("r2", "B", "2026-09-24T11:00:00Z")).await?;
        RemindersRepo::dismiss(&pool, "r2").await.unwrap();

        let pending = RemindersRepo::list(&pool, "profile-1", Some("pending")).await?;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "r1");

        let dismissed = RemindersRepo::list(&pool, "profile-1", Some("dismissed")).await?;
        assert_eq!(dismissed.len(), 1);
        assert_eq!(dismissed[0].id, "r2");

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_reminder() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        RemindersRepo::create(&pool, &sample_reminder("r3", "C", "2026-09-24T12:00:00Z")).await?;
        RemindersRepo::delete(&pool, "r3").await.unwrap();
        let found = RemindersRepo::find_by_id(&pool, "r3").await.unwrap();
        assert!(found.is_none());

        Ok(())
    }

    /// Lee `notified_at` directamente (el `Reminder` de dominio no lo expone
    /// todavía: lo consume el worker en una fase posterior).
    async fn notified_at(pool: &SqlitePool, id: &str) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>("SELECT notified_at FROM reminders WHERE id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Scenario: se marca `notified_at` de un recordatorio por su `id`, y solo
    /// de ese.
    #[tokio::test]
    async fn reminders_repo_mark_notified_sets_the_timestamp(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup().await?;
        RemindersRepo::create(
            &pool,
            &sample_reminder("rem-notify-1", "Aviso", "2026-09-24T10:00:00Z"),
        )
        .await?;
        RemindersRepo::create(
            &pool,
            &sample_reminder("rem-notify-2", "Otro", "2026-09-24T11:00:00Z"),
        )
        .await?;

        assert!(notified_at(&pool, "rem-notify-1").await.is_none());

        RemindersRepo::mark_notified(&pool, "rem-notify-1").await?;

        assert!(
            notified_at(&pool, "rem-notify-1").await.is_some(),
            "the targeted reminder must be marked as notified"
        );
        assert!(
            notified_at(&pool, "rem-notify-2").await.is_none(),
            "only the targeted reminder must be marked"
        );

        Ok(())
    }
}
