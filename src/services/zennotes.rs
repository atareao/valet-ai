//! Cliente de la API HTTP de `ZenNotes`.
//!
//! Encapsula la resolución de la configuración (ajustes → entorno → valor por
//! defecto para la URL y la carpeta) y las operaciones que consumirá la
//! herramienta `zennotes`. El token viaja en la cabecera
//! `Authorization: Bearer <token>` y **nunca** debe aparecer ni en un `Debug`,
//! ni en un log, ni en el mensaje de un [`ZennotesError`].
//!
//! Es el molde de [`crate::services::miniflux`]: misma resolución de credenciales
//! en cada llamada y misma traducción de errores.

use crate::db::repos::settings::SettingsRepo;
use sqlx::SqlitePool;

/// URL base por defecto de ZenNotes cuando ni el ajuste ni la variable de
/// entorno la definen.
pub const DEFAULT_BASE_URL: &str = "https://zennotes.territoriolinux.es";

/// Subcarpeta dedicada por defecto para las notas que crea Valet.
///
/// En la API del servidor `folder` es un enum restringido (`inbox`/`quick`/
/// `archive`); `Valet` es la **subcarpeta** por defecto dentro de `inbox`, de
/// modo que la nota nace en `inbox/Valet/`.
pub const DEFAULT_FOLDER: &str = "Valet";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Errores del cliente de ZenNotes.
///
/// [`ZennotesError::MissingCredentials`] (no hay token configurado) es una
/// variante **distinta** de [`ZennotesError::Unauthorized`] (el token no vale):
/// la primera indica dónde configurarlo y la segunda que ha caducado o es
/// incorrecto.
#[derive(Debug)]
pub enum ZennotesError {
    /// No hay token ni en `settings` ni en el entorno.
    MissingCredentials,
    /// `401`/`403` — el token no es válido, ha caducado o no tiene permiso.
    Unauthorized,
    /// `404` — la nota (o el recurso) no existe.
    NotFound,
    /// Fallo de red o `5xx` — ZenNotes no está disponible.
    Unavailable,
    /// Cualquier otro estado o una respuesta que no sea JSON interpretable.
    /// El mensaje **nunca** contiene el token.
    Http(String),
}

impl std::fmt::Display for ZennotesError {
    /// Mensajes accionables en español. **Ninguno** incluye el token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZennotesError::MissingCredentials => write!(
                f,
                "no hay token de ZenNotes configurado: defínelo en Ajustes → API Keys o en ZENNOTES_TOKEN"
            ),
            ZennotesError::Unauthorized => {
                write!(f, "el token de ZenNotes no es válido o ha caducado")
            }
            ZennotesError::NotFound => write!(f, "la nota solicitada no existe en ZenNotes"),
            ZennotesError::Unavailable => {
                write!(f, "ZenNotes no está disponible (fallo de red o error del servidor)")
            }
            ZennotesError::Http(detail) => {
                write!(f, "ZenNotes devolvió un error inesperado: {detail}")
            }
        }
    }
}

/// Configuración resuelta de ZenNotes.
#[derive(Clone, PartialEq)]
pub struct ZennotesConfig {
    pub base_url: String,
    pub token: String,
    pub folder: String,
}

impl std::fmt::Debug for ZennotesConfig {
    /// Redacta el token: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZennotesConfig")
            .field("base_url", &self.base_url)
            .field("token", &"***")
            .field("folder", &self.folder)
            .finish()
    }
}

/// Cómo aplicar un reemplazo literal: solo la primera coincidencia o todas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Occurrence {
    /// Sustituye solo la primera coincidencia (por defecto).
    #[default]
    First,
    /// Sustituye todas las coincidencias.
    All,
}

/// Cliente de ZenNotes atado a una URL base.
#[derive(Debug, Clone)]
pub struct Zennotes {
    http: reqwest::Client,
    base_url: String,
}

impl Zennotes {
    /// Cliente con la base dada, `user_agent` propio y timeout de 30 s.
    ///
    /// La base es **inyectable** para poder apuntar los tests a un servidor de
    /// prueba con `wiremock`.
    pub fn new(base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build reqwest Client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// URL base resuelta (settings → entorno → [`DEFAULT_BASE_URL`]). Nunca
    /// falla: un error de lectura de settings se trata como ausencia.
    pub async fn resolve_base_url(pool: &SqlitePool) -> String {
        setting_or_env(pool, "zennotes_base_url", "ZENNOTES_BASE_URL")
            .await
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
    }

    /// Carpeta por defecto resuelta (settings → entorno → [`DEFAULT_FOLDER`]).
    pub async fn resolve_folder(pool: &SqlitePool) -> String {
        setting_or_env(pool, "zennotes_folder", "ZENNOTES_FOLDER")
            .await
            .unwrap_or_else(|| DEFAULT_FOLDER.to_string())
    }

    /// Token resuelto (settings → entorno), o `None` si no hay ninguno.
    pub async fn resolve_token(pool: &SqlitePool) -> Option<String> {
        setting_or_env(pool, "zennotes_token", "ZENNOTES_TOKEN").await
    }

    /// Resuelve la configuración: primero `settings` (`zennotes_base_url` /
    /// `zennotes_token` / `zennotes_folder`) si el valor no está vacío tras
    /// `trim`, después las variables de entorno `ZENNOTES_BASE_URL` /
    /// `ZENNOTES_TOKEN` / `ZENNOTES_FOLDER` y, para la URL y la carpeta, el valor
    /// por defecto. Sin token devuelve [`ZennotesError::MissingCredentials`]. Las
    /// claves se leen en **cada llamada**, de modo que editarlas surte efecto sin
    /// reiniciar. Un error de lectura de settings se trata como ausencia.
    pub async fn resolve_config(pool: &SqlitePool) -> Result<ZennotesConfig, ZennotesError> {
        let base_url = Self::resolve_base_url(pool).await;
        let folder = Self::resolve_folder(pool).await;

        match Self::resolve_token(pool).await {
            Some(token) => Ok(ZennotesConfig {
                base_url,
                token,
                folder,
            }),
            None => Err(ZennotesError::MissingCredentials),
        }
    }

    /// `GET /api/vault` → `{"root":…,"name":…}`.
    pub async fn vault(&self, token: &str) -> Result<serde_json::Value, ZennotesError> {
        require_token(token)?;
        let url = format!("{}/api/vault", self.base_url);
        let response = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;
        decode_json(response).await
    }

    /// `GET /api/notes` → array crudo de metadatos de todas las notas.
    ///
    /// En un vault grande la respuesta es voluminosa (6.273 notas / ~3,3 MB): el
    /// recorte lo aplica la herramienta, no este método.
    pub async fn list_notes(&self, token: &str) -> Result<serde_json::Value, ZennotesError> {
        require_token(token)?;
        let url = format!("{}/api/notes", self.base_url);
        let response = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;
        decode_json(response).await
    }

    /// `GET /api/notes/read?path=<rel>` → la nota con su `body` (markdown).
    pub async fn read_note(
        &self,
        token: &str,
        path: &str,
    ) -> Result<serde_json::Value, ZennotesError> {
        require_token(token)?;

        // El `Serializer` se libera dentro del bloque: ningún valor no-`Send`
        // cruza el `.await` y el futuro sigue siendo `Send`.
        let url = {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("path", path)
                .finish();
            format!("{}/api/notes/read?{query}", self.base_url)
        };

        let response = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;
        decode_json(response).await
    }

    /// `GET /api/search/text?q=<q>&backend=auto` → array de coincidencias
    /// (`{path,title,folder,lineNumber,offset,lineText}`).
    pub async fn search_text(
        &self,
        token: &str,
        q: &str,
    ) -> Result<serde_json::Value, ZennotesError> {
        require_token(token)?;

        let url = {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("q", q)
                .append_pair("backend", "auto")
                .finish();
            format!("{}/api/search/text?{query}", self.base_url)
        };

        let response = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;
        decode_json(response).await
    }

    /// `POST /api/notes/create` `{folder,title,subpath}` — **sin** body. Devuelve
    /// la nota creada; usa su `path` para un [`Zennotes::write`] posterior.
    pub async fn create(
        &self,
        token: &str,
        folder: &str,
        title: &str,
        subpath: Option<&str>,
    ) -> Result<serde_json::Value, ZennotesError> {
        require_token(token)?;
        let body = serde_json::json!({
            "folder": folder,
            "title": title,
            "subpath": subpath.unwrap_or(""),
        });
        let response = self
            .http
            .post(format!("{}/api/notes/create", self.base_url))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;
        decode_json(response).await
    }

    /// Crea una nota con cuerpo: **create + write**, dos viajes, usando el `path`
    /// que devuelve `create`. Devuelve la nota creada.
    pub async fn create_with_body(
        &self,
        token: &str,
        folder: &str,
        title: &str,
        subpath: Option<&str>,
        body: &str,
    ) -> Result<serde_json::Value, ZennotesError> {
        let created = self.create(token, folder, title, subpath).await?;
        let path = created
            .get("path")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                ZennotesError::Http("la respuesta de create no incluye `path`".to_string())
            })?;
        self.write(token, path, body).await?;
        Ok(created)
    }

    /// `POST /api/notes/write` `{path,body}` — sobrescribe el cuerpo entero.
    pub async fn write(&self, token: &str, path: &str, body: &str) -> Result<(), ZennotesError> {
        require_token(token)?;
        let payload = serde_json::json!({ "path": path, "body": body });
        self.post_unit(token, "/api/notes/write", &payload).await
    }

    /// Añade `text` al final del cuerpo de la nota (read → concat → write). Se
    /// separa con un salto de línea cuando el cuerpo no está vacío y no termina
    /// ya en uno.
    pub async fn append(&self, token: &str, path: &str, text: &str) -> Result<(), ZennotesError> {
        let body = self.read_body(token, path).await?;
        let new_body = if body.is_empty() || body.ends_with('\n') {
            format!("{body}{text}")
        } else {
            format!("{body}\n{text}")
        };
        self.write(token, path, &new_body).await
    }

    /// Reemplaza literalmente `find` por `replace` en el cuerpo de la nota
    /// (read → sustituir → write). Con [`Occurrence::First`] solo la primera
    /// coincidencia; con [`Occurrence::All`], todas.
    pub async fn replace(
        &self,
        token: &str,
        path: &str,
        find: &str,
        replace: &str,
        occurrence: Occurrence,
    ) -> Result<(), ZennotesError> {
        let body = self.read_body(token, path).await?;
        let new_body = match occurrence {
            Occurrence::First => body.replacen(find, replace, 1),
            Occurrence::All => body.replace(find, replace),
        };
        self.write(token, path, &new_body).await
    }

    /// `POST /api/notes/move` `{path,targetFolder,targetSubpath}`.
    pub async fn move_note(
        &self,
        token: &str,
        path: &str,
        target_folder: &str,
        target_subpath: Option<&str>,
    ) -> Result<(), ZennotesError> {
        require_token(token)?;
        let payload = serde_json::json!({
            "path": path,
            "targetFolder": target_folder,
            "targetSubpath": target_subpath.unwrap_or(""),
        });
        self.post_unit(token, "/api/notes/move", &payload).await
    }

    /// `POST /api/notes/archive` `{path}`.
    pub async fn archive(&self, token: &str, path: &str) -> Result<(), ZennotesError> {
        self.path_action(token, path, "/api/notes/archive").await
    }

    /// `POST /api/notes/unarchive` `{path}`.
    pub async fn unarchive(&self, token: &str, path: &str) -> Result<(), ZennotesError> {
        self.path_action(token, path, "/api/notes/unarchive").await
    }

    /// `POST /api/notes/rename` `{path,title}`.
    pub async fn rename(&self, token: &str, path: &str, title: &str) -> Result<(), ZennotesError> {
        require_token(token)?;
        let payload = serde_json::json!({ "path": path, "title": title });
        self.post_unit(token, "/api/notes/rename", &payload).await
    }

    /// `POST /api/notes/trash` `{path}`.
    pub async fn trash(&self, token: &str, path: &str) -> Result<(), ZennotesError> {
        self.path_action(token, path, "/api/notes/trash").await
    }

    /// `POST /api/notes/restore` `{path}`.
    pub async fn restore(&self, token: &str, path: &str) -> Result<(), ZennotesError> {
        self.path_action(token, path, "/api/notes/restore").await
    }

    /// `POST /api/notes/delete` `{path}` — borra definitivamente.
    pub async fn delete(&self, token: &str, path: &str) -> Result<(), ZennotesError> {
        self.path_action(token, path, "/api/notes/delete").await
    }

    /// `POST` de una acción que solo lleva `{path}`.
    async fn path_action(
        &self,
        token: &str,
        path: &str,
        endpoint: &str,
    ) -> Result<(), ZennotesError> {
        require_token(token)?;
        let payload = serde_json::json!({ "path": path });
        self.post_unit(token, endpoint, &payload).await
    }

    /// `POST` a `endpoint` con `payload`; `Ok(())` si el estado es de éxito.
    async fn post_unit(
        &self,
        token: &str,
        endpoint: &str,
        payload: &serde_json::Value,
    ) -> Result<(), ZennotesError> {
        let response = self
            .http
            .post(format!("{}{endpoint}", self.base_url))
            .bearer_auth(token)
            .json(payload)
            .send()
            .await
            .map_err(|_| ZennotesError::Unavailable)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(error_from_status(status.as_u16()))
        }
    }

    /// Lee una nota y devuelve su `body` (cadena vacía si falta).
    async fn read_body(&self, token: &str, path: &str) -> Result<String, ZennotesError> {
        let note = self.read_note(token, path).await?;
        Ok(note
            .get("body")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string())
    }
}

/// Rechaza un token vacío o en blanco **antes** de tocar la red.
fn require_token(token: &str) -> Result<(), ZennotesError> {
    if token.trim().is_empty() {
        Err(ZennotesError::MissingCredentials)
    } else {
        Ok(())
    }
}

/// Lee una clave de `settings`; si está vacía o ilegible, cae a la variable de
/// entorno. Un error de la base de datos se trata como ausencia y se registra
/// con `tracing::warn!`, nunca como fallo.
async fn setting_or_env(pool: &SqlitePool, key: &str, env: &str) -> Option<String> {
    match SettingsRepo::get(pool, key).await {
        Ok(Some(value)) => {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                setting = key,
                "no se pudo leer el ajuste de zennotes: {error}"
            );
        }
    }

    std::env::var(env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Parsea el JSON de una respuesta correcta o traduce el error de ZenNotes.
///
/// Una respuesta correcta que no sea JSON interpretable se traduce a
/// [`ZennotesError::Http`]; el texto del error de `reqwest` no incluye el token
/// (viaja en una cabecera, no en la URL).
async fn decode_json(response: reqwest::Response) -> Result<serde_json::Value, ZennotesError> {
    let status = response.status();
    if status.is_success() {
        return response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ZennotesError::Http(error.to_string()));
    }

    Err(error_from_status(status.as_u16()))
}

/// Traduce un estado HTTP de error de ZenNotes a su [`ZennotesError`].
fn error_from_status(status: u16) -> ZennotesError {
    match status {
        401 | 403 => ZennotesError::Unauthorized,
        404 => ZennotesError::NotFound,
        500..=599 => ZennotesError::Unavailable,
        _ => ZennotesError::Http(format!("zennotes respondió {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    fn set_env(key: &str, value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    // -----------------------------------------------------------------------
    // resolve_config — precedencia settings > entorno > valor por defecto
    // -----------------------------------------------------------------------

    /// Scenario: El ajuste manda sobre el entorno (URL, token y carpeta).
    #[tokio::test]
    #[serial]
    async fn resolve_config_setting_wins_over_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "zennotes_base_url", "https://from-settings.test")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "zennotes_token", "setting-token")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "zennotes_folder", "FromSettings")
            .await
            .unwrap();
        set_env("ZENNOTES_BASE_URL", Some("https://from-env.test"));
        set_env("ZENNOTES_TOKEN", Some("env-token"));
        set_env("ZENNOTES_FOLDER", Some("FromEnv"));

        let config = Zennotes::resolve_config(&pool).await.unwrap();

        set_env("ZENNOTES_BASE_URL", None);
        set_env("ZENNOTES_TOKEN", None);
        set_env("ZENNOTES_FOLDER", None);

        assert_eq!(config.base_url, "https://from-settings.test");
        assert_eq!(config.token, "setting-token");
        assert_eq!(config.folder, "FromSettings");
    }

    /// Scenario: Sin token ni en settings ni en el entorno → `MissingCredentials`.
    #[tokio::test]
    #[serial]
    async fn resolve_config_missing_token_is_missing_credentials() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "zennotes_token", "")
            .await
            .unwrap();
        set_env("ZENNOTES_TOKEN", None);

        let err = Zennotes::resolve_config(&pool).await.unwrap_err();

        assert!(
            matches!(err, ZennotesError::MissingCredentials),
            "err: {err:?}"
        );
    }

    /// Scenario: sin nada configurado la URL y la carpeta caen a sus defaults.
    #[tokio::test]
    #[serial]
    async fn resolve_config_defaults_base_url_and_folder() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "zennotes_base_url", "")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "zennotes_folder", "")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "zennotes_token", "only-token")
            .await
            .unwrap();
        set_env("ZENNOTES_BASE_URL", None);
        set_env("ZENNOTES_FOLDER", None);

        let config = Zennotes::resolve_config(&pool).await.unwrap();

        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.folder, "Valet");
        assert_eq!(config.token, "only-token");
    }

    // -----------------------------------------------------------------------
    // El token nunca aparece en un Debug ni en un mensaje de error
    // -----------------------------------------------------------------------

    /// El `Debug` de la config redacta el token.
    #[test]
    fn config_debug_redacts_the_token() {
        let config = ZennotesConfig {
            base_url: "https://example.test".to_string(),
            token: "super-secret-token".to_string(),
            folder: "Valet".to_string(),
        };

        let debug = format!("{config:?}");
        assert!(
            !debug.contains("super-secret-token"),
            "el Debug filtra el token: {debug}"
        );
        assert!(
            debug.contains("***"),
            "el Debug debe redactar el token: {debug}"
        );
    }

    /// Un error de respuesta cuyo cuerpo **sí** contiene el token no lo filtra ni
    /// por `Debug` ni por `Display`.
    #[tokio::test]
    async fn error_messages_never_contain_the_token() {
        const TOKEN: &str = "super-secret-token";
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes/read"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(format!("unauthorized: {TOKEN}")),
            )
            .mount(&server)
            .await;

        let client = Zennotes::new(server.uri());
        let err = client.read_note(TOKEN, "Valet/X.md").await.unwrap_err();

        assert!(matches!(err, ZennotesError::Unauthorized), "err: {err:?}");
        assert!(
            !format!("{err:?}").contains(TOKEN),
            "el Debug filtra el token"
        );
        assert!(
            !err.to_string().contains(TOKEN),
            "el Display filtra el token"
        );
    }

    // -----------------------------------------------------------------------
    // Traducción de errores
    // -----------------------------------------------------------------------

    /// Scenario: `401`/`403` → `Unauthorized`, `404` → `NotFound`, `5xx` → `Unavailable`.
    #[tokio::test]
    async fn read_note_maps_error_statuses() {
        for (status, expected) in [
            (401u16, StatusKind::Unauthorized),
            (403, StatusKind::Unauthorized),
            (404, StatusKind::NotFound),
            (500, StatusKind::Unavailable),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/notes/read"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let client = Zennotes::new(server.uri());
            let err = client.read_note("t", "Valet/X.md").await.unwrap_err();
            assert_eq!(StatusKind::of(&err), expected, "status {status}: {err:?}");
        }
    }

    #[derive(Debug, PartialEq)]
    enum StatusKind {
        Unauthorized,
        NotFound,
        Unavailable,
    }

    impl StatusKind {
        fn of(err: &ZennotesError) -> Self {
            match err {
                ZennotesError::Unauthorized => StatusKind::Unauthorized,
                ZennotesError::NotFound => StatusKind::NotFound,
                ZennotesError::Unavailable => StatusKind::Unavailable,
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    // -----------------------------------------------------------------------
    // El token viaja en `Authorization: Bearer`
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn token_travels_in_authorization_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/vault"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "root": "/workspace", "name": "workspace"
            })))
            .mount(&server)
            .await;

        let client = Zennotes::new(server.uri());
        let value = client.vault("test-token").await.unwrap();
        assert_eq!(value["name"], "workspace");
    }

    /// Sin token (vacío o en blanco) no se toca la red.
    #[tokio::test]
    async fn missing_token_sends_no_request() {
        let server = MockServer::start().await;
        let client = Zennotes::new(server.uri());

        let err = client.vault("").await.unwrap_err();

        assert!(
            matches!(err, ZennotesError::MissingCredentials),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin token no debe enviar ninguna petición"
        );
    }

    // -----------------------------------------------------------------------
    // create + write = dos viajes; append/replace = read + write
    // -----------------------------------------------------------------------

    /// Scenario: `create_with_body` hace dos viajes: create y, con el `path` de
    /// la respuesta, write.
    #[tokio::test]
    async fn create_with_body_makes_two_trips() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(header("Authorization", "Bearer t"))
            .and(body_json(serde_json::json!({
                "folder": "Valet", "title": "X", "subpath": ""
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "path": "Valet/X.md", "title": "X"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/notes/write"))
            .and(header("Authorization", "Bearer t"))
            .and(body_json(serde_json::json!({
                "path": "Valet/X.md", "body": "hola"
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Zennotes::new(server.uri());
        let created = client
            .create_with_body("t", "Valet", "X", None, "hola")
            .await
            .unwrap();

        assert_eq!(created["path"], "Valet/X.md");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2, "create + write son dos viajes");
        let paths: Vec<String> = requests.iter().map(|r| r.url.path().to_string()).collect();
        assert!(paths.contains(&"/api/notes/create".to_string()));
        assert!(paths.contains(&"/api/notes/write".to_string()));
    }

    /// Scenario: `append` lee la nota, añade al final y escribe.
    #[tokio::test]
    async fn append_reads_then_writes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes/read"))
            .and(query_param("path", "Valet/X.md"))
            .and(header("Authorization", "Bearer t"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "path": "Valet/X.md", "body": "old"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/notes/write"))
            .and(body_json(serde_json::json!({
                "path": "Valet/X.md", "body": "old\nhola"
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Zennotes::new(server.uri());
        client.append("t", "Valet/X.md", "hola").await.unwrap();

        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    /// Scenario: `replace` lee la nota, sustituye literal y escribe.
    #[tokio::test]
    async fn replace_reads_then_writes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes/read"))
            .and(query_param("path", "Valet/X.md"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "path": "Valet/X.md", "body": "uno dos uno"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/notes/write"))
            .and(body_json(serde_json::json!({
                "path": "Valet/X.md", "body": "UNO dos UNO"
            })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Zennotes::new(server.uri());
        client
            .replace("t", "Valet/X.md", "uno", "UNO", Occurrence::All)
            .await
            .unwrap();

        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    /// Aserción de compilación: los futuros públicos deben ser `Send`.
    #[test]
    fn public_futures_are_send() {
        fn assert_send<T: Send>(_: T) {}
        let client = Zennotes::new("http://localhost:1".to_string());
        assert_send(client.vault("k"));
        assert_send(client.list_notes("k"));
        assert_send(client.read_note("k", "p"));
        assert_send(client.search_text("k", "q"));
        assert_send(client.create("k", "Valet", "T", None));
        assert_send(client.create_with_body("k", "Valet", "T", None, "b"));
        assert_send(client.write("k", "p", "b"));
        assert_send(client.append("k", "p", "t"));
        assert_send(client.replace("k", "p", "f", "r", Occurrence::First));
        assert_send(client.move_note("k", "p", "archive", None));
        assert_send(client.archive("k", "p"));
        assert_send(client.unarchive("k", "p"));
        assert_send(client.rename("k", "p", "T"));
        assert_send(client.trash("k", "p"));
        assert_send(client.restore("k", "p"));
        assert_send(client.delete("k", "p"));
    }
}
