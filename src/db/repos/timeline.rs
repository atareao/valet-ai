use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::models::timeline::{NewTimelineEvent, TimelineEvent};

/// Persistence for the `timeline_events` table.
pub struct TimelineRepo;

impl TimelineRepo {
    /// Insert a batch **through any caller-supplied executor**, so the worker's
    /// transaction can be shared. An empty batch executes nothing.
    pub async fn insert_many<'e, E>(
        executor: E,
        events: &[NewTimelineEvent],
    ) -> Result<(), sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        if events.is_empty() {
            return Ok(());
        }

        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO timeline_events (id, timestamp, category, fact, source_message_id) ",
        );
        qb.push_values(events, |mut row, event| {
            row.push_bind(&event.id)
                .push_bind(&event.timestamp)
                .push_bind(&event.category)
                .push_bind(&event.fact)
                .push_bind(&event.source_message_id);
        });
        qb.build().execute(executor).await?;
        Ok(())
    }

    /// Most recent events first. `start`/`end`/`category` are optional and both
    /// range ends are inclusive (lexicographic comparison of ISO 8601 strings).
    pub async fn list(
        pool: &SqlitePool,
        start: Option<&str>,
        end: Option<&str>,
        category: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TimelineEvent>, sqlx::Error> {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT id, timestamp, category, fact, source_message_id, created_at \
             FROM timeline_events WHERE 1 = 1",
        );
        if let Some(start) = start {
            qb.push(" AND timestamp >= ").push_bind(start);
        }
        if let Some(end) = end {
            qb.push(" AND timestamp <= ").push_bind(end);
        }
        if let Some(category) = category {
            qb.push(" AND category = ").push_bind(category);
        }
        qb.push(" ORDER BY timestamp DESC, created_at DESC LIMIT ")
            .push_bind(limit);

        qb.build_query_as::<TimelineEvent>().fetch_all(pool).await
    }

    /// `true` when a row was deleted, `false` when the id did not exist.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM timeline_events WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
