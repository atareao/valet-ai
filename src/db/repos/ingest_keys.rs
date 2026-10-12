//! Persistencia de las claves de ingesta (`ingest_keys`).
//!
//! Una clave de ingesta autoriza `POST /api/notifications` sin sesión OIDC. El
//! token se muestra **una sola vez**, al crearla; aquí solo se guardan su
//! **SHA-256 en hexadecimal** (`key_hash`) y sus **12 primeros caracteres**
//! (`prefix`). El token en claro no se persiste, no se devuelve y no aparece en
//! ningún `Debug`: un volcado de la tabla nunca revela una credencial usable.
//!
//! El generador del token (`val_` + 32 bytes aleatorios en base64url sin
//! padding) vive en este módulo, en [`generate_token`], para que la ruta de alta
//! lo use sin volver a implementarlo.

use base64::Engine as _;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

/// Una clave de ingesta tal como vive en la tabla.
///
/// No contiene el token: solo su SHA-256 (`key_hash`) y sus 12 primeros
/// caracteres (`prefix`). La ruta de listado debe mapear este struct a un DTO
/// que omita `key_hash`; el token nunca está disponible aquí.
#[derive(Clone, PartialEq, Eq)]
pub struct IngestKey {
    pub id: String,
    pub name: String,
    /// SHA-256 del token en hexadecimal.
    pub key_hash: String,
    /// Los 12 primeros caracteres del token, para identificarlo sin guardarlo.
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl std::fmt::Debug for IngestKey {
    /// Escrito a mano para dejar explícito que un `Debug` **nunca** incluye un
    /// token: el struct solo tiene el hash y el prefijo, y esta representación
    /// se mantiene deliberadamente igual aunque el struct gane campos.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestKey")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("key_hash", &self.key_hash)
            .field("prefix", &self.prefix)
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .field("revoked_at", &self.revoked_at)
            .finish()
    }
}

/// Cuántos caracteres del token se guardan como prefijo identificativo.
const PREFIX_LEN: usize = 12;

/// Prefijo de todo token de ingesta.
const TOKEN_PREFIX: &str = "val_";

/// Codifica `bytes` en hexadecimal minúsculo.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Escribir en un `String` no puede fallar.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// SHA-256 del `token` en hexadecimal. Es lo único que se persiste del token.
pub fn hash_token(token: &str) -> String {
    hex_encode(&Sha256::digest(token.as_bytes()))
}

/// Los 12 primeros caracteres del `token`.
fn prefix_token(token: &str) -> String {
    token.chars().take(PREFIX_LEN).collect()
}

/// Genera un token de ingesta nuevo: `val_` seguido de 32 bytes aleatorios de un
/// CSPRNG en base64url sin padding.
///
/// La ruta de alta lo usa así: genera el token con esta función, lo devuelve al
/// cliente **una única vez** y persiste la clave con [`IngestRepo::create`],
/// que solo guarda su hash y su prefijo. El token en claro no se persiste.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    format!("{TOKEN_PREFIX}{}", engine.encode(bytes))
}

pub struct IngestRepo;

impl IngestRepo {
    /// Da de alta una clave con un `token` ya generado y devuelve la clave
    /// creada.
    ///
    /// Solo persiste el SHA-256 del `token` (`key_hash`) y sus 12 primeros
    /// caracteres (`prefix`): el token en claro **nunca** se escribe en la base
    /// de datos. El `id` es un UUIDv4 y `created_at` lo pone la base de datos.
    pub async fn create(
        pool: &SqlitePool,
        name: &str,
        token: &str,
    ) -> Result<IngestKey, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let key_hash = hash_token(token);
        let prefix = prefix_token(token);

        sqlx::query(
            "INSERT INTO ingest_keys (id, name, key_hash, prefix)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(name)
        .bind(&key_hash)
        .bind(&prefix)
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Busca una clave por el **SHA-256 del token presentado**.
    ///
    /// Calcula el hash del `token`, lo busca por la columna UNIQUE `key_hash` y,
    /// solo si la clave no está revocada, actualiza `last_used_at` y la
    /// devuelve. Un token desconocido o de una clave revocada devuelve `None`
    /// sin tocar `last_used_at`.
    pub async fn find_by_hash(
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<IngestKey>, sqlx::Error> {
        let key_hash = hash_token(token);
        let row = sqlx::query(
            "SELECT id, name, key_hash, prefix, created_at, last_used_at, revoked_at
             FROM ingest_keys WHERE key_hash = ?1",
        )
        .bind(&key_hash)
        .fetch_optional(pool)
        .await?;

        let Some(key) = row.map(|r| row_to_key(&r)) else {
            return Ok(None);
        };

        // A revoked key never authenticates, and its last_used_at stays put.
        if key.revoked_at.is_some() {
            return Ok(None);
        }

        sqlx::query("UPDATE ingest_keys SET last_used_at = datetime('now') WHERE id = ?1")
            .bind(&key.id)
            .execute(pool)
            .await?;

        Self::get_by_id(pool, &key.id).await
    }

    /// Devuelve una clave por su `id`, o `None` si no existe.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<IngestKey>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, key_hash, prefix, created_at, last_used_at, revoked_at
             FROM ingest_keys WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| row_to_key(&r)))
    }

    /// Lista todas las claves, activas y revocadas, más recientes primero.
    ///
    /// No devuelve ningún token: el struct solo tiene el hash y el prefijo.
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<IngestKey>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, name, key_hash, prefix, created_at, last_used_at, revoked_at
             FROM ingest_keys
             ORDER BY created_at DESC, rowid DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.iter().map(row_to_key).collect())
    }

    /// Revoca una clave fijando `revoked_at`, **sin** borrar la fila.
    ///
    /// Idempotente: el `WHERE revoked_at IS NULL` hace que repetir la llamada
    /// no altere el `revoked_at` ya fijado. Un `id` inexistente es un no-op, no
    /// un error.
    pub async fn revoke(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE ingest_keys SET revoked_at = datetime('now')
             WHERE id = ?1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

/// Mapea una fila de `ingest_keys` al struct de dominio.
fn row_to_key(row: &sqlx::sqlite::SqliteRow) -> IngestKey {
    IngestKey {
        id: row.get("id"),
        name: row.get("name"),
        key_hash: row.get("key_hash"),
        prefix: row.get("prefix"),
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
        revoked_at: row.get("revoked_at"),
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

    /// SHA-256 en hexadecimal calculado de forma independiente, para no
    /// confiar en la implementación bajo prueba.
    fn expected_hash(token: &str) -> String {
        use sha2::{Digest, Sha256};
        use std::fmt::Write as _;

        let digest = Sha256::digest(token.as_bytes());
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(hex, "{byte:02x}").unwrap();
        }
        hex
    }

    /// Scenario: una clave nueva persiste hash y prefijo, nunca el token.
    #[tokio::test]
    async fn ingest_keys_repo_create_persists_hash_and_prefix_never_token() {
        let pool = setup().await;
        let token = generate_token();

        let created = IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        assert!(!created.id.is_empty(), "the id must not be empty");
        assert_eq!(created.name, "monitor");
        assert_eq!(
            created.key_hash,
            expected_hash(&token),
            "key_hash must be the SHA-256 hex of the token"
        );
        assert_eq!(
            created.prefix,
            token.chars().take(12).collect::<String>(),
            "prefix must be the first 12 characters of the token"
        );
        assert!(created.last_used_at.is_none(), "a fresh key is unused");
        assert!(created.revoked_at.is_none(), "a fresh key is not revoked");

        // No column may contain the whole token.
        let stored = IngestRepo::list_all(&pool).await.unwrap();
        assert_eq!(stored.len(), 1);
        let row = &stored[0];
        for column in [
            &row.id,
            &row.name,
            &row.key_hash,
            &row.prefix,
            &row.created_at,
        ] {
            assert!(
                !column.contains(&token),
                "a column leaked the token: {column}"
            );
        }
    }

    /// Scenario: un token válido actualiza `last_used_at` y encuentra la clave.
    #[tokio::test]
    async fn ingest_keys_repo_find_by_hash_updates_last_used_at() {
        let pool = setup().await;
        let token = generate_token();
        let created = IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        let found = IngestRepo::find_by_hash(&pool, &token)
            .await
            .unwrap()
            .expect("a valid token must resolve to its key");

        assert_eq!(found.id, created.id);
        assert!(
            found.last_used_at.is_some(),
            "a successful lookup must set last_used_at"
        );
    }

    /// Scenario: un token desconocido no resuelve a ninguna clave.
    #[tokio::test]
    async fn ingest_keys_repo_find_by_hash_unknown_returns_none() {
        let pool = setup().await;
        let token = generate_token();
        IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        let other = generate_token();
        assert!(IngestRepo::find_by_hash(&pool, &other)
            .await
            .unwrap()
            .is_none());
    }

    /// Scenario: una clave revocada no resuelve y no toca `last_used_at`.
    #[tokio::test]
    async fn ingest_keys_repo_revoked_key_not_found_and_last_used_untouched() {
        let pool = setup().await;
        let token = generate_token();
        let created = IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        IngestRepo::revoke(&pool, &created.id).await.unwrap();

        assert!(
            IngestRepo::find_by_hash(&pool, &token)
                .await
                .unwrap()
                .is_none(),
            "a revoked key must not authenticate"
        );

        let stored = IngestRepo::list_all(&pool).await.unwrap();
        assert!(
            stored[0].last_used_at.is_none(),
            "a rejected lookup must not touch last_used_at"
        );
    }

    /// Scenario: el listado devuelve activas y revocadas, sin el token.
    #[tokio::test]
    async fn ingest_keys_repo_list_all_returns_active_and_revoked_without_token() {
        let pool = setup().await;
        let active_token = generate_token();
        let revoked_token = generate_token();
        let active = IngestRepo::create(&pool, "activa", &active_token)
            .await
            .unwrap();
        let revoked = IngestRepo::create(&pool, "revocada", &revoked_token)
            .await
            .unwrap();
        IngestRepo::revoke(&pool, &revoked.id).await.unwrap();

        let all = IngestRepo::list_all(&pool).await.unwrap();
        assert_eq!(all.len(), 2, "both active and revoked keys are listed");

        for key in &all {
            assert!(!key.id.is_empty());
            assert!(!key.name.is_empty());
            assert!(!key.prefix.is_empty());
            assert!(!key.created_at.is_empty());
            for value in [
                &key.id,
                &key.name,
                &key.key_hash,
                &key.prefix,
                &key.created_at,
            ] {
                assert!(!value.contains(&active_token), "token leaked");
                assert!(!value.contains(&revoked_token), "token leaked");
            }
        }

        let listed = all.iter().find(|k| k.id == active.id).unwrap();
        assert!(listed.revoked_at.is_none(), "the active key is not revoked");
        let listed = all.iter().find(|k| k.id == revoked.id).unwrap();
        assert!(
            listed.revoked_at.is_some(),
            "the revoked key keeps a revoked_at"
        );
    }

    /// Scenario: revocar conserva la fila, es idempotente y un id inexistente
    /// no es un error.
    #[tokio::test]
    async fn ingest_keys_repo_revoke_is_idempotent_and_missing_is_not_error() {
        let pool = setup().await;
        let token = generate_token();
        let created = IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        IngestRepo::revoke(&pool, &created.id).await.unwrap();

        let after = &IngestRepo::list_all(&pool).await.unwrap()[0];
        let first_revoked_at = after
            .revoked_at
            .clone()
            .expect("the row keeps existing with revoked_at set");

        // Repeating must not change the already-set revoked_at.
        IngestRepo::revoke(&pool, &created.id).await.unwrap();
        let after = &IngestRepo::list_all(&pool).await.unwrap()[0];
        assert_eq!(
            after.revoked_at.as_deref(),
            Some(first_revoked_at.as_str()),
            "repeating the revoke must not change revoked_at"
        );

        // Revoking a missing id is a no-op, not an error.
        IngestRepo::revoke(&pool, "no-such-id").await.unwrap();
    }

    /// Scenario: la salida de depuración no filtra el token.
    #[tokio::test]
    async fn ingest_keys_repo_debug_does_not_leak_token() {
        let pool = setup().await;
        let token = generate_token();
        let created = IngestRepo::create(&pool, "monitor", &token).await.unwrap();

        let shown = format!("{created:?}");
        assert!(
            !shown.contains(&token),
            "the Debug output leaked the token: {shown}"
        );
    }
}
