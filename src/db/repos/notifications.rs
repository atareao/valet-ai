//! Persistencia de la bandeja de notificaciones (`notifications`).
//!
//! El `INSERT` precede a la respuesta de la ingesta; el estado de lectura se
//! gestiona con un `UPDATE` idempotente sobre las notificaciones no leídas.

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

        let (items, total) = NotificationsRepo::list_unread(&pool, 10, 0).await.unwrap();
        assert_eq!(total, 2, "total counts unread rows, not returned ones");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, n3, "newest unread comes first");
        assert_eq!(items[1].id, n2);
        assert!(items.iter().all(|n| n.read_at.is_none()));

        let (limited, total) = NotificationsRepo::list_unread(&pool, 1, 0).await.unwrap();
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
}
