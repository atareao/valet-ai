//! Cliente HTTP de la **Agent API** de `OxiMeal`.
//!
//! OxiMeal es el gestor *self-hosted* de recetas, menús semanales y listas de la
//! compra. Su Agent API (`/api/agent/*`) exige `Authorization: Bearer` con un
//! token obtenido por OIDC **M2M** (`grant_type=client_credentials`) contra el
//! mismo IdP que ya usa Valet (`AUTH_ISSUER_URL`).
//!
//! Este módulo encapsula:
//! - la resolución de la configuración (ajustes → entorno → valor por defecto
//!   solo para la URL) **en cada llamada**, de modo que editar los ajustes surta
//!   efecto sin reiniciar;
//! - la obtención y la caché **de proceso** del token M2M, con margen de
//!   seguridad de 30 s antes de su expiración y descubrimiento cacheado del
//!   `token_endpoint`;
//! - las seis operaciones de la Agent API, con DTOs `serde` tipados.
//!
//! El `client_secret` viaja en el cuerpo de la petición de token y **nunca**
//! debe aparecer ni en un `Debug`, ni en un log, ni en el mensaje de un
//! [`OximealError`]. Es el molde de [`crate::services::opentodo`] y
//! [`crate::services::zennotes`]: misma resolución de credenciales y misma
//! traducción de errores.

use crate::db::repos::settings::SettingsRepo;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

/// URL base por defecto de OxiMeal cuando ni el ajuste ni la variable de
/// entorno la definen.
pub const DEFAULT_BASE_URL: &str = "https://oximeal.territoriolinux.es";

/// Permisos (`scope`) que se piden en la petición `client_credentials`.
pub const DEFAULT_SCOPES: &str = "agent:read agent:plan agent:shopping agent:write";

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
///
/// Forma parte del contrato del cliente, por eso es público: un cambio aquí
/// tiene que ser deliberado.
pub const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Margen de seguridad: un token al que le falten menos segundos para expirar se
/// considera caducado y se renueva antes de usarlo.
const TOKEN_EXPIRY_MARGIN_SECS: u64 = 30;

/// Número máximo de recetas que devuelve [`Oximeal::search_recipes`] (el
/// servidor ya acota, pero el cliente no confía en ello).
const MAX_SEARCH_RESULTS: usize = 10;

/// Errores del cliente de OxiMeal.
///
/// [`OximealError::NotConfigured`] (faltan credenciales) es una variante
/// **distinta** de [`OximealError::Unauthorized`] (el token no vale): la primera
/// indica dónde configurarlo y la segunda que se ha rechazado.
#[derive(Debug)]
pub enum OximealError {
    /// Faltan `client_id` o `client_secret` (ni en `settings` ni en el entorno).
    NotConfigured,
    /// `401`/`403` — el token no es válido, ha caducado o no tiene permiso.
    Unauthorized,
    /// `404` — el recurso no existe.
    NotFound,
    /// `400` — el servidor rechazó la petición; el mensaje es el suyo (ya
    /// redactado, nunca contiene credenciales).
    InvalidInput(String),
    /// Fallo en la **fase de token**: el IdP no responde al descubrimiento OIDC
    /// o rechaza la petición `client_credentials`. Se distingue de
    /// [`OximealError::Upstream`] para poder separar «el IdP no entregó token»
    /// de «OxiMeal no responde» sin adivinar por el texto.
    TokenEndpoint(String),
    /// Fallo de red o `5xx` de **OxiMeal** (la Agent API), o un fallo al
    /// construir el cliente HTTP. Un problema del IdP cae en
    /// [`OximealError::TokenEndpoint`].
    Upstream(String),
}

impl std::fmt::Display for OximealError {
    /// Mensajes accionables en español. **Ninguno** incluye credenciales.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OximealError::NotConfigured => write!(
                f,
                "OxiMeal no está configurado: define el client_id y el client_secret en Ajustes → Integraciones → OxiMeal o en OXIMEAL_CLIENT_ID y OXIMEAL_CLIENT_SECRET"
            ),
            OximealError::Unauthorized => {
                write!(f, "OxiMeal rechazó el token: revisa las credenciales M2M")
            }
            OximealError::NotFound => write!(f, "el recurso solicitado no existe en OxiMeal"),
            OximealError::InvalidInput(detail) => {
                write!(f, "OxiMeal rechazó la petición: {detail}")
            }
            OximealError::TokenEndpoint(detail) => {
                write!(f, "el IdP no entregó un token M2M: {detail}")
            }
            OximealError::Upstream(detail) => {
                write!(f, "OxiMeal no está disponible: {detail}")
            }
        }
    }
}

impl std::error::Error for OximealError {}

/// Configuración resuelta de OxiMeal.
#[derive(Clone, PartialEq)]
pub struct OximealConfig {
    pub base_url: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
}

impl std::fmt::Debug for OximealConfig {
    /// Redacta el secreto: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OximealConfig")
            .field("base_url", &self.base_url)
            .field("issuer_url", &self.issuer_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"***")
            .finish()
    }
}

/// Receta planificada recientemente (`GET /api/agent/recent-recipes`).
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct RecentRecipe {
    pub id: String,
    pub name: String,
    pub category: String,
    pub last_planned: Option<String>,
}

/// Resumen de receta devuelto por la búsqueda (`GET /api/agent/search-recipes`).
///
/// Tolerante a campos extra: `#[serde(default)]` a nivel de estructura.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct RecipeSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub diet_type: Option<String>,
    pub prep_time_minutes: Option<i64>,
    pub servings: i64,
    pub tags: Vec<String>,
}

/// Entrada del plan semanal (`POST /api/agent/weekly-plan`).
///
/// `role` y `servings` son opcionales: el servidor aplica sus propios valores
/// por defecto si faltan, así que solo se serializan cuando están presentes.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanEntry {
    pub date: String,
    pub meal_type: String,
    pub recipe_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servings: Option<i64>,
}

/// Ingrediente que se añade a la lista de la compra
/// (`POST /api/agent/shopping-items`).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShoppingItemInput {
    pub name: String,
    pub amount: f64,
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Grupo de la lista de la compra (por categoría de ingrediente).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ShoppingListGroup {
    pub category: String,
    /// Los ítems se dejan como JSON crudo: al cliente solo le interesa
    /// reconstruir la lista, no su forma exacta.
    pub items: Vec<serde_json::Value>,
}

/// Resultado de [`Oximeal::set_weekly_plan`].
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct WeeklyPlanResult {
    pub created: i64,
    pub shopping_list: Vec<ShoppingListGroup>,
}

/// Caché de proceso del token M2M y del `token_endpoint`.
#[derive(Default)]
struct TokenCache {
    /// Emisor al que pertenecen las entradas cacheadas.
    issuer: String,
    /// `client_id` con el que se obtuvo el token cacheado. Forma parte de la
    /// clave: cambiar el `client_id` invalida el token aunque el emisor siga
    /// siendo el mismo.
    client_id: String,
    /// `token_endpoint` descubierto por OIDC.
    token_endpoint: String,
    /// Token de acceso vigente.
    access_token: String,
    /// Instante en que expira el token.
    expires_at: Option<Instant>,
}

/// Caché **compartida por todo el proceso**: el servicio se construye en cada
/// llamada, así que el estado no puede vivir en la instancia.
static TOKEN_CACHE: OnceLock<StdMutex<TokenCache>> = OnceLock::new();

/// Devuelve el mutex de la caché, inicializándolo la primera vez.
fn token_cache() -> &'static StdMutex<TokenCache> {
    TOKEN_CACHE.get_or_init(|| StdMutex::new(TokenCache::default()))
}

/// Toma el candado de la caché tolerando un mutex envenenado.
///
/// El candado **nunca** se mantiene a través de un `await` (todas las lecturas
/// y escrituras son síncronas y cortas), por eso basta con un `Mutex` estándar.
fn lock_cache() -> std::sync::MutexGuard<'static, TokenCache> {
    token_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Deja la caché como recién arrancada. Solo para tests.
#[cfg(test)]
fn clear_token_cache() {
    *lock_cache() = TokenCache::default();
}

/// Candados *single-flight* por emisor.
///
/// El candado síncrono de la caché no sirve para esto: el refresco hace `await`
/// (descubrimiento y petición de token), y mantenerlo a través de un `await`
/// bloquearía el runtime. Con un candado asíncrono por emisor, con la caché
/// fría solo un llamante lanza el refresco y los demás esperan aquí y releen la
/// caché (doble comprobación), de modo que el IdP recibe **una sola** petición
/// de token por emisor.
static REFRESH_LOCKS: OnceLock<StdMutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();

/// Devuelve (creándolo la primera vez) el candado de refresco del emisor.
fn refresh_lock(issuer: &str) -> Arc<tokio::sync::Mutex<()>> {
    let locks = REFRESH_LOCKS.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks
        .entry(issuer.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// Cliente de OxiMeal atado a una configuración.
#[derive(Debug, Clone)]
pub struct Oximeal {
    http: reqwest::Client,
    config: OximealConfig,
}

impl Oximeal {
    /// Cliente con la configuración dada, `user_agent` propio y timeout.
    ///
    /// La base es **inyectable** para poder apuntar los tests a un servidor de
    /// prueba con `wiremock`. La construcción es falible: si `reqwest` no puede
    /// inicializar su backend TLS se devuelve [`OximealError::Upstream`] en vez
    /// de caer a un cliente sin `user_agent` ni timeout.
    pub fn new(config: OximealConfig) -> Result<Self, OximealError> {
        let secret = config.client_secret.trim().to_string();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| {
                let detail = format!("no se pudo inicializar el cliente HTTP: {error}");
                let detail = if secret.is_empty() {
                    detail
                } else {
                    detail.replace(&secret, "***")
                };
                OximealError::Upstream(detail)
            })?;

        Ok(Self {
            http,
            config: OximealConfig {
                base_url: config.base_url.trim_end_matches('/').to_string(),
                issuer_url: config.issuer_url.trim_end_matches('/').to_string(),
                client_id: config.client_id.trim().to_string(),
                client_secret: secret,
            },
        })
    }

    /// URL base resuelta (settings → entorno → [`DEFAULT_BASE_URL`]). Nunca
    /// falla: un error de lectura de settings se trata como ausencia.
    pub async fn resolve_base_url(pool: &SqlitePool) -> String {
        setting_or_env(pool, "oximeal_base_url", "OXIMEAL_BASE_URL")
            .await
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
    }

    /// Emisor OIDC resuelto: `oximeal_issuer_url` (settings → entorno) y, si
    /// queda vacío, `AUTH_ISSUER_URL` de la configuración de Valet.
    ///
    /// [`crate::config::Config::from_env`] lee `AUTH_ISSUER_URL` del entorno sin
    /// transformarlo, así que aquí se lee la misma variable directamente para no
    /// reconstruir toda la `Config` en cada resolución.
    pub async fn resolve_issuer_url(pool: &SqlitePool) -> String {
        if let Some(value) = setting_or_env(pool, "oximeal_issuer_url", "OXIMEAL_ISSUER_URL").await
        {
            return value;
        }

        std::env::var("AUTH_ISSUER_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
    }

    /// `client_id` resuelto con la precedencia ajuste → entorno. Devuelve cadena
    /// vacía si no hay ninguno.
    ///
    /// Fuente única de esa precedencia: la usan [`Oximeal::resolve_config`] y el
    /// handler de estado, para no duplicar la lógica.
    pub async fn resolve_client_id(pool: &SqlitePool) -> String {
        setting_or_env(pool, "oximeal_client_id", "OXIMEAL_CLIENT_ID")
            .await
            .unwrap_or_default()
    }

    /// Resuelve la configuración completa. Faltando `client_id` o
    /// `client_secret` devuelve [`OximealError::NotConfigured`] **sin tocar la
    /// red**.
    pub async fn resolve_config(pool: &SqlitePool) -> Result<OximealConfig, OximealError> {
        let base_url = Self::resolve_base_url(pool).await;
        let issuer_url = Self::resolve_issuer_url(pool).await;
        let client_id = Self::resolve_client_id(pool).await;
        let client_secret = setting_or_env(pool, "oximeal_client_secret", "OXIMEAL_CLIENT_SECRET")
            .await
            .unwrap_or_default();

        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return Err(OximealError::NotConfigured);
        }

        Ok(OximealConfig {
            base_url,
            issuer_url,
            client_id,
            client_secret,
        })
    }

    /// Resuelve la configuración y construye el cliente.
    pub async fn from_settings(pool: &SqlitePool) -> Result<Self, OximealError> {
        let config = Self::resolve_config(pool).await?;
        Self::new(config)
    }

    /// URL base resuelta.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// `GET /api/agent/recent-recipes?days=` — recetas planificadas recientemente.
    ///
    /// `days == 0` se interpreta como el valor por defecto (7) y cualquier valor
    /// se acota a `1..=365`.
    pub async fn recent_recipes(&self, days: u32) -> Result<Vec<RecentRecipe>, OximealError> {
        let days = clamp_days(days);
        let value = self
            .send_authed(|token| {
                self.http
                    .get(self.url(&format!("/api/agent/recent-recipes?days={days}")))
                    .bearer_auth(token)
            })
            .await?;

        self.decode_list(&value, "recetas")
    }

    /// `GET /api/agent/search-recipes?query=&category=` — búsqueda de recetas.
    ///
    /// Devuelve como mucho [`MAX_SEARCH_RESULTS`] recetas, aunque el servidor ya
    /// acote.
    pub async fn search_recipes(
        &self,
        query: Option<&str>,
        category: Option<&str>,
    ) -> Result<Vec<RecipeSummary>, OximealError> {
        // La query se construye a mano: `reqwest` va sin la feature que habilita
        // `.query()` en este proyecto (mismo motivo por el que `zennotes` usa
        // `url::form_urlencoded`).
        let endpoint = {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
                serializer.append_pair("query", query);
            }
            if let Some(category) = category
                .map(str::trim)
                .filter(|category| !category.is_empty())
            {
                serializer.append_pair("category", category);
            }
            let query = serializer.finish();
            if query.is_empty() {
                "/api/agent/search-recipes".to_string()
            } else {
                format!("/api/agent/search-recipes?{query}")
            }
        };

        let value = self
            .send_authed(|token| self.http.get(self.url(&endpoint)).bearer_auth(token))
            .await?;

        let mut recipes: Vec<RecipeSummary> = self.decode_list(&value, "recetas")?;
        recipes.truncate(MAX_SEARCH_RESULTS);
        Ok(recipes)
    }

    /// `POST /api/agent/weekly-plan` — fija el plan semanal.
    pub async fn set_weekly_plan(
        &self,
        entries: &[PlanEntry],
    ) -> Result<WeeklyPlanResult, OximealError> {
        let body = serde_json::json!({ "entries": entries });
        let value = self
            .send_authed(|token| {
                self.http
                    .post(self.url("/api/agent/weekly-plan"))
                    .json(&body)
                    .bearer_auth(token)
            })
            .await?;

        serde_json::from_value(value)
            .map_err(|error| self.upstream(&format!("plan semanal ilegible: {error}")))
    }

    /// `POST /api/agent/shopping-items` — añade ingredientes a la compra y
    /// devuelve la lista reconstruida por grupos.
    pub async fn add_shopping_items(
        &self,
        items: &[ShoppingItemInput],
    ) -> Result<Vec<ShoppingListGroup>, OximealError> {
        let body = serde_json::json!({ "items": items });
        let value = self
            .send_authed(|token| {
                self.http
                    .post(self.url("/api/agent/shopping-items"))
                    .json(&body)
                    .bearer_auth(token)
            })
            .await?;

        serde_json::from_value(value)
            .map_err(|error| self.upstream(&format!("lista de la compra ilegible: {error}")))
    }

    /// `DELETE /api/agent/shopping/completed` — purga lo ya comprado y devuelve
    /// cuántos ítems se eliminaron.
    pub async fn delete_completed(&self) -> Result<u64, OximealError> {
        let value = self
            .send_authed(|token| {
                self.http
                    .delete(self.url("/api/agent/shopping/completed"))
                    .bearer_auth(token)
            })
            .await?;

        value
            .get("removed")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| self.upstream("la purga no devolvió el campo `removed`"))
    }

    /// `POST /api/agent/recipes` — crea una receta (pasa el `RecipeInput` tal
    /// cual) y devuelve la receta creada.
    pub async fn create_recipe(
        &self,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, OximealError> {
        self.send_authed(|token| {
            self.http
                .post(self.url("/api/agent/recipes"))
                .json(input)
                .bearer_auth(token)
        })
        .await
    }

    /// Sonda de conectividad para `GET /api/oximeal/check`: obtiene token y
    /// consulta las recetas de las últimas 24 h.
    pub async fn probe(&self) -> Result<(), OximealError> {
        let _ = self.recent_recipes(1).await?;
        Ok(())
    }

    // -- infraestructura HTTP ------------------------------------------------

    /// URL absoluta de un endpoint.
    fn url(&self, endpoint: &str) -> String {
        format!("{}{endpoint}", self.config.base_url)
    }

    /// Borra el secreto de cualquier texto que vaya a salir en un error.
    fn redact(&self, text: &str) -> String {
        let secret = self.config.client_secret.trim();
        if secret.is_empty() {
            text.to_string()
        } else {
            text.replace(secret, "***")
        }
    }

    /// Embudo único de los errores con **texto ajeno**: siempre pasa por
    /// [`Oximeal::redact`]. Obligatorio para cualquier detalle que venga del
    /// servidor o de una librería, porque el secreto viaja en el cuerpo de la
    /// petición de token.
    fn upstream(&self, detail: &str) -> OximealError {
        OximealError::Upstream(self.redact(detail))
    }

    /// Igual que [`Oximeal::upstream`] para los errores de validación que llevan
    /// un mensaje del servidor.
    fn invalid_input(&self, detail: &str) -> OximealError {
        OximealError::InvalidInput(self.redact(detail))
    }

    /// Embudo de los errores de la **fase de token** (descubrimiento OIDC y
    /// `client_credentials`): pasa por [`Oximeal::redact`] y usa la variante
    /// dedicada [`OximealError::TokenEndpoint`].
    fn token_endpoint_error(&self, detail: &str) -> OximealError {
        OximealError::TokenEndpoint(self.redact(detail))
    }

    /// Extrae el mensaje del servidor de un cuerpo de error, **sin** redactar.
    fn server_message(&self, body: &str) -> String {
        let detail = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|value| {
                ["error", "message", "detail", "title"]
                    .iter()
                    .find_map(|key| value.get(*key).and_then(|value| value.as_str()))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| body.trim().chars().take(300).collect::<String>());

        let detail = detail.trim();
        if detail.is_empty() {
            "sin detalle".to_string()
        } else {
            detail.to_string()
        }
    }

    /// Interpreta `{recipes:[…]}` (o un array pelado) como una lista de `T`.
    fn decode_list<T>(&self, value: &serde_json::Value, what: &str) -> Result<Vec<T>, OximealError>
    where
        T: serde::de::DeserializeOwned,
    {
        let raw = value
            .get("recipes")
            .cloned()
            .unwrap_or_else(|| value.clone());
        serde_json::from_value::<Vec<T>>(raw)
            .map_err(|error| self.upstream(&format!("lista de {what} ilegible: {error}")))
    }

    /// Devuelve un token utilizable, reutilizando la caché de proceso o
    /// solicitando uno nuevo.
    ///
    /// El descubrimiento del `token_endpoint` también se cachea (por emisor).
    async fn access_token(&self) -> Result<String, OximealError> {
        let issuer = self
            .config
            .issuer_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        if issuer.is_empty() {
            return Err(OximealError::NotConfigured);
        }
        let client_id = self.config.client_id.trim().to_string();

        if let Some(token) = cached_token(&issuer, &client_id) {
            return Ok(token);
        }

        // Single-flight por emisor: solo un llamante refresca; los demás esperan
        // aquí y, al entrar, releen la caché (doble comprobación).
        let refresh = refresh_lock(&issuer);
        let _guard = refresh.lock().await;

        if let Some(token) = cached_token(&issuer, &client_id) {
            return Ok(token);
        }

        let endpoint = match cached_endpoint(&issuer, &client_id) {
            Some(endpoint) => endpoint,
            None => {
                let endpoint = self.discover_token_endpoint(&issuer).await?;
                store_endpoint(&issuer, &client_id, &endpoint);
                endpoint
            }
        };

        let (token, expires_in) = self.fetch_token(&endpoint).await?;
        store_token(&issuer, &client_id, &token, expires_in);
        Ok(token)
    }

    /// Invalida la caché de proceso del token M2M (y el `token_endpoint`
    /// descubierto).
    ///
    /// Es pública a propósito: rotar el `client_secret`
    /// (`PUT /api/oximeal/secret`) tiene que impedir que se siga sirviendo un
    /// token obtenido con las credenciales viejas hasta que expire.
    pub fn invalidate_cache() {
        *lock_cache() = TokenCache::default();
    }

    /// Descubre el `token_endpoint` en el documento OIDC del emisor.
    async fn discover_token_endpoint(&self, issuer: &str) -> Result<String, OximealError> {
        let url = format!("{issuer}/.well-known/openid-configuration");
        let response =
            self.http.get(&url).send().await.map_err(|error| {
                self.token_endpoint_error(&format!("el IdP no responde: {error}"))
            })?;

        let status = response.status().as_u16();
        let body = response.text().await.map_err(|error| {
            self.token_endpoint_error(&format!("descubrimiento ilegible: {error}"))
        })?;

        if !(200..300).contains(&status) {
            return Err(self.token_endpoint_error(&format!(
                "el IdP respondió {status} al descubrimiento OIDC"
            )));
        }

        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("token_endpoint")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .filter(|endpoint| !endpoint.trim().is_empty())
            .ok_or_else(|| {
                self.token_endpoint_error("el descubrimiento del IdP no trae `token_endpoint`")
            })
    }

    /// Solicita un token por `client_credentials`. Devuelve el token y su vida
    /// útil en segundos.
    async fn fetch_token(&self, endpoint: &str) -> Result<(String, u64), OximealError> {
        // Cuerpo `application/x-www-form-urlencoded` construido a mano (reqwest
        // sin la feature que habilita `.form()`).
        let form = {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            serializer.append_pair("grant_type", "client_credentials");
            serializer.append_pair("client_id", self.config.client_id.trim());
            serializer.append_pair("client_secret", self.config.client_secret.trim());
            serializer.append_pair("scope", DEFAULT_SCOPES);
            serializer.finish()
        };

        let response = self
            .http
            .post(endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form)
            .send()
            .await
            .map_err(|error| {
                self.token_endpoint_error(&format!("no se pudo obtener el token: {error}"))
            })?;

        let status = response.status().as_u16();
        let body = response.text().await.map_err(|error| {
            self.token_endpoint_error(&format!("respuesta de token ilegible: {error}"))
        })?;

        if !(200..300).contains(&status) {
            return Err(self.token_endpoint_error(&format!(
                "el IdP rechazó la petición de token ({status}): {}",
                self.server_message(&body)
            )));
        }

        let value = serde_json::from_str::<serde_json::Value>(&body).map_err(|error| {
            self.token_endpoint_error(&format!("respuesta de token no JSON: {error}"))
        })?;
        let token = value
            .get("access_token")
            .and_then(|value| value.as_str())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| self.token_endpoint_error("el IdP no devolvió `access_token`"))?
            .to_string();

        // Si el IdP omite `expires_in` se trata como 0: el token se considerará
        // expirado y se pedirá uno nuevo en la siguiente llamada (opción
        // conservadora).
        let expires_in = value
            .get("expires_in")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        Ok((token, expires_in))
    }

    /// Envía una petición autenticada y normaliza la respuesta.
    ///
    /// Ante `401`/`403` invalida la caché, pide un token nuevo y reintenta
    /// **una** vez; si vuelve a fallar, devuelve [`OximealError::Unauthorized`].
    async fn send_authed<F>(&self, build: F) -> Result<serde_json::Value, OximealError>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        let token = self.access_token().await?;
        let response = build(&token)
            .send()
            .await
            .map_err(|error| self.upstream(&format!("fallo de red: {error}")))?;

        let status = response.status().as_u16();
        if status == 401 || status == 403 {
            invalidate_token();
            let retry_token = self.access_token().await?;
            let retry = build(&retry_token)
                .send()
                .await
                .map_err(|error| self.upstream(&format!("fallo de red: {error}")))?;
            return self.decode(retry).await;
        }

        self.decode(response).await
    }

    /// Lee el cuerpo y traduce el estado HTTP a un JSON o a un
    /// [`OximealError`].
    async fn decode(&self, response: reqwest::Response) -> Result<serde_json::Value, OximealError> {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| self.upstream(&format!("respuesta ilegible: {error}")))?;

        match status {
            200..=299 => {
                if body.trim().is_empty() {
                    Ok(serde_json::Value::Null)
                } else {
                    serde_json::from_str(&body)
                        .map_err(|error| self.upstream(&format!("respuesta no JSON: {error}")))
                }
            }
            400 => Err(self.invalid_input(&self.server_message(&body))),
            401 | 403 => Err(OximealError::Unauthorized),
            404 => Err(OximealError::NotFound),
            500..=599 => Err(self.upstream(&self.server_message(&body))),
            other => Err(self.upstream(&format!("OxiMeal respondió {other}"))),
        }
    }
}

/// Acota `days` a `1..=365`, con `0` interpretado como el valor por defecto (7).
fn clamp_days(days: u32) -> u32 {
    match days {
        0 => 7,
        days if days > 365 => 365,
        days => days,
    }
}

/// Token cacheado si sigue vigente para `issuer` (con margen de seguridad).
fn cached_token(issuer: &str, client_id: &str) -> Option<String> {
    let cache = lock_cache();
    if cache.issuer != issuer || cache.client_id != client_id || cache.access_token.is_empty() {
        return None;
    }
    let expires_at = cache.expires_at?;
    if expires_at > Instant::now() + Duration::from_secs(TOKEN_EXPIRY_MARGIN_SECS) {
        Some(cache.access_token.clone())
    } else {
        None
    }
}

/// `token_endpoint` cacheado para `issuer`.
fn cached_endpoint(issuer: &str, client_id: &str) -> Option<String> {
    let cache = lock_cache();
    if cache.issuer == issuer && cache.client_id == client_id && !cache.token_endpoint.is_empty() {
        Some(cache.token_endpoint.clone())
    } else {
        None
    }
}

/// Guarda el `token_endpoint`. Si cambia el emisor **o** el `client_id`, se
/// olvida también el token anterior (pertenecía a otras credenciales).
fn store_endpoint(issuer: &str, client_id: &str, endpoint: &str) {
    let mut cache = lock_cache();
    if cache.issuer != issuer || cache.client_id != client_id {
        cache.access_token.clear();
        cache.expires_at = None;
    }
    cache.issuer = issuer.to_string();
    cache.client_id = client_id.to_string();
    cache.token_endpoint = endpoint.to_string();
}

/// Guarda el token con su instante de expiración.
fn store_token(issuer: &str, client_id: &str, token: &str, expires_in: u64) {
    let mut cache = lock_cache();
    cache.issuer = issuer.to_string();
    cache.client_id = client_id.to_string();
    cache.access_token = token.to_string();
    cache.expires_at = Some(Instant::now() + Duration::from_secs(expires_in));
}

/// Invalida el token cacheado (conserva el `token_endpoint` descubierto).
fn invalidate_token() {
    let mut cache = lock_cache();
    cache.access_token.clear();
    cache.expires_at = None;
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
                "no se pudo leer el ajuste de oximeal: {error}"
            );
        }
    }

    std::env::var(env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{body_json, body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SECRET: &str = "SECRET";

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

    /// Cliente apuntado a un servidor de prueba; su emisor es el propio servidor.
    fn client_for(server: &MockServer) -> Oximeal {
        Oximeal::new(OximealConfig {
            base_url: server.uri(),
            issuer_url: server.uri(),
            client_id: "client-id".to_string(),
            client_secret: SECRET.to_string(),
        })
        .expect("cliente de test")
    }

    /// Monta el descubrimiento OIDC y el endpoint de token en el servidor.
    async fn mount_auth(server: &MockServer, token: &str, expires_in: u64) {
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .mount(server)
            .await;

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": token,
                "expires_in": expires_in,
            })))
            .mount(server)
            .await;
    }

    /// Cuántas peticiones ha recibido una ruta concreta.
    async fn count_path(server: &MockServer, target: &str) -> usize {
        server
            .received_requests()
            .await
            .expect("requests")
            .iter()
            .filter(|request| request.url.path() == target)
            .count()
    }

    // -----------------------------------------------------------------------
    // Configuración: settings → entorno → valor por defecto
    // -----------------------------------------------------------------------

    /// Scenario: el `Debug` de la configuración no contiene el secreto.
    #[test]
    fn config_debug_redacts_the_secret() {
        let config = OximealConfig {
            base_url: "https://example.test".to_string(),
            issuer_url: "https://idp.test".to_string(),
            client_id: "cid".to_string(),
            client_secret: SECRET.to_string(),
        };

        let debug = format!("{config:?}");
        assert!(
            !debug.contains(SECRET),
            "el Debug filtra el secreto: {debug}"
        );
        assert!(debug.contains("***"), "el Debug debe redactar: {debug}");
    }

    /// Scenario: sin credenciales no se toca la red.
    #[tokio::test]
    #[serial]
    async fn missing_credentials_is_not_configured_without_network() {
        let server = MockServer::start().await;
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "oximeal_base_url", &server.uri())
            .await
            .unwrap();
        SettingsRepo::set(&pool, "oximeal_client_id", "")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "oximeal_client_secret", "")
            .await
            .unwrap();
        set_env("OXIMEAL_CLIENT_ID", None);
        set_env("OXIMEAL_CLIENT_SECRET", None);

        let err = Oximeal::from_settings(&pool).await.unwrap_err();

        assert!(matches!(err, OximealError::NotConfigured), "err: {err:?}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin credenciales no debe enviar ninguna petición"
        );
    }

    /// Scenario: la URL base por defecto cuando no hay ajuste ni entorno.
    #[tokio::test]
    #[serial]
    async fn base_url_defaults_when_unset() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "oximeal_base_url", "")
            .await
            .unwrap();
        set_env("OXIMEAL_BASE_URL", None);

        assert_eq!(Oximeal::resolve_base_url(&pool).await, DEFAULT_BASE_URL);
    }

    /// El ajuste gana al entorno y este sirve de respaldo.
    #[tokio::test]
    #[serial]
    async fn setting_wins_over_env_for_base_url() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "oximeal_base_url", "https://from-settings.test")
            .await
            .unwrap();
        set_env("OXIMEAL_BASE_URL", Some("https://from-env.test"));

        let from_settings = Oximeal::resolve_base_url(&pool).await;

        SettingsRepo::set(&pool, "oximeal_base_url", "")
            .await
            .unwrap();
        let from_env = Oximeal::resolve_base_url(&pool).await;
        set_env("OXIMEAL_BASE_URL", None);

        assert_eq!(from_settings, "https://from-settings.test");
        assert_eq!(from_env, "https://from-env.test");
    }

    /// `resolve_client_id` resuelve con la precedencia ajuste → entorno y es la
    /// **única** implementación de esa precedencia (la comparten el handler de
    /// estado y `resolve_config`).
    #[tokio::test]
    #[serial]
    async fn resolve_client_id_prefers_setting_then_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "oximeal_client_id", "from-settings")
            .await
            .unwrap();
        set_env("OXIMEAL_CLIENT_ID", Some("from-env"));

        assert_eq!(
            Oximeal::resolve_client_id(&pool).await,
            "from-settings",
            "el ajuste gana al entorno"
        );

        SettingsRepo::set(&pool, "oximeal_client_id", "")
            .await
            .unwrap();
        assert_eq!(
            Oximeal::resolve_client_id(&pool).await,
            "from-env",
            "el entorno sirve de respaldo"
        );

        set_env("OXIMEAL_CLIENT_ID", None);
        assert_eq!(
            Oximeal::resolve_client_id(&pool).await,
            "",
            "sin ajuste ni entorno queda vacío"
        );
    }

    /// Scenario: `oximeal_issuer_url` vacío cae a `AUTH_ISSUER_URL`.
    #[tokio::test]
    #[serial]
    async fn issuer_url_falls_back_to_auth_issuer() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "oximeal_issuer_url", "")
            .await
            .unwrap();
        set_env("OXIMEAL_ISSUER_URL", None);
        set_env("AUTH_ISSUER_URL", Some("https://auth.example.test"));

        let issuer = Oximeal::resolve_issuer_url(&pool).await;

        set_env("AUTH_ISSUER_URL", None);

        assert_eq!(issuer, "https://auth.example.test");
    }

    // -----------------------------------------------------------------------
    // Token M2M: descubrimiento, petición, caché y renovación
    // -----------------------------------------------------------------------

    /// Scenario: el token se cachea y se reutiliza entre operaciones seguidas.
    #[tokio::test]
    #[serial]
    async fn token_is_cached_and_reused() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(header("authorization", "Bearer tok-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .expect(2)
            .mount(&server)
            .await;

        let client = client_for(&server);
        client.recent_recipes(7).await.expect("primera");
        client.recent_recipes(7).await.expect("segunda");

        assert_eq!(
            count_path(&server, "/token").await,
            1,
            "el token debe reutilizarse de la caché"
        );
        assert_eq!(
            count_path(&server, "/.well-known/openid-configuration").await,
            1,
            "el descubrimiento también se cachea"
        );
    }

    /// Scenario: dos llamadas concurrentes con la caché fría comparten un único
    /// refresco de token (*single-flight*): solo un llamante pide token y
    /// descubre el endpoint; el otro espera y reutiliza el resultado.
    #[tokio::test]
    #[serial]
    async fn concurrent_cold_cache_requests_a_single_token() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .expect(2)
            .mount(&server)
            .await;

        let client = client_for(&server);
        let (first, second) = tokio::join!(client.recent_recipes(7), client.recent_recipes(7));
        first.expect("primera");
        second.expect("segunda");

        assert_eq!(
            count_path(&server, "/token").await,
            1,
            "con la caché fría, N llamadas concurrentes deben pedir UN solo token"
        );
        assert_eq!(
            count_path(&server, "/.well-known/openid-configuration").await,
            1,
            "el descubrimiento también debe ser único"
        );
    }

    /// Scenario: el descubrimiento se hace y la petición lleva el `scope` correcto.
    #[tokio::test]
    #[serial]
    async fn discovery_and_client_credentials_request() {
        clear_token_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=client_credentials"))
            .and(body_string_contains("client_id=client-id"))
            .and(body_string_contains(
                "scope=agent%3Aread+agent%3Aplan+agent%3Ashopping+agent%3Awrite",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "tok-1",
                "expires_in": 3600,
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(header("authorization", "Bearer tok-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .mount(&server)
            .await;

        client_for(&server).recent_recipes(7).await.expect("ok");
    }

    /// Scenario: un token a punto de expirar se renueva antes de usarlo.
    #[tokio::test]
    #[serial]
    async fn expired_token_is_renewed() {
        clear_token_cache();
        let server = MockServer::start().await;
        // `expires_in` por debajo del margen de 30 s: siempre hay que renovar.
        mount_auth(&server, "tok-1", 10).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [],
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        client.recent_recipes(7).await.expect("primera");
        client.recent_recipes(7).await.expect("segunda");

        assert_eq!(
            count_path(&server, "/token").await,
            2,
            "un token a menos de 30 s de expirar debe renovarse"
        );
    }

    /// Scenario: `401` de la Agent API → `Unauthorized` e invalidación de caché.
    #[tokio::test]
    #[serial]
    async fn unauthorized_invalidates_cache() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = client_for(&server).recent_recipes(7).await.unwrap_err();

        assert!(matches!(err, OximealError::Unauthorized), "err: {err:?}");
        assert_eq!(
            count_path(&server, "/token").await,
            2,
            "el 401 debe invalidar la caché y forzar una única renovación"
        );
    }

    /// Scenario: el fallo del IdP en la fase de token se etiqueta como
    /// `TokenEndpoint`, distinto de un `Upstream` de la Agent API.
    #[tokio::test]
    #[serial]
    async fn idp_failure_is_token_endpoint() {
        clear_token_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token_endpoint": format!("{}/token", server.uri()),
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let err = client_for(&server).recent_recipes(7).await.unwrap_err();

        assert!(
            matches!(err, OximealError::TokenEndpoint(_)),
            "err: {err:?}"
        );
    }

    /// Scenario: un fallo en el **descubrimiento** OIDC también es de la fase de
    /// token (`TokenEndpoint`), no un `Upstream` de la Agent API.
    #[tokio::test]
    #[serial]
    async fn discovery_failure_is_token_endpoint() {
        clear_token_cache();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = client_for(&server).recent_recipes(7).await.unwrap_err();

        assert!(
            matches!(err, OximealError::TokenEndpoint(_)),
            "err: {err:?}"
        );
    }

    /// Scenario: un `5xx` de la **Agent API** sigue siendo `Upstream`, aunque su
    /// cuerpo mencione la palabra «token».
    #[tokio::test]
    #[serial]
    async fn oximeal_5xx_stays_upstream_even_if_it_mentions_token() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(serde_json::json!({ "error": "token bucket saturated" })),
            )
            .mount(&server)
            .await;

        let err = client_for(&server).recent_recipes(7).await.unwrap_err();

        assert!(matches!(err, OximealError::Upstream(_)), "err: {err:?}");
    }

    // -----------------------------------------------------------------------
    // Traducción de errores de la Agent API
    // -----------------------------------------------------------------------

    /// Scenario: `400` → `InvalidInput` conservando el mensaje del servidor.
    #[tokio::test]
    #[serial]
    async fn error_400_carries_the_server_message() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("POST"))
            .and(path("/api/agent/recipes"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "error": "categoría inválida: foo" })),
            )
            .mount(&server)
            .await;

        let err = client_for(&server)
            .create_recipe(&serde_json::json!({ "name": "x" }))
            .await
            .unwrap_err();

        match err {
            OximealError::InvalidInput(detail) => assert!(
                detail.contains("categoría inválida: foo"),
                "el detalle pierde el mensaje del servidor: {detail}"
            ),
            other => panic!("se esperaba InvalidInput: {other:?}"),
        }
    }

    /// `404` → `NotFound` y `5xx` → `Upstream`.
    #[tokio::test]
    #[serial]
    async fn not_found_and_upstream_statuses() {
        for (status, expected) in [(404u16, "NotFound"), (500, "Upstream"), (503, "Upstream")] {
            clear_token_cache();
            let server = MockServer::start().await;
            mount_auth(&server, "tok-1", 3600).await;
            Mock::given(method("GET"))
                .and(path("/api/agent/recent-recipes"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let err = client_for(&server).recent_recipes(7).await.unwrap_err();
            let label = match &err {
                OximealError::NotFound => "NotFound",
                OximealError::Upstream(_) => "Upstream",
                other => panic!("inesperado para {status}: {other:?}"),
            };
            assert_eq!(label, expected, "status {status}");
        }
    }

    /// Ningún mensaje de error filtra el secreto.
    #[tokio::test]
    #[serial]
    async fn errors_never_contain_the_secret() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "error": format!("usa {SECRET} bien") })),
            )
            .mount(&server)
            .await;

        let err = client_for(&server).recent_recipes(7).await.unwrap_err();

        assert!(!err.to_string().contains(SECRET), "Display: {err}");
        assert!(!format!("{err:?}").contains(SECRET), "Debug: {err:?}");
    }

    // -----------------------------------------------------------------------
    // Operaciones
    // -----------------------------------------------------------------------

    /// Scenario: la búsqueda usa `query` y `category` y mapea la respuesta.
    #[tokio::test]
    #[serial]
    async fn search_recipes_sends_query_and_category() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/search-recipes"))
            .and(query_param("query", "pasta"))
            .and(query_param("category", "main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [{
                    "id": "abc",
                    "name": "Pasta al pesto",
                    "category": "main",
                    "diet_type": "vegetarian",
                    "prep_time_minutes": 20,
                    "servings": 2,
                    "tags": ["italiana", "rápida"],
                    "extra_field": "se ignora"
                }]
            })))
            .mount(&server)
            .await;

        let recipes = client_for(&server)
            .search_recipes(Some("pasta"), Some("main"))
            .await
            .expect("ok");

        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].id, "abc");
        assert_eq!(recipes[0].name, "Pasta al pesto");
        assert_eq!(recipes[0].prep_time_minutes, Some(20));
        assert_eq!(recipes[0].tags, vec!["italiana", "rápida"]);
    }

    /// `recent_recipes`: `days` por defecto 7 y acotado a `1..=365`.
    #[tokio::test]
    #[serial]
    async fn recent_recipes_clamps_days() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(query_param("days", "7"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": [{"id":"a","name":"A","category":"main","last_planned":null}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(query_param("days", "365"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": []
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let defaulted = client.recent_recipes(0).await.expect("0 → 7");
        let clamped = client.recent_recipes(9999).await.expect("9999 → 365");

        assert_eq!(defaulted.len(), 1);
        assert!(clamped.is_empty());
    }

    /// Scenario: el plan semanal envía las entradas estructuradas.
    #[tokio::test]
    #[serial]
    async fn set_weekly_plan_sends_structured_entries() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("POST"))
            .and(path("/api/agent/weekly-plan"))
            .and(body_json(serde_json::json!({
                "entries": [{
                    "date": "2026-11-09",
                    "meal_type": "dinner",
                    "recipe_id": "abc",
                    "role": "main",
                    "servings": 4
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "created": 1,
                "shopping_list": [{"category": "verduras", "items": []}]
            })))
            .mount(&server)
            .await;

        let entries = vec![PlanEntry {
            date: "2026-11-09".to_string(),
            meal_type: "dinner".to_string(),
            recipe_id: "abc".to_string(),
            role: Some("main".to_string()),
            servings: Some(4),
        }];

        let result = client_for(&server)
            .set_weekly_plan(&entries)
            .await
            .expect("ok");

        assert_eq!(result.created, 1);
        assert_eq!(result.shopping_list.len(), 1);
        assert_eq!(result.shopping_list[0].category, "verduras");
    }

    /// `add_shopping_items` envía `{items:[…]}` y mapea los grupos devueltos.
    #[tokio::test]
    #[serial]
    async fn add_shopping_items_sends_items_and_maps_groups() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("POST"))
            .and(path("/api/agent/shopping-items"))
            .and(body_json(serde_json::json!({
                "items": [{
                    "name": "Tomate",
                    "amount": 3.0,
                    "unit": "ud",
                    "category": "verduras"
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"category": "verduras", "items": [{"name": "Tomate"}]}
            ])))
            .mount(&server)
            .await;

        let items = vec![ShoppingItemInput {
            name: "Tomate".to_string(),
            amount: 3.0,
            unit: "ud".to_string(),
            category: Some("verduras".to_string()),
        }];

        let groups = client_for(&server)
            .add_shopping_items(&items)
            .await
            .expect("ok");

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].category, "verduras");
    }

    /// `delete_completed` devuelve el número de ítems purgados.
    #[tokio::test]
    #[serial]
    async fn delete_completed_returns_removed_count() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("DELETE"))
            .and(path("/api/agent/shopping/completed"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "removed": 3 })),
            )
            .mount(&server)
            .await;

        let removed = client_for(&server).delete_completed().await.expect("ok");

        assert_eq!(removed, 3);
    }

    /// `create_recipe` pasa el `RecipeInput` tal cual y devuelve la receta.
    #[tokio::test]
    #[serial]
    async fn create_recipe_passes_body_through() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        let input = serde_json::json!({
            "name": "Tortilla",
            "category": "main",
            "ingredients": [{"name": "Huevo", "amount": 3, "unit": "ud"}]
        });
        Mock::given(method("POST"))
            .and(path("/api/agent/recipes"))
            .and(body_json(&input))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "new-id",
                "name": "Tortilla",
                "category": "main"
            })))
            .mount(&server)
            .await;

        let created = client_for(&server).create_recipe(&input).await.expect("ok");

        assert_eq!(created.get("id").and_then(|v| v.as_str()), Some("new-id"));
    }

    /// `probe` consulta las recetas de las últimas 24 h (`days=1`).
    #[tokio::test]
    #[serial]
    async fn probe_uses_days_one() {
        clear_token_cache();
        let server = MockServer::start().await;
        mount_auth(&server, "tok-1", 3600).await;
        Mock::given(method("GET"))
            .and(path("/api/agent/recent-recipes"))
            .and(query_param("days", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "recipes": []
            })))
            .mount(&server)
            .await;

        client_for(&server).probe().await.expect("sonda ok");
    }
}
