use sqlx::{Row, SqlitePool};

use crate::models::PersistentMemory;
use crate::persistent_memory::GLOBAL_STATE_ID;

/// Upsert of the single `'global_state'` row. Shared by [`PersistentMemoryRepo::upsert`]
/// and [`PersistentMemoryRepo::upsert_in_tx`] so the SQL exists only once.
const UPSERT_SQL: &str =
    "INSERT INTO persistent_memory (id, payload, updated_at) VALUES (?1, ?2, ?3) \
     ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated_at = excluded.updated_at";

/// Repository for the Capa C `persistent_memory` table: a single logical row
/// (`id = 'global_state'`) holding the versioned JSON state.
pub struct PersistentMemoryRepo;

impl PersistentMemoryRepo {
    /// Insert or update the single `'global_state'` row inside a transaction.
    pub async fn upsert_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        payload: &str,
        updated_at: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(UPSERT_SQL)
            .bind(GLOBAL_STATE_ID)
            .bind(payload)
            .bind(updated_at)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    /// Read the single persistent-memory row.
    ///
    /// The **absence** of the row is not an error: it means "empty state" and
    /// yields `None`. Reading never creates the row.
    pub async fn get(pool: &SqlitePool) -> Result<Option<PersistentMemory>, sqlx::Error> {
        let row =
            sqlx::query("SELECT id, payload, updated_at FROM persistent_memory WHERE id = ?1")
                .bind(GLOBAL_STATE_ID)
                .fetch_optional(pool)
                .await?;

        Ok(row.map(|r| PersistentMemory {
            id: r.get(0),
            payload: r.get(1),
            updated_at: r.get(2),
        }))
    }

    /// Insert or update the single `'global_state'` row.
    ///
    /// `updated_at` is passed in already resolved by the caller (Rust owns the
    /// timestamp rule; see [`crate::persistent_memory`]).
    pub async fn upsert(
        pool: &SqlitePool,
        payload: &str,
        updated_at: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(UPSERT_SQL)
            .bind(GLOBAL_STATE_ID)
            .bind(payload)
            .bind(updated_at)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Update the `'global_state'` row **only if** its current `updated_at`
    /// equals `expected_updated_at`.
    ///
    /// The guard lives in the SQL `WHERE`, so the check and the write are one
    /// atomic statement: no read-then-write race window. Returns the number of
    /// affected rows; `0` means the row is absent or its mark differs, and the
    /// caller must treat it as a conflict.
    pub async fn update_if_mark(
        pool: &SqlitePool,
        payload: &str,
        updated_at: &str,
        expected_updated_at: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE persistent_memory SET payload = ?1, updated_at = ?2 \
             WHERE id = ?3 AND updated_at = ?4",
        )
        .bind(payload)
        .bind(updated_at)
        .bind(GLOBAL_STATE_ID)
        .bind(expected_updated_at)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Insert the `'global_state'` row **only if** it is absent.
    ///
    /// `ON CONFLICT(id) DO NOTHING` makes the absence check and the insert one
    /// atomic statement. Returns the number of affected rows; `0` means a row
    /// already existed and the caller must treat it as a conflict.
    pub async fn insert_if_absent(
        pool: &SqlitePool,
        payload: &str,
        updated_at: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO persistent_memory (id, payload, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(GLOBAL_STATE_ID)
        .bind(payload)
        .bind(updated_at)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Delete the single `'global_state'` row.
    ///
    /// Idempotent: deleting an absent row is a no-op that still succeeds.
    pub async fn delete(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM persistent_memory WHERE id = ?1")
            .bind(GLOBAL_STATE_ID)
            .execute(pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        pool
    }

    async fn row_count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM persistent_memory")
            .fetch_one(pool)
            .await
            .expect("count")
    }

    /// 2.3 — reading an empty state returns `None` without error and without
    /// creating the row.
    #[tokio::test]
    async fn test_get_empty_state_returns_none_without_creating_row() {
        let pool = setup().await;

        let state = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get on an empty table must not error");

        assert!(state.is_none(), "no row ⇒ empty state (None)");
        assert_eq!(row_count(&pool).await, 0, "reading must not create the row");
    }

    /// 2.3 — the upsert creates the `'global_state'` row and a later upsert
    /// updates the very same row (never a second one).
    #[tokio::test]
    async fn test_upsert_creates_then_updates_single_row() {
        let pool = setup().await;

        PersistentMemoryRepo::upsert(&pool, r#"{"schema_version":1}"#, "2026-09-01T10:00:00Z")
            .await
            .expect("first upsert");

        let first = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get")
            .expect("row must exist after upsert");
        assert_eq!(first.id, GLOBAL_STATE_ID);
        assert_eq!(first.payload, r#"{"schema_version":1}"#);
        assert_eq!(first.updated_at, "2026-09-01T10:00:00Z");
        assert_eq!(row_count(&pool).await, 1, "exactly one row");

        PersistentMemoryRepo::upsert(
            &pool,
            r#"{"schema_version":1,"system_rules":["sé breve"]}"#,
            "2026-10-03T08:00:00Z",
        )
        .await
        .expect("second upsert");

        let second = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get")
            .expect("row still exists");
        assert_eq!(second.id, GLOBAL_STATE_ID);
        assert_eq!(
            second.payload,
            r#"{"schema_version":1,"system_rules":["sé breve"]}"#
        );
        assert_eq!(second.updated_at, "2026-10-03T08:00:00Z");
        assert_eq!(
            row_count(&pool).await,
            1,
            "the second upsert must update the row, not add one"
        );
    }

    /// `delete` removes the single row and is idempotent: deleting again is a
    /// no-op that still succeeds.
    #[tokio::test]
    async fn test_delete_removes_row_and_is_idempotent() {
        let pool = setup().await;

        PersistentMemoryRepo::upsert(&pool, r#"{"schema_version":1}"#, "2026-09-01T10:00:00Z")
            .await
            .expect("upsert");
        assert_eq!(row_count(&pool).await, 1);

        PersistentMemoryRepo::delete(&pool).await.expect("delete");
        assert!(
            PersistentMemoryRepo::get(&pool)
                .await
                .expect("get")
                .is_none(),
            "the row must be gone"
        );
        assert_eq!(row_count(&pool).await, 0);

        // Idempotent: no row, still no error and no row created.
        PersistentMemoryRepo::delete(&pool)
            .await
            .expect("second delete must succeed");
        assert_eq!(
            row_count(&pool).await,
            0,
            "deleting an absent row creates nothing"
        );
    }

    /// `update_if_mark` updates only when the current mark matches, and reports
    /// `0` otherwise — the atomic guard behind optimistic concurrency.
    #[tokio::test]
    async fn test_update_if_mark_guards_on_the_mark() {
        let pool = setup().await;
        PersistentMemoryRepo::upsert(&pool, r#"{"a":1}"#, "T1")
            .await
            .expect("seed");

        // Wrong mark: nothing is affected and the row is untouched.
        let affected = PersistentMemoryRepo::update_if_mark(&pool, r#"{"a":2}"#, "T2", "T0")
            .await
            .expect("update with stale mark");
        assert_eq!(affected, 0, "a stale mark must not write");
        let row = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get")
            .expect("row");
        assert_eq!(row.payload, r#"{"a":1}"#);
        assert_eq!(row.updated_at, "T1");

        // Right mark: exactly one row updated.
        let affected = PersistentMemoryRepo::update_if_mark(&pool, r#"{"a":2}"#, "T2", "T1")
            .await
            .expect("update with current mark");
        assert_eq!(affected, 1);
        let row = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get")
            .expect("row");
        assert_eq!(row.payload, r#"{"a":2}"#);
        assert_eq!(row.updated_at, "T2");
    }

    /// `update_if_mark` on an absent row affects nothing (and creates nothing).
    #[tokio::test]
    async fn test_update_if_mark_missing_row_affects_nothing() {
        let pool = setup().await;

        let affected = PersistentMemoryRepo::update_if_mark(&pool, r#"{"a":2}"#, "T2", "T1")
            .await
            .expect("update on empty table");
        assert_eq!(affected, 0);
        assert_eq!(row_count(&pool).await, 0, "no row must be created");
    }

    /// `insert_if_absent` inserts only when there is no row, and reports `0`
    /// otherwise without touching the existing row.
    #[tokio::test]
    async fn test_insert_if_absent_guards_on_absence() {
        let pool = setup().await;

        let affected = PersistentMemoryRepo::insert_if_absent(&pool, r#"{"a":1}"#, "T1")
            .await
            .expect("insert when absent");
        assert_eq!(affected, 1);
        assert_eq!(row_count(&pool).await, 1);

        // A second insert does nothing and leaves the existing row intact.
        let affected = PersistentMemoryRepo::insert_if_absent(&pool, r#"{"a":9}"#, "T9")
            .await
            .expect("insert when present");
        assert_eq!(affected, 0, "an existing row must not be overwritten");
        let row = PersistentMemoryRepo::get(&pool)
            .await
            .expect("get")
            .expect("row");
        assert_eq!(row.payload, r#"{"a":1}"#);
        assert_eq!(row.updated_at, "T1");
    }
}
