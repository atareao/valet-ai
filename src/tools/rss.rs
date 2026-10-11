//! Herramientas de lectura de feeds sobre la API v1 de `Miniflux`.
//!
//! Cuatro herramientas sobre el lector de feeds: listar las entradas no leídas
//! sin su contenido (`rss_list_unread`), traer una entrada concreta en texto
//! plano sin marcarla como leída (`rss_get_entry`), buscar por palabra clave
//! —también entre lo ya leído— (`rss_search_entries`) y marcar entradas como
//! leídas de forma idempotente (`rss_mark_read`).
//!
//! La configuración (URL base y token) se resuelve con
//! [`crate::services::miniflux::Miniflux::resolve_config`] en cada ejecución; el
//! token viaja en la cabecera `X-Auth-Token` y **nunca** aparece en el mensaje
//! de un error. Ver `src/services/miniflux.rs`.
//!
//! Fase **GREEN**: las cuatro herramientas están implementadas y los escenarios
//! del spec `tools/rss` se cubren con los tests de este módulo.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::services::miniflux::{Miniflux, MinifluxConfig, MinifluxError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// `limit` por defecto de `rss_list_unread` y `rss_search_entries` cuando el
/// modelo no lo indica.
const DEFAULT_LIMIT: u64 = 50;

/// Cota inferior aceptada para `limit`.
const MIN_LIMIT: u64 = 1;

/// Cota superior aceptada para `limit`.
const MAX_LIMIT: u64 = 200;

/// Máximo de caracteres a los que se recorta el `content` de `rss_get_entry`
/// (contado en caracteres, no en bytes, para no partir un carácter UTF-8).
pub const MAX_CONTENT_CHARS: usize = 20_000;

/// Acceso compartido a la configuración de Miniflux: la base de datos (de donde
/// `resolve_config` la resuelve en cada llamada) más un override disponible solo
/// en tests, para apuntar a un servidor `wiremock` sin tocar el entorno ni los
/// ajustes.
struct RssAccess {
    db: SqlitePool,
    #[cfg(test)]
    config_override: Option<MinifluxConfig>,
}

impl RssAccess {
    fn new(db: SqlitePool) -> Self {
        Self {
            db,
            #[cfg(test)]
            config_override: None,
        }
    }

    #[cfg(test)]
    fn with_config(db: SqlitePool, config: MinifluxConfig) -> Self {
        Self {
            db,
            config_override: Some(config),
        }
    }

    /// Resuelve la configuración de Miniflux en cada llamada y la traduce a
    /// [`ToolError`] si no hay token.
    async fn resolve(&self) -> Result<MinifluxConfig, ToolError> {
        #[cfg(test)]
        if let Some(config) = &self.config_override {
            return Ok(config.clone());
        }
        Miniflux::resolve_config(&self.db)
            .await
            .map_err(miniflux_error)
    }
}

/// Traduce un [`MinifluxError`] a un [`ToolError`] accionable.
///
/// Ningún mensaje incluye el token: el detalle crudo de
/// [`MinifluxError::Http`] se descarta a propósito.
fn miniflux_error(error: MinifluxError) -> ToolError {
    match error {
        MinifluxError::MissingCredentials => ToolError::ExecutionError(
            "No hay token de Miniflux configurado. Añádelo en Ajustes → API Keys, \
             o define la variable de entorno MINIFLUX_API_KEY."
                .to_string(),
        ),
        MinifluxError::Unauthorized => ToolError::ExecutionError(
            "El token de Miniflux no es válido o ha caducado.".to_string(),
        ),
        MinifluxError::NotFound => {
            ToolError::NotFound("La entrada indicada no existe en Miniflux.".to_string())
        }
        MinifluxError::Unavailable => {
            ToolError::ExecutionError("Miniflux no está disponible en este momento.".to_string())
        }
        MinifluxError::Http(_) => {
            ToolError::ExecutionError("No se pudo completar la operación con Miniflux.".to_string())
        }
    }
}

/// Valida el `limit` opcional y lo devuelve acotado a `1..=200` (50 si se
/// omite). No toca la red.
fn parse_limit(args: &Value) -> Result<u32, ToolError> {
    let Some(raw) = args.get("limit") else {
        return Ok(DEFAULT_LIMIT as u32);
    };
    let requested = raw.as_u64().ok_or_else(|| {
        ToolError::InvalidArguments(format!(
            "limit inválido: {raw}; debe ser un entero entre {MIN_LIMIT} y {MAX_LIMIT}"
        ))
    })?;
    if !(MIN_LIMIT..=MAX_LIMIT).contains(&requested) {
        return Err(ToolError::InvalidArguments(format!(
            "limit inválido: {requested}; debe ser un entero entre {MIN_LIMIT} y {MAX_LIMIT}"
        )));
    }
    Ok(requested as u32)
}

/// Valida un argumento entero obligatorio y mayor que 0. No toca la red.
fn positive_id(args: &Value, key: &str) -> Result<i64, ToolError> {
    match args.get(key).and_then(|v| v.as_i64()) {
        Some(value) if value > 0 => Ok(value),
        _ => Err(ToolError::InvalidArguments(format!(
            "{key} es obligatorio y debe ser un entero mayor que 0"
        ))),
    }
}

/// Extrae la `query` (obligatoria y no en blanco tras `trim`). No toca la red.
fn query_arg(args: &Value) -> Result<String, ToolError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(ToolError::InvalidArguments(
            "query es obligatorio y no puede estar en blanco".to_string(),
        ));
    }
    Ok(query)
}

/// Extrae `entry_ids` como array no vacío de enteros. Descarta las entradas que
/// no sean enteros; si no sobrevive ninguna, es un argumento inválido. No toca
/// la red.
fn entry_ids(args: &Value) -> Result<Vec<i64>, ToolError> {
    let ids: Vec<i64> = args
        .get("entry_ids")
        .and_then(|v| v.as_array())
        .map(|entries| entries.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if ids.is_empty() {
        return Err(ToolError::InvalidArguments(
            "entry_ids es obligatorio y debe ser un array no vacío de enteros".to_string(),
        ));
    }
    Ok(ids)
}

/// Resuelve el **nombre** de una categoría a su `id` contra
/// `GET /v1/categories`, sin distinguir mayúsculas. Si no coincide ninguna,
/// devuelve un error que enumera los nombres disponibles.
async fn resolve_category_id(
    client: &Miniflux,
    api_key: &str,
    name: &str,
) -> Result<i64, ToolError> {
    let value = client.categories(api_key).await.map_err(miniflux_error)?;
    let categories = value.as_array().cloned().unwrap_or_default();

    let wanted = name.to_lowercase();
    for category in &categories {
        let title = category.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if title.to_lowercase() == wanted {
            if let Some(id) = category.get("id").and_then(|v| v.as_i64()) {
                return Ok(id);
            }
        }
    }

    let available: Vec<String> = categories
        .iter()
        .filter_map(|category| category.get("title").and_then(|v| v.as_str()))
        .map(|title| title.to_string())
        .collect();
    let list = if available.is_empty() {
        "(ninguna)".to_string()
    } else {
        available.join(", ")
    };
    Err(ToolError::InvalidArguments(format!(
        "La categoría «{name}» no existe en Miniflux. Categorías disponibles: {list}."
    )))
}

/// Resumen de una entrada para el listado y la búsqueda: metadatos sin el
/// `content`. `feed` es el título del medio.
fn entry_summary(entry: &Value) -> Value {
    json!({
        "id": entry.get("id").and_then(|v| v.as_i64()),
        "title": entry.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        "feed": entry
            .get("feed")
            .and_then(|feed| feed.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        "url": entry.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "published_at": entry
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        "reading_time": entry.get("reading_time").and_then(|v| v.as_i64()),
    })
}

/// Extrae la lista de resúmenes de una respuesta `{total, entries:[…]}`.
fn entries_of(value: &Value) -> Vec<Value> {
    value
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|entries| entries.iter().map(entry_summary).collect())
        .unwrap_or_default()
}

/// Recorta `input` a `max` **caracteres** (nunca a bytes) y dice si hubo
/// recorte. Trabajar por caracteres evita partir un carácter UTF-8, que
/// reventaría con `&str`.
fn truncate_chars(input: &str, max: usize) -> (String, bool) {
    let mut chars = input.chars();
    let truncated: String = chars.by_ref().take(max).collect();
    let was_truncated = chars.next().is_some();
    (truncated, was_truncated)
}

/// Elimina las etiquetas HTML. Cada etiqueta se sustituye por un espacio para no
/// pegar palabras contiguas.
fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if in_tag => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Descodifica una entidad HTML nombrada, decimal (`&#NNN;`) o hexadecimal
/// (`&#xHH;`). Devuelve `None` para una entidad desconocida o malformada, de
/// modo que se conserve literal.
fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "39" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = entity.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

/// Descodifica las entidades HTML de `input`. Las entidades desconocidas se
/// dejan tal cual.
fn decode_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(rel) = input[i..].find(';').filter(|&rel| rel <= 32) {
                let entity = &input[i + 1..i + rel];
                if let Some(decoded) = decode_entity(entity) {
                    out.push(decoded);
                    i += rel + 1;
                    continue;
                }
            }
        }
        // `i` siempre cae en una frontera de carácter, así que hay uno.
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Colapsa cualquier secuencia de espacios en blanco en un solo espacio y
/// recorta los extremos.
fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

/// Convierte HTML de una entrada a texto plano: fuera etiquetas, entidades
/// descodificadas y espacios colapsados.
fn html_to_plain_text(html: &str) -> String {
    collapse_whitespace(&decode_entities(&strip_tags(html)))
}

/// `rss_list_unread` — lista las entradas no leídas sin su contenido.
pub struct RssListUnreadTool {
    access: RssAccess,
}

impl RssListUnreadTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: RssAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for RssListUnreadTool {
    fn name(&self) -> &'static str {
        "rss_list_unread"
    }

    fn description(&self) -> &'static str {
        "Lista las entradas no leídas de los feeds de Miniflux. Devuelve el total de no leídas y, \
         de cada entrada, su `id`, `title`, `feed`, `url`, `published_at` y `reading_time`, sin \
         el contenido. Ningún argumento es obligatorio: `limit` (opcional, máximo de entradas, 50 \
         por defecto, entre 1 y 200) y `category` (opcional, nombre de la categoría, resuelto sin \
         distinguir mayúsculas). No altera el estado de ninguna entrada."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "minimum": MIN_LIMIT,
                    "maximum": MAX_LIMIT,
                    "description": format!("Número máximo de entradas a devolver ({DEFAULT_LIMIT} por defecto, entre {MIN_LIMIT} y {MAX_LIMIT})")
                },
                "category": {
                    "type": "string",
                    "description": "Nombre de la categoría; se resuelve contra las categorías de Miniflux sin distinguir mayúsculas"
                }
            },
            "required": []
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        // Todo lo que no necesita red se valida antes de resolver la
        // configuración, para no salir a la red ante argumentos inválidos.
        let limit = parse_limit(&args)?;
        let category_name = args
            .get("category")
            .and_then(|v| v.as_str())
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());

        let config = self.access.resolve().await?;
        let client = Miniflux::new(config.base_url);

        let category_id = match category_name {
            Some(name) => Some(resolve_category_id(&client, &config.api_key, &name).await?),
            None => None,
        };

        let value = client
            .list_unread(&config.api_key, limit, category_id)
            .await
            .map_err(miniflux_error)?;

        let entries = entries_of(&value);
        let total = value
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(entries.len() as u64);

        Ok(ToolResult {
            success: true,
            data: json!({ "total": total, "entries": entries }),
            message: Some(format!("{total} entradas sin leer")),
        })
    }
}

/// `rss_get_entry` — devuelve el contenido de una entrada sin marcarla como
/// leída.
pub struct RssGetEntryTool {
    access: RssAccess,
}

impl RssGetEntryTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: RssAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for RssGetEntryTool {
    fn name(&self) -> &'static str {
        "rss_get_entry"
    }

    fn description(&self) -> &'static str {
        "Devuelve una entrada de Miniflux por su identificador. El argumento `entry_id` es \
         obligatorio y debe ser un entero positivo. Devuelve `id`, `title`, `feed`, `url`, \
         `author`, `published_at`, `status` y `reading_time`, además del `content` convertido a \
         texto plano (etiquetas HTML eliminadas y entidades descodificadas) y recortado a 20 000 \
         caracteres, con `truncated` en true si hubo recorte. Leer el contenido no marca la \
         entrada como leída."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entry_id": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Identificador de la entrada (obligatorio)"
                }
            },
            "required": ["entry_id"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        // La validación del argumento no necesita red.
        let entry_id = positive_id(&args, "entry_id")?;

        let config = self.access.resolve().await?;
        let client = Miniflux::new(config.base_url);

        // `get_entry` no cambia el estado de la entrada.
        let value = client
            .get_entry(&config.api_key, entry_id)
            .await
            .map_err(miniflux_error)?;

        let content_html = value.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let (content, truncated) =
            truncate_chars(&html_to_plain_text(content_html), MAX_CONTENT_CHARS);

        Ok(ToolResult {
            success: true,
            data: json!({
                "id": value.get("id").and_then(|v| v.as_i64()),
                "title": value.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "feed": value
                    .get("feed")
                    .and_then(|feed| feed.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                "url": value.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                "author": value.get("author").and_then(|v| v.as_str()).unwrap_or(""),
                "published_at": value
                    .get("published_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                "status": value.get("status").and_then(|v| v.as_str()).unwrap_or(""),
                "reading_time": value.get("reading_time").and_then(|v| v.as_i64()),
                "content": content,
                "truncated": truncated,
            }),
            message: None,
        })
    }
}

/// `rss_search_entries` — busca por palabra clave sin filtrar por estado.
pub struct RssSearchEntriesTool {
    access: RssAccess,
}

impl RssSearchEntriesTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: RssAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for RssSearchEntriesTool {
    fn name(&self) -> &'static str {
        "rss_search_entries"
    }

    fn description(&self) -> &'static str {
        "Busca entradas en Miniflux por palabra clave. El argumento `query` es obligatorio y no \
         puede estar en blanco; `limit` es opcional (50 por defecto, entre 1 y 200). Busca sin \
         filtrar por estado, de modo que encuentra también las entradas ya leídas, y devuelve los \
         mismos metadatos que el listado, sin el contenido."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Palabra clave a buscar (obligatorio, no puede estar en blanco)"
                },
                "limit": {
                    "type": "integer",
                    "minimum": MIN_LIMIT,
                    "maximum": MAX_LIMIT,
                    "description": format!("Número máximo de entradas a devolver ({DEFAULT_LIMIT} por defecto, entre {MIN_LIMIT} y {MAX_LIMIT})")
                }
            },
            "required": ["query"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        // Todo lo que no necesita red se valida antes de resolver la
        // configuración.
        let query = query_arg(&args)?;
        let limit = parse_limit(&args)?;

        let config = self.access.resolve().await?;
        let client = Miniflux::new(config.base_url);

        // `search_entries` **no** envía `status`: encuentra también lo ya leído.
        let value = client
            .search_entries(&config.api_key, &query, limit)
            .await
            .map_err(miniflux_error)?;

        let entries = entries_of(&value);
        let total = value
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(entries.len() as u64);

        Ok(ToolResult {
            success: true,
            data: json!({ "total": total, "entries": entries }),
            message: Some(format!("{total} entradas encontradas")),
        })
    }
}

/// `rss_mark_read` — marca entradas como leídas de forma idempotente.
pub struct RssMarkReadTool {
    access: RssAccess,
}

impl RssMarkReadTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: RssAccess::new(db),
        }
    }
}

#[async_trait]
impl Tool for RssMarkReadTool {
    fn name(&self) -> &'static str {
        "rss_mark_read"
    }

    fn description(&self) -> &'static str {
        "Marca entradas de Miniflux como leídas. El argumento `entry_ids` es obligatorio: un array \
         no vacío de identificadores enteros. Las marca en una sola petición y devuelve cuántos \
         identificadores se pidieron. Es idempotente: marcar una entrada ya leída no es un error."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "entry_ids": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "minItems": 1,
                    "description": "Identificadores de las entradas a marcar como leídas (obligatorio, array no vacío)"
                }
            },
            "required": ["entry_ids"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        // La validación del argumento no necesita red.
        let ids = entry_ids(&args)?;
        let count = ids.len();

        let config = self.access.resolve().await?;
        let client = Miniflux::new(config.base_url);

        // Una sola petición; es idempotente (marcar lo ya leído no es error).
        client
            .mark_read(&config.api_key, &ids)
            .await
            .map_err(miniflux_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "count": count }),
            message: Some(format!("{count} entradas marcadas como leídas")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use serial_test::serial;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Base de datos en memoria migrada, como en `src/tools/email.rs`.
    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn clear_miniflux_env() {
        std::env::remove_var("MINIFLUX_BASE_URL");
        std::env::remove_var("MINIFLUX_API_KEY");
    }

    /// Config apuntando al servidor `wiremock` con el token de test.
    fn config_for(server: &MockServer) -> MinifluxConfig {
        config_with(server, "test-key")
    }

    /// Config apuntando al servidor `wiremock` con un token concreto.
    fn config_with(server: &MockServer, api_key: &str) -> MinifluxConfig {
        MinifluxConfig {
            base_url: server.uri(),
            api_key: api_key.to_string(),
        }
    }

    /// Entrada de Miniflux con los metadatos que devuelven listado y búsqueda.
    fn entry_json(id: i64, title: &str, feed: &str) -> Value {
        json!({
            "id": id,
            "title": title,
            "url": format!("https://example.com/{id}"),
            "published_at": "2026-01-02T03:04:05Z",
            "reading_time": 4,
            "content": "<p>no debería aparecer</p>",
            "feed": { "id": 1, "title": feed }
        })
    }

    // -----------------------------------------------------------------------
    // Superficie: nombres, descripciones, esquema y permisos
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_names() {
        let db = setup_db().await;
        assert_eq!(RssListUnreadTool::new(db.clone()).name(), "rss_list_unread");
        assert_eq!(RssGetEntryTool::new(db.clone()).name(), "rss_get_entry");
        assert_eq!(
            RssSearchEntriesTool::new(db.clone()).name(),
            "rss_search_entries"
        );
        assert_eq!(RssMarkReadTool::new(db).name(), "rss_mark_read");
    }

    #[tokio::test]
    async fn test_descriptions_are_spanish_and_mention_required() {
        let db = setup_db().await;
        let list = RssListUnreadTool::new(db.clone());
        let get = RssGetEntryTool::new(db.clone());
        let search = RssSearchEntriesTool::new(db.clone());
        let mark = RssMarkReadTool::new(db);

        assert!(
            list.description().contains("no leídas"),
            "{}",
            list.description()
        );
        assert!(
            list.description().contains("limit"),
            "{}",
            list.description()
        );
        assert!(
            list.description().contains("opcional"),
            "{}",
            list.description()
        );

        assert!(
            get.description().contains("entry_id"),
            "{}",
            get.description()
        );
        assert!(
            get.description().contains("obligatorio"),
            "{}",
            get.description()
        );
        assert!(
            get.description().contains("texto plano"),
            "{}",
            get.description()
        );

        assert!(
            search.description().contains("query"),
            "{}",
            search.description()
        );
        assert!(
            search.description().contains("obligatorio"),
            "{}",
            search.description()
        );
        assert!(
            search.description().contains("leídas"),
            "{}",
            search.description()
        );

        assert!(
            mark.description().contains("entry_ids"),
            "{}",
            mark.description()
        );
        assert!(
            mark.description().contains("obligatorio"),
            "{}",
            mark.description()
        );
        assert!(
            mark.description().contains("idempotente"),
            "{}",
            mark.description()
        );
    }

    #[tokio::test]
    async fn test_parameters_are_object_schemas() {
        let db = setup_db().await;

        let list = RssListUnreadTool::new(db.clone()).parameters();
        assert_eq!(list["type"], "object");
        assert_eq!(list["properties"]["limit"]["type"], "integer");
        assert_eq!(list["properties"]["limit"]["minimum"], 1);
        assert_eq!(list["properties"]["limit"]["maximum"], 200);
        assert_eq!(list["properties"]["category"]["type"], "string");
        assert!(list["required"].as_array().unwrap().is_empty());

        let get = RssGetEntryTool::new(db.clone()).parameters();
        assert_eq!(get["type"], "object");
        assert_eq!(get["properties"]["entry_id"]["type"], "integer");
        assert_eq!(get["required"], json!(["entry_id"]));

        let search = RssSearchEntriesTool::new(db.clone()).parameters();
        assert_eq!(search["type"], "object");
        assert_eq!(search["properties"]["query"]["type"], "string");
        assert_eq!(search["required"], json!(["query"]));

        let mark = RssMarkReadTool::new(db).parameters();
        assert_eq!(mark["type"], "object");
        assert_eq!(mark["properties"]["entry_ids"]["type"], "array");
        assert_eq!(mark["required"], json!(["entry_ids"]));
    }

    #[tokio::test]
    async fn test_permissions_are_noconfirm() {
        let db = setup_db().await;
        assert_eq!(
            RssListUnreadTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            RssGetEntryTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            RssSearchEntriesTool::new(db.clone()).permission(&json!({})),
            Permission::NoConfirm
        );
        assert_eq!(
            RssMarkReadTool::new(db).permission(&json!({})),
            Permission::NoConfirm
        );
    }

    // -----------------------------------------------------------------------
    // rss_list_unread
    // -----------------------------------------------------------------------

    /// Scenario: un listado devuelve el total y los metadatos sin contenido.
    #[tokio::test]
    async fn test_list_unread_returns_total_and_metadata_without_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .and(query_param("status", "unread"))
            .and(query_param("limit", "50"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total": 2,
                "entries": [
                    entry_json(10, "Primera", "Medio A"),
                    entry_json(11, "Segunda", "Medio B")
                ]
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({})).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["total"], 2);

        let entries = result.data["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0]["id"], 10);
        assert_eq!(entries[0]["title"], "Primera");
        assert_eq!(entries[0]["feed"], "Medio A");
        assert_eq!(entries[0]["url"], "https://example.com/10");
        assert_eq!(entries[0]["published_at"], "2026-01-02T03:04:05Z");
        assert_eq!(entries[0]["reading_time"], 4);
        assert!(
            entries[0].get("content").is_none(),
            "el listado no debe incluir el content: {}",
            entries[0]
        );

        assert_eq!(entries[1]["id"], 11);
        assert_eq!(entries[1]["feed"], "Medio B");
        assert!(entries[1].get("content").is_none(), "{}", entries[1]);
    }

    /// Scenario: un `limit` fuera de rango es inválido y no sale a la red.
    #[tokio::test]
    async fn test_list_unread_invalid_limit_without_request() {
        let server = MockServer::start().await;
        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        for args in [json!({ "limit": 0 }), json!({ "limit": 500 })] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un limit inválido no debe enviar ninguna petición"
        );
    }

    /// Scenario: `category` resuelve el nombre sin distinguir mayúsculas.
    #[tokio::test]
    async fn test_list_unread_category_resolves_case_insensitively() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/categories"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 1, "title": "Noticias" },
                { "id": 3, "title": "Tecnología" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .and(query_param("status", "unread"))
            .and(query_param("limit", "50"))
            .and(query_param("category_id", "3"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total": 1,
                "entries": [ entry_json(10, "Primera", "Medio A") ]
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool
            .execute(json!({ "category": "tecnología" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["total"], 1);

        let requests = server.received_requests().await.unwrap();
        let entries_request = requests
            .iter()
            .find(|r| r.url.path() == "/v1/entries")
            .expect("debe consultarse /v1/entries con la categoría resuelta");
        let query = entries_request.url.query().unwrap_or("");
        assert!(query.contains("category_id=3"), "query: {query}");
    }

    /// Scenario: una categoría inexistente enumera las disponibles.
    #[tokio::test]
    async fn test_list_unread_unknown_category_lists_available() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/categories"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 1, "title": "Noticias" },
                { "id": 2, "title": "Tecnología" }
            ])))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let err = tool
            .execute(json!({ "category": "Deportes" }))
            .await
            .unwrap_err();
        let message = match err {
            ToolError::InvalidArguments(message) => message,
            other => panic!("expected InvalidArguments, got {other:?}"),
        };
        assert!(message.contains("Noticias"), "{message}");
        assert!(message.contains("Tecnología"), "{message}");
    }

    // -----------------------------------------------------------------------
    // rss_get_entry
    // -----------------------------------------------------------------------

    /// Scenario: el contenido llega en texto plano y recortado a 20 000.
    #[tokio::test]
    async fn test_get_entry_content_is_plain_text_and_truncated() {
        let server = MockServer::start().await;
        let long_content = format!(
            "<p>Hola &amp; &lt;tag&gt; &quot;cita&quot; mundo! {}</p>",
            "á".repeat(25_000)
        );
        Mock::given(method("GET"))
            .and(path("/v1/entries/42"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 42,
                "title": "Artículo",
                "url": "https://example.com/42",
                "author": "Ada",
                "published_at": "2026-01-02T03:04:05Z",
                "status": "unread",
                "reading_time": 7,
                "content": long_content,
                "feed": { "id": 1, "title": "Medio A" }
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssGetEntryTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "entry_id": 42 })).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["id"], 42);
        assert_eq!(result.data["feed"], "Medio A");
        assert_eq!(result.data["author"], "Ada");
        assert_eq!(result.data["status"], "unread");

        let content = result.data["content"].as_str().unwrap();
        assert!(!content.contains("<p>"), "las etiquetas deben eliminarse");
        assert!(!content.contains("</p>"), "las etiquetas deben eliminarse");
        assert!(
            !content.contains("&amp;"),
            "las entidades deben descodificarse"
        );
        assert!(
            !content.contains("&lt;"),
            "las entidades deben descodificarse"
        );
        assert!(
            !content.contains("&quot;"),
            "las entidades deben descodificarse"
        );
        assert!(content.contains('&'), "&amp; debe quedar como & literal");
        assert!(content.contains('<'), "&lt; debe quedar como < literal");
        assert!(content.contains('"'), "&quot; debe quedar como \" literal");
        assert_eq!(content.chars().count(), MAX_CONTENT_CHARS);
        assert_eq!(result.data["truncated"], true);

        let requests = server.received_requests().await.unwrap();
        assert!(
            !requests.iter().any(|r| r.method.as_str() == "PUT"),
            "leer una entrada no debe marcar nada como leído"
        );
    }

    /// Un contenido corto no se recorta ni indica truncamiento.
    #[tokio::test]
    async fn test_get_entry_short_content_is_not_truncated() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries/7"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 7,
                "title": "Corto",
                "url": "https://example.com/7",
                "author": "Ada",
                "published_at": "2026-01-02T03:04:05Z",
                "status": "unread",
                "reading_time": 1,
                "content": "<p>Hola &amp; mundo</p>",
                "feed": { "id": 1, "title": "Medio A" }
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssGetEntryTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "entry_id": 7 })).await.unwrap();

        assert_eq!(result.data["content"], "Hola & mundo");
        assert_eq!(result.data["truncated"], false);
    }

    /// Scenario: consultar una entrada no la marca como leída.
    #[tokio::test]
    async fn test_get_entry_does_not_mark_as_read() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries/42"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 42,
                "title": "Artículo",
                "url": "https://example.com/42",
                "author": "Ada",
                "published_at": "2026-01-02T03:04:05Z",
                "status": "unread",
                "reading_time": 7,
                "content": "<p>texto</p>",
                "feed": { "id": 1, "title": "Medio A" }
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssGetEntryTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "entry_id": 42 })).await.unwrap();
        assert_eq!(result.data["status"], "unread");

        let requests = server.received_requests().await.unwrap();
        assert!(
            !requests.iter().any(|r| r.method.as_str() == "PUT"),
            "no debe recibirse ningún PUT: la entrada sigue sin leer"
        );
    }

    /// Scenario: un `entry_id` ausente o no positivo es inválido y no sale a la
    /// red.
    #[tokio::test]
    async fn test_get_entry_invalid_id_without_request() {
        let server = MockServer::start().await;
        let db = setup_db().await;
        let tool = RssGetEntryTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        for args in [
            json!({}),
            json!({ "entry_id": 0 }),
            json!({ "entry_id": -1 }),
        ] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un entry_id inválido no debe enviar ninguna petición"
        );
    }

    /// Scenario: un id inexistente (404) es un error accionable.
    #[tokio::test]
    async fn test_get_entry_not_found_is_actionable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries/77"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssGetEntryTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let err = tool.execute(json!({ "entry_id": 77 })).await.unwrap_err();
        match err {
            ToolError::NotFound(message) => {
                assert!(message.to_lowercase().contains("no existe"), "{message}");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // rss_search_entries
    // -----------------------------------------------------------------------

    /// Scenario: la petición no lleva filtro de estado.
    #[tokio::test]
    async fn test_search_entries_omits_status_filter() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .and(query_param("search", "rust"))
            .and(query_param("limit", "50"))
            .and(header("X-Auth-Token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total": 1,
                "entries": [ entry_json(10, "Rust", "Medio A") ]
            })))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssSearchEntriesTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "query": "rust" })).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["total"], 1);
        assert!(
            result.data["entries"][0].get("content").is_none(),
            "la búsqueda no debe incluir el content"
        );

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let query = requests[0].url.query().unwrap_or("");
        assert!(query.contains("search=rust"), "query: {query}");
        assert!(
            !query.contains("status="),
            "la búsqueda no debe filtrar por estado: {query}"
        );
    }

    /// Scenario: una búsqueda sin resultados es una lista vacía, no un error.
    #[tokio::test]
    async fn test_search_entries_empty_result_is_not_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .and(query_param("search", "nada"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "total": 0, "entries": [] })),
            )
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssSearchEntriesTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool.execute(json!({ "query": "nada" })).await.unwrap();

        assert!(result.success);
        assert_eq!(result.data["total"], 0);
        assert!(result.data["entries"].as_array().unwrap().is_empty());
    }

    /// Scenario: un `query` ausente o en blanco es inválido y no sale a la red.
    #[tokio::test]
    async fn test_search_entries_blank_query_without_request() {
        let server = MockServer::start().await;
        let db = setup_db().await;
        let tool = RssSearchEntriesTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        for args in [json!({}), json!({ "query": "   " })] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un query inválido no debe enviar ninguna petición"
        );
    }

    /// Un `limit` fuera de rango en la búsqueda también es inválido sin red.
    #[tokio::test]
    async fn test_search_entries_invalid_limit_without_request() {
        let server = MockServer::start().await;
        let db = setup_db().await;
        let tool = RssSearchEntriesTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        for args in [
            json!({ "query": "rust", "limit": 0 }),
            json!({ "query": "rust", "limit": 500 }),
        ] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un limit inválido no debe enviar ninguna petición"
        );
    }

    // -----------------------------------------------------------------------
    // rss_mark_read
    // -----------------------------------------------------------------------

    /// Scenario: marca varios en una sola petición con el cuerpo indicado.
    #[tokio::test]
    async fn test_mark_read_sends_single_put_with_body() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v1/entries"))
            .and(header("X-Auth-Token", "test-key"))
            .and(body_json(
                json!({ "entry_ids": [1, 2, 3], "status": "read" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssMarkReadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let result = tool
            .execute(json!({ "entry_ids": [1, 2, 3] }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["count"], 3);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.method.as_str() == "PUT")
                .count(),
            1,
            "debe enviarse una única petición PUT"
        );
    }

    /// Scenario: marcar dos veces no es un error.
    #[tokio::test]
    async fn test_mark_read_is_idempotent() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v1/entries"))
            .and(body_json(json!({ "entry_ids": [5], "status": "read" })))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssMarkReadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        let first = tool.execute(json!({ "entry_ids": [5] })).await.unwrap();
        let second = tool.execute(json!({ "entry_ids": [5] })).await.unwrap();

        assert!(first.success);
        assert!(second.success);
        assert_eq!(second.data["count"], 1);
    }

    /// Scenario: un `entry_ids` ausente o vacío es inválido y no sale a la red.
    #[tokio::test]
    async fn test_mark_read_empty_ids_without_request() {
        let server = MockServer::start().await;
        let db = setup_db().await;
        let tool = RssMarkReadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        for args in [json!({}), json!({ "entry_ids": [] })] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "una lista vacía no debe enviar ninguna petición"
        );
    }

    // -----------------------------------------------------------------------
    // Configuración ausente y traducción de errores
    // -----------------------------------------------------------------------

    /// Scenario: sin token, las cuatro herramientas devuelven un error
    /// accionable que indica dónde configurarlo, sin salir a la red.
    #[tokio::test]
    #[serial]
    async fn test_missing_token_is_actionable_without_request() {
        clear_miniflux_env();
        let db = setup_db().await;

        let results = vec![
            RssListUnreadTool::new(db.clone()).execute(json!({})).await,
            RssGetEntryTool::new(db.clone())
                .execute(json!({ "entry_id": 1 }))
                .await,
            RssSearchEntriesTool::new(db.clone())
                .execute(json!({ "query": "rust" }))
                .await,
            RssMarkReadTool::new(db.clone())
                .execute(json!({ "entry_ids": [1] }))
                .await,
        ];

        for result in results {
            match result {
                Err(ToolError::ExecutionError(message)) => {
                    assert!(message.contains("Ajustes"), "{message}");
                }
                other => panic!("expected ExecutionError, got {other:?}"),
            }
        }
    }

    /// Scenario: un `401` se traduce a token inválido o caducado.
    #[tokio::test]
    async fn test_error_401_maps_to_invalid_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        match tool.execute(json!({})).await {
            Err(ToolError::ExecutionError(message)) => {
                let lower = message.to_lowercase();
                assert!(lower.contains("token"), "{message}");
                assert!(
                    lower.contains("válido") || lower.contains("caducado"),
                    "{message}"
                );
            }
            other => panic!("expected ExecutionError, got {other:?}"),
        }
    }

    /// Scenario: un `5xx` se traduce a Miniflux no disponible.
    #[tokio::test]
    async fn test_error_503_maps_to_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(ResponseTemplate::new(503).set_body_string("miniflux down"))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_for(&server)),
        };

        match tool.execute(json!({})).await {
            Err(ToolError::ExecutionError(message)) => {
                assert!(message.to_lowercase().contains("disponible"), "{message}");
            }
            other => panic!("expected ExecutionError, got {other:?}"),
        }
    }

    /// Scenario: el token no aparece en la descripción ni en el error. El mock
    /// responde con un cuerpo que **sí** contiene el token, de modo que el test
    /// fallaría si el error reenviara ese cuerpo.
    #[tokio::test]
    async fn test_token_not_exposed_in_error() {
        const TOKEN: &str = "super-secret-token";
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/entries"))
            .respond_with(ResponseTemplate::new(401).set_body_string(format!("nope: {TOKEN}")))
            .mount(&server)
            .await;

        let db = setup_db().await;
        let tool = RssListUnreadTool {
            access: RssAccess::with_config(db, config_with(&server, TOKEN)),
        };

        assert!(!tool.description().contains(TOKEN));

        let err = tool.execute(json!({})).await.unwrap_err();
        let rendered = format!("{err:?} {err}");
        assert!(!rendered.contains(TOKEN), "el token se filtra: {rendered}");
    }
}
