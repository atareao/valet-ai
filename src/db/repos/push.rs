//! Persistencia de las suscripciones WebPush del navegador (`push_subscriptions`).
//!
//! El *upsert* va por `endpoint` y conserva el `id` original de la fila; el
//! borrado por `endpoint` es idempotente. La clave privada VAPID no vive aquí.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

/// Una suscripción WebPush registrada por el navegador.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushSubscription {
    pub id: String,
    pub user_id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PushRepo;

impl PushRepo {
    /// Registra o actualiza una suscripción por `endpoint` y devuelve su `id`.
    ///
    /// El *upsert* conserva el `id` original de la fila y refresca `p256dh`,
    /// `auth`, `user_agent` y `updated_at`. Un `endpoint` nuevo recibe un
    /// UUIDv4.
    pub async fn upsert(
        pool: &SqlitePool,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        user_agent: Option<&str>,
    ) -> Result<String, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth, user_agent)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(endpoint) DO UPDATE SET
                 p256dh = excluded.p256dh,
                 auth = excluded.auth,
                 user_agent = excluded.user_agent,
                 updated_at = datetime('now')",
        )
        .bind(&id)
        .bind(endpoint)
        .bind(p256dh)
        .bind(auth)
        .bind(user_agent)
        .execute(pool)
        .await?;

        // On conflict the generated `id` is discarded and the original wins, so
        // read it back.
        sqlx::query_scalar("SELECT id FROM push_subscriptions WHERE endpoint = ?1")
            .bind(endpoint)
            .fetch_one(pool)
            .await
    }

    /// Elimina la suscripción de `endpoint`. Idempotente: no falla si no existe.
    pub async fn delete_by_endpoint(pool: &SqlitePool, endpoint: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?1")
            .bind(endpoint)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Devuelve todas las suscripciones registradas.
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<PushSubscription>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, user_id, endpoint, p256dh, auth, user_agent, created_at, updated_at
             FROM push_subscriptions",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| PushSubscription {
                id: r.get("id"),
                user_id: r.get("user_id"),
                endpoint: r.get("endpoint"),
                p256dh: r.get("p256dh"),
                auth: r.get("auth"),
                user_agent: r.get("user_agent"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
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

    /// Scenario: un *upsert* nuevo inserta una fila y devuelve su `id`.
    #[tokio::test]
    async fn push_repo_upsert_inserts_and_returns_id() {
        let pool = setup().await;

        let id = PushRepo::upsert(
            &pool,
            "https://example.invalid/push/1",
            "p256dh-1",
            "auth-1",
            Some("ua/1"),
        )
        .await
        .unwrap();

        assert!(!id.is_empty(), "the returned id must not be empty");
        let all = PushRepo::list_all(&pool).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].endpoint, "https://example.invalid/push/1");
        assert_eq!(all[0].p256dh, "p256dh-1");
        assert_eq!(all[0].auth, "auth-1");
        assert_eq!(all[0].user_agent.as_deref(), Some("ua/1"));
        assert_eq!(all[0].user_id, "default");
    }

    /// Scenario: volver a hacer *upsert* del mismo `endpoint` conserva el `id` y
    /// actualiza `p256dh`, `auth`, `user_agent` y `updated_at`.
    #[tokio::test]
    async fn push_repo_upsert_preserves_id_and_updates_fields() {
        let pool = setup().await;

        let first = PushRepo::upsert(
            &pool,
            "https://example.invalid/push/1",
            "p256dh-old",
            "auth-old",
            Some("ua/old"),
        )
        .await
        .unwrap();

        // Force a different updated_at so we can observe the change.
        sqlx::query(
            "UPDATE push_subscriptions SET updated_at = '2000-01-01 00:00:00' WHERE id = ?1",
        )
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();

        let second = PushRepo::upsert(
            &pool,
            "https://example.invalid/push/1",
            "p256dh-new",
            "auth-new",
            Some("ua/new"),
        )
        .await
        .unwrap();

        assert_eq!(first, second, "the upsert must preserve the original id");

        let all = PushRepo::list_all(&pool).await.unwrap();
        assert_eq!(all.len(), 1, "the same endpoint must yield a single row");
        assert_eq!(all[0].p256dh, "p256dh-new");
        assert_eq!(all[0].auth, "auth-new");
        assert_eq!(all[0].user_agent.as_deref(), Some("ua/new"));
        assert_ne!(
            all[0].updated_at, "2000-01-01 00:00:00",
            "updated_at must be bumped by the upsert"
        );
    }

    /// Scenario: el borrado por `endpoint` es idempotente.
    #[tokio::test]
    async fn push_repo_delete_by_endpoint_is_idempotent() {
        let pool = setup().await;

        PushRepo::upsert(&pool, "https://example.invalid/push/1", "p", "a", None)
            .await
            .unwrap();

        PushRepo::delete_by_endpoint(&pool, "https://example.invalid/push/1")
            .await
            .unwrap();
        assert!(PushRepo::list_all(&pool).await.unwrap().is_empty());

        // Deleting a missing endpoint is a no-op, not an error.
        PushRepo::delete_by_endpoint(&pool, "https://example.invalid/push/1")
            .await
            .unwrap();
        PushRepo::delete_by_endpoint(&pool, "https://example.invalid/push/unknown")
            .await
            .unwrap();
    }

    /// Scenario: el listado devuelve todas las suscripciones.
    #[tokio::test]
    async fn push_repo_list_all_returns_every_subscription() {
        let pool = setup().await;

        PushRepo::upsert(&pool, "https://example.invalid/push/1", "p1", "a1", None)
            .await
            .unwrap();
        PushRepo::upsert(&pool, "https://example.invalid/push/2", "p2", "a2", None)
            .await
            .unwrap();

        let all = PushRepo::list_all(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        let endpoints: Vec<_> = all.iter().map(|s| s.endpoint.clone()).collect();
        assert!(endpoints.contains(&"https://example.invalid/push/1".to_string()));
        assert!(endpoints.contains(&"https://example.invalid/push/2".to_string()));
    }
}
