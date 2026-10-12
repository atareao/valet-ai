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

        let created = IngestRepo::create(&pool, "monitor", &token)
            .await
            .unwrap();

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
        let created = IngestRepo::create(&pool, "monitor", &token)
            .await
            .unwrap();

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
        let created = IngestRepo::create(&pool, "monitor", &token)
            .await
            .unwrap();

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
        let created = IngestRepo::create(&pool, "monitor", &token)
            .await
            .unwrap();

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
        let created = IngestRepo::create(&pool, "monitor", &token)
            .await
            .unwrap();

        let shown = format!("{created:?}");
        assert!(
            !shown.contains(&token),
            "the Debug output leaked the token: {shown}"
        );
    }
}
