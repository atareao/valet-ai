//! Cliente de Strava (OAuth2, un único atleta, solo lectura).
//!
//! Ver `openspec/specs/tools/strava/spec.md` (R1–R3).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use sqlx::SqlitePool;

use crate::auth::StateStore;
use crate::db::repos::settings::SettingsRepo;

/// Scope OAuth solicitado a Strava: solo lectura (nunca `activity:write`).
pub const STRAVA_SCOPE: &str = "read,activity:read_all";

/// Claves de `settings` que almacenan los tokens/identidad del atleta.
/// La desconexión las vacía (se ponen a `""`).
const TOKEN_KEYS: [&str; 6] = [
    "strava_access_token",
    "strava_refresh_token",
    "strava_expires_at",
    "strava_athlete_id",
    "strava_athlete_name",
    "strava_scope",
];

/// Margen (en segundos) con el que se considera un access token todavía válido.
const REFRESH_MARGIN_SECS: i64 = 3600;

/// Content type de los cuerpos `application/x-www-form-urlencoded`.
const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";

/// Ventana (en segundos) durante la cual una lectura repetida se sirve de caché.
const CACHE_TTL_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct StravaStatus {
    pub connected: bool,
    pub athlete_id: Option<String>,
    pub athlete_name: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StravaError {
    #[error("no hay conexión con Strava: conecta tu cuenta")]
    NotConnected,
    #[error("falta el client_id/client_secret de Strava")]
    MissingCredentials,
    #[error("state OAuth inválido o caducado")]
    InvalidState,
    #[error("la autorización fue denegada")]
    Denied,
    #[error("error de Strava: {0}")]
    Http(String),
    #[error("Strava ha alcanzado el límite de peticiones; inténtalo dentro de unos minutos")]
    RateLimited,
    #[error("error interno: {0}")]
    Internal(String),
}

/// Respuesta del endpoint `oauth/token` de Strava.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    athlete: Option<Athlete>,
}

#[derive(Debug, Deserialize)]
struct Athlete {
    id: i64,
    #[serde(default)]
    firstname: Option<String>,
    #[serde(default)]
    lastname: Option<String>,
}

#[derive(Clone)]
pub struct Strava {
    oauth_base: String,
    /// Base de la API de datos; la consumen las herramientas de consulta.
    api_base: String,
    http: reqwest::Client,
    /// Namespace de la caché en proceso, único por instancia. Dos clientes
    /// distintos no comparten entradas aunque consulten la misma URL: en
    /// producción cada herramienta conserva su `Strava` durante toda la vida
    /// del proceso, y en los tests una instancia nueva por prueba evita que un
    /// puerto reutilizado sirva una respuesta ajena desde la caché.
    cache_ns: u64,
}

/// Secuencia que asigna un namespace distinto a cada instancia de [`Strava`].
fn next_cache_ns() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

fn state_store() -> &'static StateStore {
    static STORE: OnceLock<StateStore> = OnceLock::new();
    STORE.get_or_init(StateStore::new)
}

/// Único punto de refresco por proceso: dos turnos concurrentes no se pisan.
fn refresh_lock() -> &'static tokio::sync::Mutex<()> {
    static REFRESH: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    REFRESH.get_or_init(|| tokio::sync::Mutex::new(()))
}

impl Strava {
    /// Cliente apuntando a las bases reales de Strava.
    pub fn new() -> Self {
        Self::with_bases(
            "https://www.strava.com".to_string(),
            "https://www.strava.com/api/v3".to_string(),
        )
    }

    /// Cliente con bases inyectables (tests con `wiremock`).
    pub fn with_bases(oauth_base: String, api_base: String) -> Self {
        Self {
            oauth_base,
            api_base,
            http: reqwest::Client::new(),
            cache_ns: next_cache_ns(),
        }
    }

    /// Compone la URL de autorización de Strava y registra un `state` de un solo uso.
    pub async fn authorize_url(
        &self,
        pool: &SqlitePool,
        redirect_url: &str,
    ) -> Result<String, StravaError> {
        let (client_id, _client_secret) = self.credentials(pool).await?;
        let (state, _nonce) = state_store().issue();

        Ok(format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope={}",
            self.oauth_base,
            percent_encode(&client_id),
            percent_encode(redirect_url),
            percent_encode(&state),
            STRAVA_SCOPE,
        ))
    }

    /// Valida el `state` y canjea el `code` por tokens.
    ///
    /// Convención de la firma: cuando Strava deniega el acceso, el handler de
    /// la ruta pasa `error=access_denied` como `code`, de modo que aquí se
    /// distingue la denegación del canje normal.
    pub async fn handle_callback(
        &self,
        pool: &SqlitePool,
        code: &str,
        state: &str,
        redirect_url: &str,
    ) -> Result<(), StravaError> {
        // El `state` se consume exactamente una vez, y antes de tocar `settings`:
        // un callback con state inválido o caducado no deja rastro.
        state_store()
            .consume(state)
            .ok_or(StravaError::InvalidState)?;

        if code == "access_denied" {
            return Err(StravaError::Denied);
        }

        let (client_id, client_secret) = self.credentials(pool).await?;
        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_url),
        ];

        let response = self
            .http
            .post(format!("{}/oauth/token", self.oauth_base))
            .header(reqwest::header::CONTENT_TYPE, FORM_CONTENT_TYPE)
            .body(form_body(&params))
            .send()
            .await
            .map_err(|error| StravaError::Http(error.to_string()))?;

        if !response.status().is_success() {
            return Err(StravaError::Http(format!(
                "el canje del código falló con estado {}",
                response.status().as_u16()
            )));
        }

        let body: TokenResponse = response
            .json()
            .await
            .map_err(|error| StravaError::Http(error.to_string()))?;

        persist_tokens(pool, &body).await
    }

    /// Devuelve un access token válido, refrescándolo si está caducado.
    ///
    /// El refresco se serializa con un lock de proceso y hace *double-check* de
    /// `strava_expires_at` tras adquirirlo, de forma que dos turnos concurrentes
    /// comparten un único refresco.
    pub async fn access_token(&self, pool: &SqlitePool) -> Result<String, StravaError> {
        let token = self.setting(pool, "strava_access_token").await?;
        let expires_at = parse_expires_at(self.setting(pool, "strava_expires_at").await?);
        if token_still_valid(&token, expires_at) {
            return Ok(token);
        }

        if self.setting(pool, "strava_refresh_token").await?.is_empty() {
            return Err(StravaError::NotConnected);
        }

        self.refresh(pool).await
    }

    /// Refresca el access token (lock de proceso + rotación del refresh token).
    ///
    /// Es el único punto de refresco: tras adquirir el lock se vuelve a leer el
    /// token, de modo que dos turnos concurrentes comparten un único refresco.
    /// El `refresh_token` devuelto sustituye **siempre** al anterior, porque
    /// Strava invalida el viejo en cada refresco.
    async fn refresh(&self, pool: &SqlitePool) -> Result<String, StravaError> {
        let _guard = refresh_lock().lock().await;

        // Double-check: otro turno pudo refrescar mientras esperábamos el lock.
        let token = self.setting(pool, "strava_access_token").await?;
        let expires_at = parse_expires_at(self.setting(pool, "strava_expires_at").await?);
        if token_still_valid(&token, expires_at) {
            return Ok(token);
        }

        // El refresh token pudo rotar mientras esperábamos el lock.
        let refresh_token = self.setting(pool, "strava_refresh_token").await?;
        if refresh_token.is_empty() {
            return Err(StravaError::NotConnected);
        }

        let (client_id, client_secret) = self.credentials(pool).await?;
        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ];

        let response = self
            .http
            .post(format!("{}/oauth/token", self.oauth_base))
            .header(reqwest::header::CONTENT_TYPE, FORM_CONTENT_TYPE)
            .body(form_body(&params))
            .send()
            .await
            .map_err(|error| StravaError::Http(error.to_string()))?;

        if !response.status().is_success() {
            return Err(StravaError::Http(format!(
                "el refresco del token falló con estado {}",
                response.status().as_u16()
            )));
        }

        let body: TokenResponse = response
            .json()
            .await
            .map_err(|error| StravaError::Http(error.to_string()))?;

        // El refresh token devuelto sustituye siempre al anterior.
        persist_tokens(pool, &body).await?;
        Ok(body.access_token)
    }

    /// `GET` autenticado contra la API de datos de Strava.
    ///
    /// Sirve las repeticiones dentro de [`CACHE_TTL_SECS`] desde una caché en
    /// proceso —sin salir a la red— y, ante un `401`, refresca el token y
    /// reintenta **una sola vez**.
    pub async fn api_get(
        &self,
        pool: &SqlitePool,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<serde_json::Value, StravaError> {
        let url = build_url(&self.api_base, path, query);
        let cache_key = format!("{}:{url}", self.cache_ns);
        if let Some(cached) = cache_get(&cache_key) {
            return Ok(cached);
        }

        let token = self.access_token(pool).await?;
        let response = self.send_get(&url, &token).await?;

        match response.status().as_u16() {
            200..=299 => {
                let body = parse_json(response).await?;
                cache_put(&cache_key, &body);
                Ok(body)
            }
            401 => {
                // El token que conoce Strava ya no sirve aunque `expires_at`
                // diga lo contrario: se invalida para forzar el refresco.
                SettingsRepo::set(pool, "strava_access_token", "")
                    .await
                    .map_err(internal)?;
                SettingsRepo::set(pool, "strava_expires_at", "")
                    .await
                    .map_err(internal)?;

                let token = self.refresh(pool).await?;
                let response = self.send_get(&url, &token).await?;
                if !response.status().is_success() {
                    return Err(StravaError::Http(format!(
                        "Strava respondió {} tras refrescar el token",
                        response.status().as_u16()
                    )));
                }

                let body = parse_json(response).await?;
                cache_put(&cache_key, &body);
                Ok(body)
            }
            429 => Err(StravaError::RateLimited),
            status => Err(StravaError::Http(format!(
                "Strava respondió {status} al consultar {path}"
            ))),
        }
    }

    /// Envía un `GET` con `Authorization: Bearer`.
    async fn send_get(&self, url: &str, token: &str) -> Result<reqwest::Response, StravaError> {
        self.http
            .get(url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .map_err(|error| StravaError::Http(error.to_string()))
    }

    /// Revoca el acceso en Strava (best-effort) y borra los tokens locales.
    ///
    /// No falla si no había conexión: en ese caso solo limpia (si acaso) las claves.
    pub async fn disconnect(&self, pool: &SqlitePool) -> Result<(), StravaError> {
        let refresh_token = self.setting(pool, "strava_refresh_token").await?;

        if !refresh_token.is_empty() {
            if let Ok((client_id, client_secret)) = self.credentials(pool).await {
                use base64::Engine as _;
                let credentials = base64::engine::general_purpose::STANDARD
                    .encode(format!("{client_id}:{client_secret}"));

                // Best-effort: si la revocación falla, igualmente limpiamos.
                let _ = self
                    .http
                    .post(format!("{}/oauth/revoke", self.oauth_base))
                    .header(
                        reqwest::header::AUTHORIZATION,
                        format!("Basic {credentials}"),
                    )
                    .header(reqwest::header::CONTENT_TYPE, FORM_CONTENT_TYPE)
                    .body(form_body(&[("token", refresh_token.as_str())]))
                    .send()
                    .await;
            }
        }

        for key in TOKEN_KEYS {
            SettingsRepo::set(pool, key, "").await.map_err(internal)?;
        }

        Ok(())
    }

    /// Estado de la conexión, sin revelar jamás los tokens.
    pub async fn status(&self, pool: &SqlitePool) -> Result<StravaStatus, StravaError> {
        let refresh_token = self.setting(pool, "strava_refresh_token").await?;
        let athlete_id = SettingsRepo::get(pool, "strava_athlete_id")
            .await
            .map_err(internal)?
            .filter(|value| !value.trim().is_empty());
        let athlete_name = SettingsRepo::get(pool, "strava_athlete_name")
            .await
            .map_err(internal)?
            .filter(|value| !value.trim().is_empty());
        let scope = SettingsRepo::get(pool, "strava_scope")
            .await
            .map_err(internal)?
            .filter(|value| !value.trim().is_empty());

        Ok(StravaStatus {
            connected: !refresh_token.is_empty(),
            athlete_id,
            athlete_name,
            scope,
        })
    }

    /// Lee una clave de `settings`, devolviendo `""` si no existe.
    async fn setting(&self, pool: &SqlitePool, key: &str) -> Result<String, StravaError> {
        Ok(SettingsRepo::get(pool, key)
            .await
            .map_err(internal)?
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    /// Resuelve `client_id`/`client_secret`: primero `settings`, luego ENV.
    async fn credentials(&self, pool: &SqlitePool) -> Result<(String, String), StravaError> {
        let client_id = self
            .setting_or_env(pool, "strava_client_id", "STRAVA_CLIENT_ID")
            .await?;
        let client_secret = self
            .setting_or_env(pool, "strava_client_secret", "STRAVA_CLIENT_SECRET")
            .await?;

        match (client_id, client_secret) {
            (Some(client_id), Some(client_secret)) => Ok((client_id, client_secret)),
            _ => Err(StravaError::MissingCredentials),
        }
    }

    /// Lee una clave de `settings`; si está vacía, cae a la variable de entorno.
    async fn setting_or_env(
        &self,
        pool: &SqlitePool,
        key: &str,
        env: &str,
    ) -> Result<Option<String>, StravaError> {
        if let Some(value) = SettingsRepo::get(pool, key)
            .await
            .map_err(internal)?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(Some(value));
        }

        Ok(std::env::var(env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()))
    }
}

impl Default for Strava {
    fn default() -> Self {
        Self::new()
    }
}

/// Persiste los tokens devueltos por Strava en `settings`.
///
/// El `refresh_token` rotado se confirma **primero y en su propia transacción**
/// (commit inmediato): Strava invalida el refresh token anterior en cada
/// refresco, así que si un write posterior fallara y su rollback revirtiera
/// también el refresh token, quedaría en la base el viejo —ya muerto— y la
/// integración moriría hasta re-autorizar. Confirmándolo aparte, un fallo en el
/// resto nunca lo pierde y el siguiente [`Strava::access_token`] puede
/// recuperarse refrescando de nuevo.
///
/// El access token, `expires_at`, el atleta y el scope se escriben **después**,
/// en una segunda transacción. Esta sí es atómica entre sus claves (nunca un
/// access token nuevo con un `expires_at` viejo), pero un fallo aquí ya no
/// arrastra al refresh token. Si el atleta no viene en la respuesta, el nombre
/// previo no se toca.
async fn persist_tokens(pool: &SqlitePool, body: &TokenResponse) -> Result<(), StravaError> {
    // 1) El refresh token rotado se confirma solo, de inmediato, para que
    //    ningún fallo posterior lo revierta.
    let mut refresh_tx = pool.begin().await.map_err(internal)?;
    set_in_tx(&mut refresh_tx, "strava_refresh_token", &body.refresh_token).await?;
    refresh_tx.commit().await.map_err(internal)?;

    // 2) El resto, en su propia transacción. Un fallo aquí no revierte (1).
    let mut tx = pool.begin().await.map_err(internal)?;

    set_in_tx(&mut tx, "strava_access_token", &body.access_token).await?;
    set_in_tx(&mut tx, "strava_expires_at", &body.expires_at.to_string()).await?;

    if let Some(athlete) = &body.athlete {
        set_in_tx(&mut tx, "strava_athlete_id", &athlete.id.to_string()).await?;

        let name = athlete_name(athlete);
        if !name.is_empty() {
            set_in_tx(&mut tx, "strava_athlete_name", &name).await?;
        }
    }
    if let Some(scope) = &body.scope {
        set_in_tx(&mut tx, "strava_scope", scope).await?;
    }

    tx.commit().await.map_err(internal)?;
    Ok(())
}

/// `INSERT ... ON CONFLICT` de una clave de `settings` dentro de una transacción.
///
/// Réplica de [`SettingsRepo::set`] sobre el ejecutor de la transacción, para
/// que cada lote de escrituras de [`persist_tokens`] sea atómico.
async fn set_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
    value: &str,
) -> Result<(), StravaError> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(key)
    .bind(value)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}

/// Convierte un error de `sqlx` en [`StravaError::Internal`].
fn internal(error: sqlx::Error) -> StravaError {
    StravaError::Internal(error.to_string())
}

/// Nombre visible del atleta: `"{firstname} {lastname}"` recortado.
///
/// Cualquiera de los dos campos puede faltar; si ninguno llega, devuelve `""`.
fn athlete_name(athlete: &Athlete) -> String {
    let first = athlete.firstname.as_deref().unwrap_or_default().trim();
    let last = athlete.lastname.as_deref().unwrap_or_default().trim();
    format!("{first} {last}").trim().to_string()
}

/// `true` si el token no está vacío y le quedan más de [`REFRESH_MARGIN_SECS`].
fn token_still_valid(token: &str, expires_at: Option<i64>) -> bool {
    !token.is_empty()
        && expires_at
            .map(|expires_at| expires_at.saturating_sub(current_epoch_secs()) > REFRESH_MARGIN_SECS)
            .unwrap_or(false)
}

/// Parsea el `expires_at` (epoch en segundos) almacenado como texto.
fn parse_expires_at(raw: String) -> Option<i64> {
    raw.parse::<i64>().ok()
}

/// Epoch actual en segundos.
fn current_epoch_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Serializa pares clave/valor como `application/x-www-form-urlencoded`.
fn form_body(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Percent-encoding mínimo para valores de query (RFC 3986 unreserved).
///
/// Se codifica solo lo imprescindible para no romper `redirect_uri`, y se deja
/// el `scope` sin codificar para que la URL contenga el literal
/// `scope=read,activity:read_all`.
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Compone la URL de la API de datos: `{api_base}{path}?{query}`.
///
/// Los parámetros llegan **ya formateados** (`key=value`), de modo que aquí solo
/// se concatenan; la URL resultante también sirve como clave de caché.
fn build_url(api_base: &str, path: &str, query: &[(&str, String)]) -> String {
    if query.is_empty() {
        return format!("{api_base}{path}");
    }
    let query_string = query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{api_base}{path}?{query_string}")
}

/// Caché en proceso de las respuestas correctas de la API de datos.
fn api_cache() -> &'static Mutex<HashMap<String, (Instant, serde_json::Value)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, serde_json::Value)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Devuelve la respuesta cacheada si sigue dentro de [`CACHE_TTL_SECS`].
fn cache_get(url: &str) -> Option<serde_json::Value> {
    let cache = api_cache().lock().ok()?;
    let (stored_at, value) = cache.get(url)?;
    if stored_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
        Some(value.clone())
    } else {
        None
    }
}

/// Guarda una respuesta correcta en la caché corta.
///
/// Antes de insertar purga las entradas ya caducadas: una entrada fuera de
/// [`CACHE_TTL_SECS`] nunca se sirve, así que retirarla es seguro y evita que
/// la caché crezca sin límite.
fn cache_put(url: &str, value: &serde_json::Value) {
    if let Ok(mut cache) = api_cache().lock() {
        cache_put_in(&mut cache, url, value, Instant::now());
    }
}

/// Implementación de [`cache_put`] sobre un mapa explícito.
///
/// Separada para poder probar la purga con instantes sintéticos, sin depender
/// del reloj real (mismo patrón que `StateStore::issue_with_now`).
fn cache_put_in(
    cache: &mut HashMap<String, (Instant, serde_json::Value)>,
    url: &str,
    value: &serde_json::Value,
    now: Instant,
) {
    cache.retain(|_, (stored_at, _)| {
        now.saturating_duration_since(*stored_at) < Duration::from_secs(CACHE_TTL_SECS)
    });
    cache.insert(url.to_string(), (now, value.clone()));
}

/// Parsea el cuerpo JSON de una respuesta.
async fn parse_json(response: reqwest::Response) -> Result<serde_json::Value, StravaError> {
    response
        .json()
        .await
        .map_err(|error| StravaError::Http(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const REDIRECT: &str = "https://app.example.com/api/strava/callback";

    async fn setup_pool() -> SqlitePool {
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

    async fn seed_creds(pool: &SqlitePool) {
        SettingsRepo::set(pool, "strava_client_id", "cid")
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_client_secret", "csecret")
            .await
            .unwrap();
    }

    async fn get(pool: &SqlitePool, key: &str) -> String {
        SettingsRepo::get(pool, key)
            .await
            .unwrap()
            .unwrap_or_default()
    }

    fn extract_state(url: &str) -> String {
        url::Url::parse(url)
            .expect("valid url")
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .expect("state in authorize url")
    }

    fn now_epoch() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[tokio::test]
    async fn authorize_url_includes_read_only_scope_and_state() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;

        let strava = Strava::with_bases("https://oauth.test".into(), "https://api.test".into());
        let url = strava.authorize_url(&pool, REDIRECT).await.unwrap();

        assert!(
            url.starts_with("https://oauth.test/oauth/authorize?"),
            "url: {url}"
        );
        assert!(url.contains("scope=read,activity:read_all"), "url: {url}");
        assert!(url.contains("response_type=code"), "url: {url}");
        assert!(url.contains("client_id=cid"), "url: {url}");
        assert!(url.contains("state="), "url: {url}");
        assert!(!url.contains("activity:write"), "url: {url}");
    }

    #[tokio::test]
    async fn callback_rejects_invalid_state() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;

        let strava = Strava::with_bases("https://oauth.test".into(), "https://api.test".into());
        let err = strava
            .handle_callback(&pool, "thecode", "unknown-state", REDIRECT)
            .await
            .unwrap_err();

        assert!(matches!(err, StravaError::InvalidState));
        for key in TOKEN_KEYS {
            assert_eq!(get(&pool, key).await, "", "no token for {key}");
        }
    }

    #[tokio::test]
    async fn callback_denied_leaves_no_trace() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;

        let strava = Strava::with_bases("https://oauth.test".into(), "https://api.test".into());
        let url = strava.authorize_url(&pool, REDIRECT).await.unwrap();
        let state = extract_state(&url);

        let err = strava
            .handle_callback(&pool, "access_denied", &state, REDIRECT)
            .await
            .unwrap_err();

        assert!(matches!(err, StravaError::Denied));
        for key in TOKEN_KEYS {
            assert_eq!(get(&pool, key).await, "", "no token for {key}");
        }
    }

    #[tokio::test]
    async fn callback_exchange_persists_tokens() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "access_token": "ACC",
                "refresh_token": "REF",
                "expires_at": 1_700_000_000,
                "expires_in": 21600,
                "scope": "read,activity:read_all",
                "athlete": { "id": 12345, "firstname": "Jane", "lastname": "Doe" }
            })))
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let url = strava.authorize_url(&pool, REDIRECT).await.unwrap();
        let state = extract_state(&url);

        strava
            .handle_callback(&pool, "thecode", &state, REDIRECT)
            .await
            .unwrap();

        assert_eq!(get(&pool, "strava_access_token").await, "ACC");
        assert_eq!(get(&pool, "strava_refresh_token").await, "REF");
        assert_eq!(get(&pool, "strava_expires_at").await, "1700000000");
        assert_eq!(get(&pool, "strava_athlete_id").await, "12345");
        assert_eq!(get(&pool, "strava_athlete_name").await, "Jane Doe");
        assert_eq!(get(&pool, "strava_scope").await, "read,activity:read_all");
    }

    #[tokio::test]
    async fn refresh_rotates_and_persists_new_refresh_token() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;
        SettingsRepo::set(&pool, "strava_refresh_token", "OLD")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_access_token", "OLDACC")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_expires_at", "1000")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_athlete_name", "Old Name")
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "access_token": "NEWACC",
                "refresh_token": "NEWREF",
                "expires_at": now_epoch() + 21600,
                "expires_in": 21600
            })))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let token = strava.access_token(&pool).await.unwrap();

        assert_eq!(token, "NEWACC");
        assert_eq!(get(&pool, "strava_access_token").await, "NEWACC");
        assert_eq!(get(&pool, "strava_refresh_token").await, "NEWREF");
        assert_ne!(get(&pool, "strava_refresh_token").await, "OLD");
        // Sin atleta en la respuesta, el nombre previo no se toca.
        assert_eq!(get(&pool, "strava_athlete_name").await, "Old Name");
    }

    /// El refresh token rotado se confirma en su propia transacción: un fallo al
    /// persistir el resto del lote **no** lo revierte. Aquí se fuerza el fallo
    /// de la escritura del access token con un trigger que aborta y se comprueba
    /// que el refresh token devuelto por Strava sigue en `settings`, de modo que
    /// la siguiente llamada a `access_token` puede recuperarse refrescando.
    #[tokio::test]
    async fn persist_tokens_keeps_rotated_refresh_token_when_the_rest_fails() {
        let pool = setup_pool().await;

        // Trigger que aborta cualquier escritura de `strava_access_token`: hace
        // fallar el segundo lote de `persist_tokens` (el primero, el del refresh
        // token, escribe otra clave y no lo dispara).
        let trigger = "CREATE TRIGGER fail_access_token_insert BEFORE INSERT ON settings
             WHEN NEW.key = 'strava_access_token'
             BEGIN SELECT RAISE(ABORT, 'forced failure'); END;
             CREATE TRIGGER fail_access_token_update BEFORE UPDATE ON settings
             WHEN NEW.key = 'strava_access_token'
             BEGIN SELECT RAISE(ABORT, 'forced failure'); END";
        sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
            .execute(&pool)
            .await
            .unwrap();

        // Estado previo: el refresh token viejo que Strava acaba de invalidar.
        SettingsRepo::set(&pool, "strava_refresh_token", "OLD")
            .await
            .unwrap();

        let body = TokenResponse {
            access_token: "NEWACC".to_string(),
            refresh_token: "NEWREF".to_string(),
            expires_at: now_epoch() + 21600,
            scope: Some("read,activity:read_all".to_string()),
            athlete: Some(Athlete {
                id: 42,
                firstname: Some("Jane".to_string()),
                lastname: Some("Doe".to_string()),
            }),
        };

        let err = persist_tokens(&pool, &body).await.unwrap_err();
        assert!(matches!(err, StravaError::Internal(_)), "err: {err:?}");

        // El refresh token rotado sobrevive al rollback del resto del lote...
        assert_eq!(get(&pool, "strava_refresh_token").await, "NEWREF");
        assert_ne!(get(&pool, "strava_refresh_token").await, "OLD");
        // ...y el resto no llegó a confirmarse.
        assert_eq!(get(&pool, "strava_access_token").await, "");
    }

    #[tokio::test]
    async fn concurrent_refreshes_share_a_single_refresh() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;
        SettingsRepo::set(&pool, "strava_refresh_token", "OLD")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_access_token", "OLDACC")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_expires_at", "1000")
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "access_token": "NEWACC",
                "refresh_token": "NEWREF",
                "expires_at": now_epoch() + 21600,
                "expires_in": 21600
            })))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let (a, b) = tokio::join!(strava.access_token(&pool), strava.access_token(&pool));

        assert_eq!(a.unwrap(), "NEWACC");
        assert_eq!(b.unwrap(), "NEWACC");
        assert_eq!(get(&pool, "strava_refresh_token").await, "NEWREF");
    }

    #[tokio::test]
    async fn valid_access_token_is_reused_without_network() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;
        SettingsRepo::set(&pool, "strava_refresh_token", "REF")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_access_token", "VALID")
            .await
            .unwrap();
        SettingsRepo::set(
            &pool,
            "strava_expires_at",
            &(now_epoch() + 2 * 3600).to_string(),
        )
        .await
        .unwrap();

        let server = MockServer::start().await;
        // Ningún refresco debe salir a la red.
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let token = strava.access_token(&pool).await.unwrap();

        assert_eq!(token, "VALID");
    }

    #[tokio::test]
    async fn status_never_exposes_tokens() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "strava_refresh_token", "REF")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_access_token", "ACC")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_athlete_id", "999")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_athlete_name", "Jane Doe")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_scope", "read,activity:read_all")
            .await
            .unwrap();

        let strava = Strava::new();
        let status = strava.status(&pool).await.unwrap();

        assert!(status.connected);
        assert_eq!(status.athlete_id.as_deref(), Some("999"));
        assert_eq!(status.athlete_name.as_deref(), Some("Jane Doe"));
        assert_eq!(status.scope.as_deref(), Some("read,activity:read_all"));

        // Un literal exhaustivo (sin `..`) demuestra que no hay campos de token.
        let explicit = StravaStatus {
            connected: true,
            athlete_id: None,
            athlete_name: None,
            scope: None,
        };
        assert!(explicit.connected);

        let debug = format!("{status:?}");
        assert!(!debug.contains("ACC"), "debug leaks access token: {debug}");
        assert!(!debug.contains("REF"), "debug leaks refresh token: {debug}");
    }

    #[tokio::test]
    async fn disconnect_revokes_and_clears() {
        let pool = setup_pool().await;
        seed_creds(&pool).await;
        SettingsRepo::set(&pool, "strava_refresh_token", "REF")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_access_token", "ACC")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_expires_at", "1000")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_athlete_id", "999")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "strava_scope", "read,activity:read_all")
            .await
            .unwrap();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        strava.disconnect(&pool).await.unwrap();

        for key in TOKEN_KEYS {
            assert_eq!(get(&pool, key).await, "", "{key} must be cleared");
        }
    }

    /// Siembra una conexión con un access token vigente (más de 1 h de margen),
    /// de modo que `access_token` no necesite salir a refrescar.
    async fn seed_connected(pool: &SqlitePool, access_token: &str) {
        seed_creds(pool).await;
        SettingsRepo::set(pool, "strava_refresh_token", "REF")
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_access_token", access_token)
            .await
            .unwrap();
        SettingsRepo::set(
            pool,
            "strava_expires_at",
            &(now_epoch() + 6 * 3600).to_string(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn api_get_sends_the_bearer_token() {
        let pool = setup_pool().await;
        seed_connected(&pool, "VALID").await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(header("authorization", "Bearer VALID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activities": [{ "id": 1, "name": "Morning Run" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let body = strava
            .api_get(
                &pool,
                "/athlete/activities",
                &[("per_page", "10".to_string())],
            )
            .await
            .unwrap();

        assert_eq!(body["activities"][0]["id"], 1);
    }

    #[tokio::test]
    async fn api_get_retries_once_on_401_after_refreshing() {
        let pool = setup_pool().await;
        seed_connected(&pool, "STALE").await;

        let server = MockServer::start().await;
        // Primer intento con el token viejo: 401.
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(header("authorization", "Bearer STALE"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        // Reintento con el token refrescado: 200.
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(header("authorization", "Bearer FRESH"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_type": "Bearer",
                "access_token": "FRESH",
                "refresh_token": "NEWREF",
                "expires_at": now_epoch() + 21600,
                "expires_in": 21600
            })))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let body = strava
            .api_get(
                &pool,
                "/athlete/activities",
                &[("per_page", "10".to_string())],
            )
            .await
            .unwrap();

        assert_eq!(body["ok"], true);
        assert_eq!(get(&pool, "strava_access_token").await, "FRESH");
        assert_eq!(get(&pool, "strava_refresh_token").await, "NEWREF");
    }

    #[tokio::test]
    async fn api_get_maps_429_to_rate_limited() {
        let pool = setup_pool().await;
        seed_connected(&pool, "VALID").await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .respond_with(ResponseTemplate::new(429))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let err = strava
            .api_get(
                &pool,
                "/athlete/activities",
                &[("per_page", "10".to_string())],
            )
            .await
            .unwrap_err();

        assert!(matches!(err, StravaError::RateLimited));
    }

    #[tokio::test]
    async fn api_get_serves_repeats_from_the_short_cache() {
        let pool = setup_pool().await;
        seed_connected(&pool, "VALID").await;

        let server = MockServer::start().await;
        // Solo una petición de red: la segunda se sirve de la caché.
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(header("authorization", "Bearer VALID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "n": 1
            })))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let query = [("per_page", "10".to_string())];
        let first = strava
            .api_get(&pool, "/athlete/activities", &query)
            .await
            .unwrap();
        let second = strava
            .api_get(&pool, "/athlete/activities", &query)
            .await
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(second["n"], 1);
    }

    #[test]
    fn cache_put_purges_expired_entries() {
        let base = Instant::now();
        let ttl = Duration::from_secs(CACHE_TTL_SECS);
        let mut cache: HashMap<String, (Instant, serde_json::Value)> = HashMap::new();
        cache.insert(
            "stale".to_string(),
            (base, serde_json::json!({ "stale": true })),
        );
        cache.insert(
            "fresh".to_string(),
            (base + ttl, serde_json::json!({ "fresh": true })),
        );

        // Un `now` justo por encima del TTL: "stale" caduca y "fresh" aún no.
        cache_put_in(
            &mut cache,
            "new",
            &serde_json::json!({ "new": true }),
            base + ttl + Duration::from_secs(1),
        );

        assert!(
            cache.contains_key("new"),
            "la entrada nueva debe insertarse"
        );
        assert!(
            cache.contains_key("fresh"),
            "la entrada vigente debe permanecer"
        );
        assert!(
            !cache.contains_key("stale"),
            "cache_put debe purgar las entradas caducadas"
        );
    }
}
