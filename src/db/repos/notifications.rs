//! Persistencia de la bandeja de notificaciones (`notifications`).
//!
//! El `INSERT` precede a la respuesta de la ingesta; el estado de lectura se
//! gestiona con un `UPDATE` idempotente sobre las notificaciones no leídas.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

/// Una notificación de la bandeja.
///
/// `metadata` llega como el JSON serializado tal cual se guardó (`TEXT`) o como
/// `None` si la columna es NULL; parsearlo a un objeto JSON es cosa de la ruta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notification {
    pub id: String,
    pub origin: String,
    pub author: String,
    pub message: String,
    pub level: String,
    pub metadata: Option<String>,
    pub read_at: Option<String>,
    pub created_at: String,
}

pub struct NotificationsRepo;

impl NotificationsRepo {
    /// Inserta una notificación y devuelve su `id` (UUIDv4).
    ///
    /// La fila nace con `read_at` NULL; `created_at` lo pone la base de datos.
    pub async fn insert(
        pool: &SqlitePool,
        origin: &str,
        author: &str,
        message: &str,
        level: &str,
        metadata: Option<&str>,
    ) -> Result<String, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO notifications (id, origin, author, message, level, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(origin)
        .bind(author)
        .bind(message)
        .bind(level)
        .bind(metadata)
        .execute(pool)
        .await?;
        Ok(id)
    }

    /// Devuelve una notificación por su `id`, o `None` si no existe.
    pub async fn get_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<Notification>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, origin, author, message, level, metadata, read_at, created_at
             FROM notifications WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| Notification {
            id: r.get("id"),
            origin: r.get("origin"),
            author: r.get("author"),
            message: r.get("message"),
            level: r.get("level"),
            metadata: r.get("metadata"),
            read_at: r.get("read_at"),
            created_at: r.get("created_at"),
        }))
    }

    /// Lista las notificaciones no leídas, más recientes primero, junto con el
    /// cursor de continuación (`None` si no hay más páginas) y el número total
    /// de no leídas (independiente del `limit`).
    ///
    /// El cursor es keyset y codifica `created_at` y `rowid` como
    /// `"<created_at>|<rowid>"`, de modo que dos filas con el mismo
    /// `created_at` (la granularidad de la columna es de segundo) no se saltan
    /// al paginar.
    pub async fn list_unread(
        pool: &SqlitePool,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Notification>, Option<String>, i64), sqlx::Error> {
        let _ = cursor; // RED: paginación por cursor sin implementar
        let rows = sqlx::query(
            "SELECT id, origin, author, message, level, metadata, read_at, created_at
             FROM notifications
             WHERE read_at IS NULL
             ORDER BY created_at DESC, rowid DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let items = rows
            .iter()
            .map(|r| Notification {
                id: r.get("id"),
                origin: r.get("origin"),
                author: r.get("author"),
                message: r.get("message"),
                level: r.get("level"),
                metadata: r.get("metadata"),
                read_at: r.get("read_at"),
                created_at: r.get("created_at"),
            })
            .collect();

        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE read_at IS NULL")
                .fetch_one(pool)
                .await?;

        Ok((items, None, total))
    }

    /// Marca como leídas las notificaciones no leídas cuyos `id` figuran en
    /// `ids`, y devuelve cuántas filas cambiaron.
    ///
    /// Idempotente: repetir la llamada devuelve `0` sin tocar los `read_at` ya
    /// fijados. Los ids inexistentes se ignoran y una lista vacía es un no-op.
    pub async fn mark_read(pool: &SqlitePool, ids: &[String]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        // Build the `IN (...)` placeholders dynamically (`?,?,?`) with
        // `QueryBuilder`, so the ids are always passed as bound parameters and
        // never interpolated into the SQL.
        let mut builder = sqlx::QueryBuilder::new(
            "UPDATE notifications SET read_at = datetime('now') \
             WHERE read_at IS NULL AND id IN (",
        );
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected())
    }

    /// Marca como leídas TODAS las notificaciones no leídas y devuelve cuántas
    /// filas cambiaron. Idempotente: la segunda llamada devuelve 0.
    pub async fn mark_all_read(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let _ = pool; // RED: sin implementar.
        Ok(0)
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
            .unwrap();
        crate::db::schema::run_migrations(&pool).await.unwrap();
        pool
    }

    /// Scenario: una notificación nace sin leer, con `level = 'info'` y
    /// `created_at` puesto por la base de datos.
    #[tokio::test]
    async fn notifications_repo_insert_persists_unread_info_row() {
        let pool = setup().await;

        let id = NotificationsRepo::insert(
            &pool,
            "monitor",
            "backup",
            "el respaldo terminó",
            "info",
            None,
        )
        .await
        .unwrap();

        assert!(!id.is_empty(), "the returned id must not be empty");

        let row = NotificationsRepo::get_by_id(&pool, &id)
            .await
            .unwrap()
            .expect("the inserted row must be readable");

        assert_eq!(row.id, id);
        assert_eq!(row.origin, "monitor");
        assert_eq!(row.author, "backup");
        assert_eq!(row.message, "el respaldo terminó");
        assert_eq!(row.level, "info");
        assert!(row.read_at.is_none(), "a fresh notification is unread");
        assert!(!row.created_at.is_empty(), "created_at must be set");
    }

    /// Scenario: `metadata` admite ausencia (NULL) y objeto JSON serializado.
    #[tokio::test]
    async fn notifications_repo_insert_metadata_null_or_object() {
        let pool = setup().await;

        let without = NotificationsRepo::insert(&pool, "o", "a", "sin metadata", "info", None)
            .await
            .unwrap();
        let row = NotificationsRepo::get_by_id(&pool, &without)
            .await
            .unwrap()
            .unwrap();
        assert!(row.metadata.is_none(), "absent metadata must be NULL");

        let with = NotificationsRepo::insert(
            &pool,
            "o",
            "a",
            "con metadata",
            "info",
            Some(r#"{"source":"cron"}"#),
        )
        .await
        .unwrap();
        let row = NotificationsRepo::get_by_id(&pool, &with)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.metadata.as_deref(), Some(r#"{"source":"cron"}"#));
    }

    /// Scenario: solo se listan las no leídas, más recientes primero, con el
    /// total de no leídas (no el de devueltas) y el `limit` respetado.
    #[tokio::test]
    async fn notifications_repo_list_unread_only_unread_newest_first() {
        let pool = setup().await;

        let n1 = NotificationsRepo::insert(&pool, "o", "a", "primera", "info", None)
            .await
            .unwrap();
        let n2 = NotificationsRepo::insert(&pool, "o", "a", "segunda", "info", None)
            .await
            .unwrap();
        let n3 = NotificationsRepo::insert(&pool, "o", "a", "tercera", "info", None)
            .await
            .unwrap();

        // `created_at` is `datetime('now')` at second granularity, so equal
        // timestamps would make the order non-deterministic; pin distinct
        // values by direct SQL to assert ordering by date.
        sqlx::query("UPDATE notifications SET created_at = '2020-01-01 00:00:01' WHERE id = ?1")
            .bind(&n1)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notifications SET created_at = '2020-01-01 00:00:02' WHERE id = ?1")
            .bind(&n2)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE notifications SET created_at = '2020-01-01 00:00:03' WHERE id = ?1")
            .bind(&n3)
            .execute(&pool)
            .await
            .unwrap();

        // Mark the oldest as read: it must drop out of the unread list.
        NotificationsRepo::mark_read(&pool, std::slice::from_ref(&n1))
            .await
            .unwrap();

        let (items, _next_cursor, total) = NotificationsRepo::list_unread(&pool, 10, None)
            .await
            .unwrap();
        assert_eq!(total, 2, "total counts unread rows, not returned ones");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, n3, "newest unread comes first");
        assert_eq!(items[1].id, n2);
        assert!(items.iter().all(|n| n.read_at.is_none()));

        let (limited, _next_cursor, total) = NotificationsRepo::list_unread(&pool, 1, None)
            .await
            .unwrap();
        assert_eq!(limited.len(), 1, "limit must be respected");
        assert_eq!(limited[0].id, n3);
        assert_eq!(total, 2, "total is independent of the page size");
    }

    /// Scenario: `mark_read` marca las no leídas, es idempotente, ignora los ids
    /// inexistentes y no falla con una lista vacía.
    #[tokio::test]
    async fn notifications_repo_mark_read_is_idempotent_and_ignores_missing() {
        let pool = setup().await;

        let a = NotificationsRepo::insert(&pool, "o", "a", "a", "info", None)
            .await
            .unwrap();
        let b = NotificationsRepo::insert(&pool, "o", "a", "b", "info", None)
            .await
            .unwrap();

        let marked = NotificationsRepo::mark_read(&pool, &[a.clone(), b.clone()])
            .await
            .unwrap();
        assert_eq!(marked, 2, "both unread rows must be marked");

        let row_a = NotificationsRepo::get_by_id(&pool, &a)
            .await
            .unwrap()
            .unwrap();
        let row_b = NotificationsRepo::get_by_id(&pool, &b)
            .await
            .unwrap()
            .unwrap();
        let read_at_a = row_a.read_at.clone().expect("a must be read now");
        let read_at_b = row_b.read_at.clone().expect("b must be read now");

        // Repeating is idempotent: nothing changes, nothing is marked.
        let again = NotificationsRepo::mark_read(&pool, &[a.clone(), b.clone()])
            .await
            .unwrap();
        assert_eq!(again, 0, "repeating the mark must be a no-op");

        let row_a = NotificationsRepo::get_by_id(&pool, &a)
            .await
            .unwrap()
            .unwrap();
        let row_b = NotificationsRepo::get_by_id(&pool, &b)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row_a.read_at.as_deref(), Some(read_at_a.as_str()));
        assert_eq!(row_b.read_at.as_deref(), Some(read_at_b.as_str()));

        // A missing id is ignored without error.
        let missing = NotificationsRepo::mark_read(&pool, &["no-such-id".to_string()])
            .await
            .unwrap();
        assert_eq!(missing, 0);

        // An empty list is a no-op, not an error.
        let empty = NotificationsRepo::mark_read(&pool, &[]).await.unwrap();
        assert_eq!(empty, 0);
    }

    /// Scenario: `mark_all_read` marca todas las no leídas y respeta las que ya
    /// estaban leídas (cuyo `read_at` no debe cambiar).
    #[tokio::test]
    async fn notifications_repo_mark_all_read_marks_every_unread_row() {
        let pool = setup().await;

        // Una fila que ya estaba leída: `mark_all_read` no debe tocarla.
        let already = NotificationsRepo::insert(&pool, "o", "a", "ya leída", "info", None)
            .await
            .unwrap();
        NotificationsRepo::mark_read(&pool, std::slice::from_ref(&already))
            .await
            .unwrap();
        let already_read_at = NotificationsRepo::get_by_id(&pool, &already)
            .await
            .unwrap()
            .unwrap()
            .read_at
            .expect("the pre-marked row must be read");

        // Tres filas no leídas: son las que `mark_all_read` debe marcar.
        let u1 = NotificationsRepo::insert(&pool, "o", "a", "no leída 1", "info", None)
            .await
            .unwrap();
        let u2 = NotificationsRepo::insert(&pool, "o", "a", "no leída 2", "info", None)
            .await
            .unwrap();
        let u3 = NotificationsRepo::insert(&pool, "o", "a", "no leída 3", "info", None)
            .await
            .unwrap();

        let marked = NotificationsRepo::mark_all_read(&pool).await.unwrap();
        assert_eq!(marked, 3, "the three unread rows must be marked");

        for id in [&u1, &u2, &u3] {
            let row = NotificationsRepo::get_by_id(&pool, id)
                .await
                .unwrap()
                .unwrap();
            assert!(row.read_at.is_some(), "every unread row must now be read");
        }

        let after = NotificationsRepo::get_by_id(&pool, &already)
            .await
            .unwrap()
            .unwrap()
            .read_at
            .expect("the pre-marked row keeps its read_at");
        assert_eq!(after, already_read_at, "an already-read row must not change");
    }

    /// Scenario: repetir `mark_all_read` es idempotente (devuelve 0).
    #[tokio::test]
    async fn notifications_repo_mark_all_read_is_idempotent() {
        let pool = setup().await;

        for i in 0..3 {
            NotificationsRepo::insert(&pool, "o", "a", &format!("n{i}"), "info", None)
                .await
                .unwrap();
        }

        let first = NotificationsRepo::mark_all_read(&pool).await.unwrap();
        assert_eq!(first, 3, "the first call marks every unread row");

        let second = NotificationsRepo::mark_all_read(&pool).await.unwrap();
        assert_eq!(second, 0, "repeating mark_all_read must be a no-op");
    }

    /// Scenario: el cursor keyset no se salta filas que comparten el mismo
    /// `created_at` (granularidad de segundo): la unión de las dos páginas son
    /// tres ids distintos, sin repetidos.
    #[tokio::test]
    async fn notifications_repo_unread_cursor_does_not_skip_same_second_rows() {
        let pool = setup().await;

        let mut inserted = Vec::new();
        for i in 0..3 {
            let id = NotificationsRepo::insert(&pool, "o", "a", &format!("n{i}"), "info", None)
                .await
                .unwrap();
            // Pin the very same created_at for all three: same-second ties.
            sqlx::query(
                "UPDATE notifications SET created_at = '2020-01-01 00:00:00' WHERE id = ?1",
            )
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
            inserted.push(id);
        }

        let (page1, cursor, total) = NotificationsRepo::list_unread(&pool, 2, None)
            .await
            .unwrap();
        assert_eq!(page1.len(), 2, "first page returns up to the limit");
        assert_eq!(total, 3);
        let cursor = cursor.expect("a full page with more rows must yield a cursor");

        let (page2, _cursor2, _total) = NotificationsRepo::list_unread(&pool, 2, Some(cursor.as_str()))
            .await
            .unwrap();
        assert_eq!(page2.len(), 1, "second page returns the remaining row");

        let mut seen: Vec<String> = page1
            .iter()
            .chain(page2.iter())
            .map(|n| n.id.clone())
            .collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3, "the union of both pages must be three distinct ids");
        assert!(
            inserted.iter().all(|id| seen.contains(id)),
            "every inserted row must appear exactly once across the pages"
        );
    }

    /// Scenario: el `total` cuenta todas las no leídas, sea cual sea la página.
    /// (Invariante heredado: ya se cumple en el andamiaje RED.)
    #[tokio::test]
    async fn notifications_repo_unread_total_is_independent_of_page() {
        let pool = setup().await;

        for i in 0..25 {
            NotificationsRepo::insert(&pool, "o", "a", &format!("n{i}"), "info", None)
                .await
                .unwrap();
        }

        let (items, _cursor, total) = NotificationsRepo::list_unread(&pool, 10, None)
            .await
            .unwrap();
        assert_eq!(items.len(), 10, "the page size must be respected");
        assert_eq!(total, 25, "total counts every unread row, not the page");
    }

    /// Scenario: en la última página no hay cursor de continuación.
    /// (Invariante heredado: ya se cumple en el andamiaje RED.)
    #[tokio::test]
    async fn notifications_repo_unread_next_cursor_is_none_on_last_page() {
        let pool = setup().await;

        for i in 0..3 {
            NotificationsRepo::insert(&pool, "o", "a", &format!("n{i}"), "info", None)
                .await
                .unwrap();
        }

        let (items, cursor, total) = NotificationsRepo::list_unread(&pool, 10, None)
            .await
            .unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(total, 3);
        assert!(cursor.is_none(), "no more rows means no next cursor");
    }
}
