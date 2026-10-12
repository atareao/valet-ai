//! Claves VAPID para las notificaciones WebPush.
//!
//! El par VAPID es único por instalación y se genera la primera vez que se
//! solicita, cuando `push_vapid_public_key` o `push_vapid_private_key` están
//! ausentes o vacías (la migración las siembra vacías). Ambas se persisten en
//! `settings` dentro de una misma transacción, de modo que dos peticiones
//! concurrentes converjan en el mismo par.
//!
//! La clave privada es material sensible: nunca aparece en un `Debug`, en un
//! log ni en el mensaje de un [`PushError`].

use base64::Engine as _;
use sqlx::SqlitePool;
use thiserror::Error;
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair};
use web_push_native::{p256::PublicKey, Auth, WebPushBuilder};

use crate::db::repos::settings::SettingsRepo;

/// Re-exportado para que el resto del proyecto construya claves y suscriptores
/// sin depender directamente de `web-push-native`.
pub use web_push_native::{jwt_simple, p256};

/// Clave de `settings` con la clave pública VAPID (base64url sin padding).
pub const PUBLIC_KEY_SETTING: &str = "push_vapid_public_key";

/// Clave de `settings` con la clave privada VAPID (base64url sin padding).
pub const PRIVATE_KEY_SETTING: &str = "push_vapid_private_key";

/// Un par de claves VAPID listo para firmar peticiones WebPush.
pub struct VapidKeys {
    /// Clave pública (65 bytes uncompressed) en base64url sin padding.
    pub public_key: String,
    /// Clave privada (32 bytes crudos) en base64url sin padding.
    pub private_key: String,
}

impl std::fmt::Debug for VapidKeys {
    /// Redacta la clave privada: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VapidKeys")
            .field("public_key", &self.public_key)
            .field("private_key", &"***")
            .finish()
    }
}

/// Errores al cargar, generar o construir peticiones WebPush.
#[derive(Debug, Error)]
pub enum PushError {
    /// Fallo de la base de datos al leer o escribir `settings`.
    #[error("no se pudo acceder a los ajustes de VAPID")]
    Database(#[from] sqlx::Error),
    /// El par persistido no es una clave VAPID válida.
    #[error("las claves VAPID almacenadas no son válidas")]
    InvalidStoredKey,
    /// El `endpoint` de la suscripción no es una URI válida.
    #[error("el endpoint de la suscripción no es válido")]
    InvalidEndpoint,
    /// La clave `p256dh` o el `auth` de la suscripción no son válidos.
    #[error("las claves de la suscripción no son válidas")]
    InvalidSubscription,
    /// El payload no se pudo cifrar ni firmar con VAPID.
    #[error("no se pudo construir la petición WebPush")]
    Build(#[source] web_push_native::Error),
}

/// Motor base64url sin padding compartido por toda la generación y lectura.
fn engine() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

/// Genera un par VAPID nuevo.
///
/// La privada se serializa como base64url sin padding de los 32 bytes crudos y
/// la pública como base64url sin padding de los 65 bytes *uncompressed* que
/// espera el navegador.
fn generate_vapid_pair() -> VapidKeys {
    let key_pair = ES256KeyPair::generate();
    let public_key = engine().encode(key_pair.public_key().public_key().to_bytes_uncompressed());
    let private_key = engine().encode(key_pair.to_bytes());
    VapidKeys {
        public_key,
        private_key,
    }
}

/// Comprueba que el par persistido es válido y consistente: la privada parsea
/// y la pública derivada coincide con la almacenada. Nunca un par mezclado.
fn validate_pair(keys: &VapidKeys) -> Result<(), PushError> {
    let raw = engine()
        .decode(&keys.private_key)
        .map_err(|_| PushError::InvalidStoredKey)?;
    let key_pair = ES256KeyPair::from_bytes(&raw).map_err(|_| PushError::InvalidStoredKey)?;
    let derived = engine().encode(key_pair.public_key().public_key().to_bytes_uncompressed());
    if derived != keys.public_key {
        return Err(PushError::InvalidStoredKey);
    }
    Ok(())
}

/// Devuelve el valor salvo que sea blanco, en cuyo caso cuenta como ausente.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

/// Lee una clave de `settings` dentro de la transacción.
async fn read_setting(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await
}

/// Escribe las dos claves, pero **solo** si faltan o están vacías.
///
/// El `ON CONFLICT ... DO UPDATE ... WHERE value = '' OR value IS NULL` es el
/// mismo idioma de la migración: rellena una siembra vacía sin pisar jamás un
/// valor personalizado. Es equivalente a un `DO NOTHING` para un valor ya
/// presente, que es lo que hace converger a dos llamadas concurrentes en el
/// par del primero que escribe.
async fn upsert_pair(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    keys: &VapidKeys,
) -> Result<(), sqlx::Error> {
    for (key, value) in [
        (PUBLIC_KEY_SETTING, keys.public_key.as_str()),
        (PRIVATE_KEY_SETTING, keys.private_key.as_str()),
    ] {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
             WHERE settings.value = '' OR settings.value IS NULL",
        )
        .bind(key)
        .bind(value)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Carga las claves VAPID de `settings`; si faltan o están vacías, genera un
/// par y lo persiste de forma atómica.
///
/// Si ya existen, no escribe nada y devuelve el par almacenado. Si hay que
/// generarlo, la escritura ocurre dentro de una transacción de modo que dos
/// peticiones concurrentes converjan en el mismo par: gana la que escribe
/// primero y la otra lee el par ya persistido.
pub async fn load_or_generate(pool: &SqlitePool) -> Result<VapidKeys, PushError> {
    // Camino rápido: el par ya está persistido y no está vacío.
    let public = non_empty(SettingsRepo::get(pool, PUBLIC_KEY_SETTING).await?);
    let private = non_empty(SettingsRepo::get(pool, PRIVATE_KEY_SETTING).await?);
    if let (Some(public_key), Some(private_key)) = (public, private) {
        let keys = VapidKeys {
            public_key,
            private_key,
        };
        validate_pair(&keys)?;
        return Ok(keys);
    }

    // Candidato nuevo, convergente con cualquier llamada concurrente.
    let candidate = generate_vapid_pair();

    let mut tx = pool.begin().await?;
    upsert_pair(&mut tx, &candidate).await?;
    let public = read_setting(&mut tx, PUBLIC_KEY_SETTING).await?;
    let private = read_setting(&mut tx, PRIVATE_KEY_SETTING).await?;
    tx.commit().await?;

    let keys = VapidKeys {
        public_key: public.ok_or(PushError::InvalidStoredKey)?,
        private_key: private.ok_or(PushError::InvalidStoredKey)?,
    };
    validate_pair(&keys)?;
    Ok(keys)
}

/// Contacto VAPID (`sub` de la firma). Los servicios de push habituales exigen
/// un `mailto:` o una URL de contacto del administrador.
const VAPID_CONTACT: &str = "mailto:push@valet.local";

/// Construye la petición `POST` WebPush lista para enviar con `reqwest`.
///
/// Cifra `payload` conforme a RFC 8188/8291 (`aes128gcm`) y firma con VAPID
/// (RFC 8292) usando el par `keys`. El `endpoint`, la clave `p256dh` (65 bytes
/// `uncompressed` en base64url sin padding) y el `auth` (16 bytes en base64url
/// sin padding) proceden de la suscripción del navegador.
///
/// No realiza ninguna operación de red: devuelve un [`http::Request`] con el
/// cuerpo cifrado y las cabeceras `TTL`, `Content-Encoding`, `Content-Length`,
/// `Content-Type` y `Authorization: vapid t=…`.
pub fn build_request(
    keys: &VapidKeys,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    payload: &str,
) -> Result<http::Request<Vec<u8>>, PushError> {
    let endpoint_uri: http::Uri = endpoint.parse().map_err(|_| PushError::InvalidEndpoint)?;

    let private_raw = engine()
        .decode(&keys.private_key)
        .map_err(|_| PushError::InvalidStoredKey)?;
    let key_pair =
        ES256KeyPair::from_bytes(&private_raw).map_err(|_| PushError::InvalidStoredKey)?;

    let p256dh_raw = engine()
        .decode(p256dh)
        .map_err(|_| PushError::InvalidSubscription)?;
    let ua_public =
        PublicKey::from_sec1_bytes(&p256dh_raw).map_err(|_| PushError::InvalidSubscription)?;

    let auth_raw: [u8; 16] = engine()
        .decode(auth)
        .map_err(|_| PushError::InvalidSubscription)?
        .try_into()
        .map_err(|_| PushError::InvalidSubscription)?;
    // `Auth` es un `GenericArray<u8, U16>`; la conversión desde `[u8; 16]`
    // garantiza la longitud exacta sin entrar en pánico.
    let ua_auth = Auth::from(auth_raw);

    WebPushBuilder::new(endpoint_uri, ua_public, ua_auth)
        .with_vapid(&key_pair, VAPID_CONTACT)
        .build(payload.as_bytes())
        .map_err(PushError::Build)
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
            .expect("in-memory sqlite");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        pool
    }

    /// Scenario: la primera llamada genera el par y lo persiste.
    #[tokio::test]
    async fn vapid_first_call_generates_and_persists() {
        let pool = setup().await;

        let keys = load_or_generate(&pool)
            .await
            .expect("the first call must generate a pair");

        assert!(!keys.public_key.is_empty(), "public key must not be empty");
        assert!(
            !keys.private_key.is_empty(),
            "private key must not be empty"
        );

        let stored_public = SettingsRepo::get(&pool, PUBLIC_KEY_SETTING)
            .await
            .unwrap()
            .expect("push_vapid_public_key must exist");
        let stored_private = SettingsRepo::get(&pool, PRIVATE_KEY_SETTING)
            .await
            .unwrap()
            .expect("push_vapid_private_key must exist");

        assert!(
            !stored_public.is_empty(),
            "stored public key must not be empty"
        );
        assert!(
            !stored_private.is_empty(),
            "stored private key must not be empty"
        );
        assert_eq!(stored_public, keys.public_key);
        assert_eq!(stored_private, keys.private_key);
    }

    /// Scenario: la segunda llamada reutiliza el par existente.
    #[tokio::test]
    async fn vapid_second_call_reuses_existing_pair() {
        let pool = setup().await;

        let first = load_or_generate(&pool).await.unwrap();
        let second = load_or_generate(&pool).await.unwrap();

        assert_eq!(
            first.public_key, second.public_key,
            "the public key must be reused, not regenerated"
        );
        assert_eq!(first.private_key, second.private_key);

        let stored_public = SettingsRepo::get(&pool, PUBLIC_KEY_SETTING)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored_public, first.public_key);
    }

    /// Scenario: dos generaciones concurrentes convergen en un único par.
    #[tokio::test]
    async fn vapid_concurrent_calls_converge() {
        let pool = setup().await;

        let (a, b) = tokio::join!(load_or_generate(&pool), load_or_generate(&pool));
        let a = a.expect("first concurrent call must succeed");
        let b = b.expect("second concurrent call must succeed");

        assert_eq!(a.public_key, b.public_key, "both calls must converge");
        assert_eq!(a.private_key, b.private_key);

        let stored_public = SettingsRepo::get(&pool, PUBLIC_KEY_SETTING)
            .await
            .unwrap()
            .unwrap();
        let stored_private = SettingsRepo::get(&pool, PRIVATE_KEY_SETTING)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored_public, a.public_key);
        assert_eq!(stored_private, a.private_key);
    }

    /// Scenario: la clave privada no aparece ni en el `Debug` del struct ni en
    /// el mensaje de un error.
    #[test]
    fn vapid_private_key_never_leaks_in_debug_or_error() {
        let keys = VapidKeys {
            public_key: "public-canary".to_string(),
            private_key: "CANARY-PRIVATE-KEY".to_string(),
        };

        let debug = format!("{keys:?}");
        assert!(
            !debug.contains("CANARY-PRIVATE-KEY"),
            "private key leaked in Debug: {debug}"
        );
        assert!(
            debug.contains("***"),
            "private key must be redacted: {debug}"
        );
        assert!(debug.contains("public-canary"), "{debug}");

        let error = PushError::InvalidStoredKey;
        let message = format!("{error} {error:?}");
        assert!(
            !message.contains("CANARY-PRIVATE-KEY"),
            "private key leaked in error: {message}"
        );
    }

    /// Suscriptor de contrato: 65 bytes `uncompressed` en base64url sin padding
    /// y un `auth` de 16 bytes. Son fijos porque el cifrado solo necesita un
    /// punto SEC1 válido, no la clave privada del cliente.
    const CONTRACT_P256DH: &str =
        "BLn9b-VR0ca83knDNZ32dCHGyjJp-1riX9ZTN40MqV8K_LpQmLqxC_DoHvqvFXO_nGdAB4W9dogZb_sM-uV4JbY";
    const CONTRACT_AUTH: &str = "_ordMnz7uTCmrpBTeUV4Bw";

    /// Scenario: la petición construida respeta el formato RFC 8291 (aes128gcm)
    /// y RFC 8292 (VAPID): método `POST`, cabeceras `TTL`, `Content-Encoding`,
    /// `Content-Length` y `Authorization`, y cuerpo con `salt`, `rs`, `idlen` y
    /// la clave efímera en los offsets correctos. Sin red.
    #[test]
    fn push_request_contract_has_rfc8291_layout() {
        let keys = generate_vapid_pair();

        let request = build_request(
            &keys,
            "https://example.invalid/push/abc",
            CONTRACT_P256DH,
            CONTRACT_AUTH,
            r#"{"title":"t","body":"b"}"#,
        )
        .expect("the request must be built");

        assert_eq!(request.method(), http::Method::POST);

        let headers = request.headers();
        assert_eq!(
            headers
                .get("content-encoding")
                .and_then(|v| v.to_str().ok()),
            Some("aes128gcm"),
            "Content-Encoding must be aes128gcm"
        );
        assert!(
            headers.contains_key("ttl"),
            "the TTL header must be present"
        );

        let authorization = headers
            .get("authorization")
            .expect("Authorization must be present")
            .to_str()
            .expect("Authorization must be valid ASCII");
        assert!(
            authorization.starts_with("vapid t="),
            "Authorization must start with `vapid t=`: {authorization}"
        );

        let body = request.body();
        let content_length: usize = headers
            .get("content-length")
            .expect("Content-Length must be present")
            .to_str()
            .expect("Content-Length must be valid ASCII")
            .parse()
            .expect("Content-Length must be a number");
        assert_eq!(
            content_length,
            body.len(),
            "Content-Length must match the body length"
        );

        assert!(
            body.len() >= 86,
            "the RFC 8291 body must be at least 86 bytes: {}",
            body.len()
        );
        // `salt` vive en [0..16]; solo comprobamos que la ventana existe.
        let _salt = &body[0..16];
        let _rs = u32::from_be_bytes(body[16..20].try_into().expect("rs window of 4 bytes"));
        assert_eq!(body[20], 65, "idlen must be 65");
        assert_eq!(
            body[21], 0x04,
            "the ephemeral public key must start with 0x04"
        );
    }

    /// Scenario: una clave almacenada inválida no se refleja en el error.
    #[tokio::test]
    async fn vapid_invalid_stored_key_does_not_leak() {
        let pool = setup().await;
        SettingsRepo::set(&pool, PUBLIC_KEY_SETTING, "AQAB")
            .await
            .unwrap();
        SettingsRepo::set(&pool, PRIVATE_KEY_SETTING, "CANARY-PRIVATE-KEY!!!")
            .await
            .unwrap();

        let error = load_or_generate(&pool)
            .await
            .expect_err("an invalid stored key must be rejected");

        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("CANARY-PRIVATE-KEY"),
            "private key leaked in error: {shown}"
        );
    }
}
