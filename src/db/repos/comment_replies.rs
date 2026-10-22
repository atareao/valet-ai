//! Persistencia del registro de idempotencia de las respuestas a comentarios del
//! blog (`comment_replies`).
//!
//! La reserva es la pieza clave: se toma con un único
//! `INSERT ... ON CONFLICT(comment_id) DO NOTHING`, de modo que dos llamadas
//! concurrentes para el mismo comentario no puedan ganar las dos. La fila pasa
//! por `reserved` → `published` / `uncertain`, o se **libera** con un `DELETE`
//! cuando el desenlace garantiza que no se creó nada (`400`, `401`, `403`, `429`).
//! Ante un desenlace ambiguo (timeout, fallo de red o `5xx`) la reserva **se
//! conserva** en `uncertain`: publicar dos veces es el fallo más caro.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

/// El estado de la publicación de una respuesta, cerrado por el `CHECK` de la
/// tabla (`state IN ('reserved', 'published', 'uncertain')`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentReplyState {
    /// Reserva tomada; la petición aún no tiene desenlace (o el proceso murió
    /// entre la reserva y el desenlace).
    Reserved,
    /// WordPress creó la respuesta.
    Published,
    /// Desenlace ambiguo (timeout, fallo de red o `5xx`): no se sabe si se creó,
    /// así que la reserva se conserva y no se publica de nuevo.
    Uncertain,
}

impl CommentReplyState {
    /// La forma que guarda la columna `state` y que impone su `CHECK`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Published => "published",
            Self::Uncertain => "uncertain",
        }
    }

    /// Parsea el valor de la columna. Un valor fuera del conjunto cerrado —que el
    /// `CHECK` de la tabla impide— es un error, no un estado silencioso.
    pub fn parse(raw: &str) -> Result<Self, sqlx::Error> {
        match raw {
            "reserved" => Ok(Self::Reserved),
            "published" => Ok(Self::Published),
            "uncertain" => Ok(Self::Uncertain),
            other => Err(sqlx::Error::Decode(
                format!("estado de comment_replies desconocido: '{other}'").into(),
            )),
        }
    }
}

/// Una fila de `comment_replies`: el registro de una respuesta publicada (o en
/// curso) a un comentario del blog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentReply {
    /// El id del comentario del blog al que se responde, en su forma decimal.
    pub comment_id: String,
    /// El id de la entrada del blog.
    pub post_id: String,
    /// El id de la notificación de la que salieron los identificadores, si se
    /// conoce.
    pub notification_id: Option<String>,
    /// El estado de la publicación.
    pub state: CommentReplyState,
    /// El id que devolvió WordPress al crear la respuesta, si lo hubo (un `201`
    /// con cuerpo ilegible deja `None`).
    pub wp_comment_id: Option<String>,
    /// El código HTTP del último intento, si lo hubo.
    pub last_status: Option<i64>,
    /// El instante en que se tomó la reserva.
    pub created_at: String,
    /// El instante de la última transición.
    pub updated_at: String,
}

/// El desenlace de un intento de reserva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reservation {
    /// La reserva quedó tomada: es el turno de salir a la red.
    Acquired,
    /// Ya había una fila en cualquiera de los tres estados: **no** se sale a la
    /// red.
    AlreadyReserved,
}

/// Persistencia de `comment_replies`: la reserva atómica y sus transiciones.
pub struct CommentRepliesRepo;

impl CommentRepliesRepo {
    /// Intenta reservar `comment_id`.
    ///
    /// Atómico: el `INSERT ... ON CONFLICT(comment_id) DO NOTHING` solo
    /// materializa la fila si no existía, y `rows_affected()` lo delata. Si
    /// devuelve [`Reservation::AlreadyReserved`] —porque ya hay una fila en
    /// `reserved`, `published` o `uncertain`— el llamador **no** debe salir a la
    /// red. La fila nace en estado `reserved`.
    pub async fn reserve(
        pool: &SqlitePool,
        comment_id: &str,
        post_id: &str,
        notification_id: Option<&str>,
    ) -> Result<Reservation, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO comment_replies (comment_id, post_id, notification_id, state) \
             VALUES (?1, ?2, ?3, 'reserved') \
             ON CONFLICT(comment_id) DO NOTHING",
        )
        .bind(comment_id)
        .bind(post_id)
        .bind(notification_id)
        .execute(pool)
        .await?;

        Ok(if result.rows_affected() == 1 {
            Reservation::Acquired
        } else {
            Reservation::AlreadyReserved
        })
    }

    /// Marca la reserva como `published`. `wp_comment_id` puede ser `None` (un
    /// `201` con cuerpo ilegible) y `last_status` es el código HTTP del intento.
    /// Devuelve `true` si había una fila que actualizar.
    pub async fn mark_published(
        pool: &SqlitePool,
        comment_id: &str,
        wp_comment_id: Option<&str>,
        last_status: i64,
    ) -> Result<bool, sqlx::Error> {
        Self::set_state(
            pool,
            comment_id,
            "published",
            wp_comment_id,
            Some(last_status),
        )
        .await
    }

    /// Marca la reserva como `uncertain` sin soltarla. La fila **se conserva**:
    /// es lo que impide publicar dos veces tras un desenlace ambiguo.
    pub async fn mark_uncertain(
        pool: &SqlitePool,
        comment_id: &str,
        last_status: Option<i64>,
    ) -> Result<bool, sqlx::Error> {
        Self::set_state(pool, comment_id, "uncertain", None, last_status).await
    }

    /// Libera la reserva (borra la fila). Solo se llama ante un desenlace que
    /// garantiza que **no** se creó nada (`400`, `401`, `403`, `429`). Devuelve
    /// `true` si borró la fila.
    pub async fn release(pool: &SqlitePool, comment_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM comment_replies WHERE comment_id = ?1")
            .bind(comment_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// La fila registrada para ese comentario, o `None` si no hay registro.
    pub async fn find(
        pool: &SqlitePool,
        comment_id: &str,
    ) -> Result<Option<CommentReply>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT comment_id, post_id, notification_id, state, wp_comment_id, \
             last_status, created_at, updated_at \
             FROM comment_replies WHERE comment_id = ?1",
        )
        .bind(comment_id)
        .fetch_optional(pool)
        .await?;

        row.as_ref().map(reply_from_row).transpose()
    }

    /// El cuerpo común de `mark_published` y `mark_uncertain`: fija `state`, el
    /// `wp_comment_id` (o `NULL`) y el `last_status`, y sella `updated_at`.
    async fn set_state(
        pool: &SqlitePool,
        comment_id: &str,
        state: &str,
        wp_comment_id: Option<&str>,
        last_status: Option<i64>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE comment_replies \
             SET state = ?2, wp_comment_id = ?3, last_status = ?4, updated_at = datetime('now') \
             WHERE comment_id = ?1",
        )
        .bind(comment_id)
        .bind(state)
        .bind(wp_comment_id)
        .bind(last_status)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Mapea una fila de `comment_replies` al modelo.
fn reply_from_row(row: &SqliteRow) -> Result<CommentReply, sqlx::Error> {
    Ok(CommentReply {
        comment_id: row.try_get("comment_id")?,
        post_id: row.try_get("post_id")?,
        notification_id: row.try_get("notification_id")?,
        state: CommentReplyState::parse(&row.try_get::<String, _>("state")?)?,
        wp_comment_id: row.try_get("wp_comment_id")?,
        last_status: row.try_get("last_status")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
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

    /// Scenario: la reserva se materializa con un solo INSERT atómico y la fila
    /// nace en estado `reserved` con sus identificadores.
    #[tokio::test]
    async fn comment_replies_repo_reserve_acquires_when_absent() {
        let pool = setup().await;

        let outcome = CommentRepliesRepo::reserve(&pool, "901", "42", Some("notif-1"))
            .await
            .unwrap();
        assert_eq!(outcome, Reservation::Acquired);

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .expect("la reserva debe materializar una fila");
        assert_eq!(row.comment_id, "901");
        assert_eq!(row.post_id, "42");
        assert_eq!(row.notification_id.as_deref(), Some("notif-1"));
        assert_eq!(row.state, CommentReplyState::Reserved);
        assert_eq!(row.wp_comment_id, None);
        assert_eq!(row.last_status, None);
    }

    /// Scenario: reservar dos veces el mismo comentario no crea una segunda fila
    /// y la segunda reserva **no** se concede (no se sale a la red).
    #[tokio::test]
    async fn comment_replies_repo_reserve_is_atomic_for_duplicates() {
        let pool = setup().await;

        let first = CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();
        let second = CommentRepliesRepo::reserve(&pool, "901", "99", None)
            .await
            .unwrap();
        assert_eq!(first, Reservation::Acquired);
        assert_eq!(second, Reservation::AlreadyReserved);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comment_replies")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "solo debe quedar una fila para '901'");

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.post_id, "42",
            "la fila existente no se sobrescribe (DO NOTHING)"
        );
    }

    /// Scenario: `mark_published` transiciona `reserved` → `published` con el
    /// `wp_comment_id` y el código HTTP del intento.
    #[tokio::test]
    async fn comment_replies_repo_marks_published() {
        let pool = setup().await;
        CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();

        let updated = CommentRepliesRepo::mark_published(&pool, "901", Some("7701"), 201)
            .await
            .unwrap();
        assert!(updated, "debe haber una fila que actualizar");

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Published);
        assert_eq!(row.wp_comment_id.as_deref(), Some("7701"));
        assert_eq!(row.last_status, Some(201));
    }

    /// Scenario: un `201` con cuerpo ilegible deja la fila en `published` con el
    /// `wp_comment_id` ausente.
    #[tokio::test]
    async fn comment_replies_repo_marks_published_without_wp_id() {
        let pool = setup().await;
        CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();

        CommentRepliesRepo::mark_published(&pool, "901", None, 201)
            .await
            .unwrap();

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Published);
        assert_eq!(row.wp_comment_id, None);
    }

    /// Scenario: `mark_uncertain` **conserva** la fila y la deja `uncertain`.
    #[tokio::test]
    async fn comment_replies_repo_marks_uncertain_keeping_the_row() {
        let pool = setup().await;
        CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();

        let updated = CommentRepliesRepo::mark_uncertain(&pool, "901", Some(503))
            .await
            .unwrap();
        assert!(updated);

        let row = CommentRepliesRepo::find(&pool, "901")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.state, CommentReplyState::Uncertain);
        assert_eq!(row.last_status, Some(503));

        // Y una segunda reserva sigue chocando: la fila no se soltó.
        let again = CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();
        assert_eq!(again, Reservation::AlreadyReserved);
    }

    /// Scenario: `release` borra la reserva y una segunda puede volver a tomarla.
    #[tokio::test]
    async fn comment_replies_repo_release_frees_the_reservation() {
        let pool = setup().await;
        CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();

        let released = CommentRepliesRepo::release(&pool, "901").await.unwrap();
        assert!(released, "debe borrar la fila existente");

        // Un segundo `release` no encuentra nada.
        assert!(!CommentRepliesRepo::release(&pool, "901").await.unwrap());

        let again = CommentRepliesRepo::reserve(&pool, "901", "42", None)
            .await
            .unwrap();
        assert_eq!(again, Reservation::Acquired);
    }

    /// Scenario: `find` de una fila ausente devuelve `None`.
    #[tokio::test]
    async fn comment_replies_repo_find_returns_none_when_absent() {
        let pool = setup().await;

        assert_eq!(
            CommentRepliesRepo::find(&pool, "no-existe").await.unwrap(),
            None
        );
    }

    /// Scenario: `mark_published` sobre un comentario sin reserva no actualiza
    /// ninguna fila (`false`), en vez de inventarse una.
    #[tokio::test]
    async fn comment_replies_repo_mark_on_absent_row_is_a_noop() {
        let pool = setup().await;

        let updated = CommentRepliesRepo::mark_published(&pool, "no-existe", Some("1"), 201)
            .await
            .unwrap();
        assert!(!updated, "no hay fila que actualizar");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comment_replies")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "no debe crearse una fila a partir de un UPDATE");
    }

    /// Scenario: el parser del estado acepta los tres valores y rechaza el resto.
    #[test]
    fn comment_reply_state_parse_is_closed() {
        assert_eq!(
            CommentReplyState::parse("reserved").unwrap(),
            CommentReplyState::Reserved
        );
        assert_eq!(
            CommentReplyState::parse("published").unwrap(),
            CommentReplyState::Published
        );
        assert_eq!(
            CommentReplyState::parse("uncertain").unwrap(),
            CommentReplyState::Uncertain
        );
        assert!(CommentReplyState::parse("otro").is_err());
    }
}
