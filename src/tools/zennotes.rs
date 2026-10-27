//! Herramienta `zennotes`: lee y escribe las notas del vault de ZenNotes del
//! usuario contra la API HTTP de su servidor autoalojado.
//!
//! Es una herramienta multioperación (molde [`crate::tools::calendar`]): despacha
//! por un parámetro `operation` con el enum completo y declara un permiso por
//! operación —lecturas sin confirmación, escrituras aditivas con aviso e
//! irreversibles con aprobación explícita—. La configuración (URL base, token y
//! carpeta) se resuelve con [`crate::services::zennotes::Zennotes::resolve_config`]
//! en cada ejecución; el token nunca aparece en un error.
//!
//! El recorte es obligatorio: `list` devuelve como mucho `limit` notas (20 por
//! defecto, hasta 200) y solo sus metadatos —nunca el `body`—, y `read` recorta
//! el cuerpo a [`MAX_BODY_CHARS`] caracteres indicando si hubo recorte. Volcar el
//! vault entero (≈6.273 notas) al prompt es justo lo que se evita.

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::services::zennotes::{Occurrence, Zennotes, ZennotesConfig, ZennotesError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// `limit` por defecto de `list` y `search`.
const DEFAULT_LIMIT: u64 = 20;

/// Cota inferior aceptada para `limit`.
const MIN_LIMIT: u64 = 1;

/// Cota superior aceptada para `limit`.
const MAX_LIMIT: u64 = 200;

/// Máximo de caracteres a los que `read` recorta el `body` (contado en
/// caracteres, no en bytes, para no partir un carácter UTF-8).
pub const MAX_BODY_CHARS: usize = 20_000;

/// Carpetas válidas de la API de ZenNotes.
///
/// En el servidor `folder` es un enum **restringido** a `inbox`, `quick` y
/// `archive` (más `trash`, que no se expone para crear o mover). `Valet` no es
/// una carpeta sino una **subcarpeta** dentro de `inbox`; enviarla como `folder`
/// hace que el servidor devuelva un error de servidor.
const VALID_FOLDERS: [&str; 3] = ["inbox", "quick", "archive"];

/// Carpeta destino por defecto al crear: `inbox` (el ajuste `zennotes_folder`
/// viaja como `subpath`, no como `folder`).
const DEFAULT_CREATE_FOLDER: &str = "inbox";

/// Valida un argumento de carpeta contra el enum del servidor y lo devuelve.
///
/// Ausente o en blanco → `default`. Fuera de [`VALID_FOLDERS`] →
/// [`ToolError::InvalidArguments`] **sin salir a la red** (el servidor responde
/// con un error de servidor ante una carpeta desconocida).
fn parse_folder(args: &Value, key: &str, default: &str) -> Result<String, ToolError> {
    match args.get(key).and_then(|v| v.as_str()).map(str::trim) {
        None | Some("") => Ok(default.to_string()),
        Some(value) => {
            if VALID_FOLDERS.contains(&value) {
                Ok(value.to_string())
            } else {
                Err(ToolError::InvalidArguments(format!(
                    "{key} inválido: {value}; debe ser inbox, quick o archive"
                )))
            }
        }
    }
}

/// Modo de `search`: por contenido, por título o por etiqueta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    /// Busca en el contenido con el endpoint de búsqueda del servidor.
    Text,
    /// Filtra localmente el listado por título.
    Title,
    /// Filtra localmente el listado por etiqueta.
    Tag,
}

/// Acceso compartido a la configuración de ZenNotes: la base de datos (de donde
/// `resolve_config` la resuelve en cada llamada) más un override disponible solo
/// en tests, para apuntar a un servidor `wiremock` sin tocar el entorno ni los
/// ajustes.
struct ZennotesAccess {
    db: SqlitePool,
    #[cfg(test)]
    config_override: Option<ZennotesConfig>,
}

impl ZennotesAccess {
    fn new(db: SqlitePool) -> Self {
        Self {
            db,
            #[cfg(test)]
            config_override: None,
        }
    }

    #[cfg(test)]
    fn with_config(db: SqlitePool, config: ZennotesConfig) -> Self {
        Self {
            db,
            config_override: Some(config),
        }
    }

    /// Resuelve la configuración en cada llamada y la traduce a [`ToolError`].
    async fn resolve(&self) -> Result<ZennotesConfig, ToolError> {
        #[cfg(test)]
        if let Some(config) = &self.config_override {
            return Ok(config.clone());
        }
        Zennotes::resolve_config(&self.db)
            .await
            .map_err(zennotes_error)
    }
}

/// Traduce un [`ZennotesError`] a un [`ToolError`] accionable.
///
/// Ningún mensaje incluye el token: el detalle crudo de [`ZennotesError::Http`]
/// se descarta a propósito.
fn zennotes_error(error: ZennotesError) -> ToolError {
    match error {
        ZennotesError::MissingCredentials => ToolError::ExecutionError(
            "No hay token de ZenNotes configurado. Añádelo en Ajustes → API Keys, \
             o define la variable de entorno ZENNOTES_TOKEN."
                .to_string(),
        ),
        ZennotesError::Unauthorized => ToolError::ExecutionError(
            "El token de ZenNotes no es válido o ha caducado.".to_string(),
        ),
        ZennotesError::NotFound => {
            ToolError::NotFound("La nota indicada no existe en ZenNotes.".to_string())
        }
        ZennotesError::Unavailable => {
            ToolError::ExecutionError("ZenNotes no está disponible en este momento.".to_string())
        }
        ZennotesError::Http(_) => {
            ToolError::ExecutionError("No se pudo completar la operación con ZenNotes.".to_string())
        }
    }
}

/// Valida el `limit` opcional y lo devuelve acotado a `1..=200` (20 si se
/// omite). No toca la red.
fn parse_limit(args: &Value) -> Result<usize, ToolError> {
    let Some(raw) = args.get("limit") else {
        return Ok(DEFAULT_LIMIT as usize);
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
    Ok(requested as usize)
}

/// Extrae un argumento de cadena obligatorio y no en blanco tras `trim`. No toca
/// la red.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ToolError::InvalidArguments(format!("{key} es obligatorio y no puede estar en blanco"))
        })
}

/// Extrae un argumento de cadena obligatorio **sin** recortar (para cuerpos y
/// textos cuyo contenido literal importa). No toca la red.
fn required_raw<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArguments(format!("{key} es obligatorio")))
}

/// Extrae un argumento de cadena opcional, ignorando los vacíos tras `trim`.
fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Parsea el `mode` opcional de `search` (`text` por defecto).
fn parse_mode(args: &Value) -> Result<SearchMode, ToolError> {
    match args.get("mode").and_then(|v| v.as_str()) {
        None | Some("text") => Ok(SearchMode::Text),
        Some("title") => Ok(SearchMode::Title),
        Some("tag") => Ok(SearchMode::Tag),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "mode inválido: {other}; debe ser text, title o tag"
        ))),
    }
}

/// Parsea el `occurrence` opcional de `replace` (`first` por defecto).
fn parse_occurrence(args: &Value) -> Result<Occurrence, ToolError> {
    match args.get("occurrence").and_then(|v| v.as_str()) {
        None | Some("first") => Ok(Occurrence::First),
        Some("all") => Ok(Occurrence::All),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "occurrence inválido: {other}; debe ser first o all"
        ))),
    }
}

/// Recorta `input` a `max` **caracteres** (nunca a bytes) y dice si hubo
/// recorte. Trabajar por caracteres evita partir un carácter UTF-8.
fn truncate_chars(input: &str, max: usize) -> (String, bool) {
    let mut chars = input.chars();
    let truncated: String = chars.by_ref().take(max).collect();
    let was_truncated = chars.next().is_some();
    (truncated, was_truncated)
}

/// Extrae la lista de notas de una respuesta de `GET /api/notes`, tolerando
/// tanto un array crudo como un objeto `{notes:[…]}`.
fn notes_array(value: &Value) -> Vec<Value> {
    if let Some(array) = value.as_array() {
        array.clone()
    } else if let Some(array) = value.get("notes").and_then(|v| v.as_array()) {
        array.clone()
    } else {
        Vec::new()
    }
}

/// Resumen recortado de una nota: **solo** `path`, `title`, `folder`, `tags` y
/// `updatedAt`, nunca el `body`.
fn note_summary(note: &Value) -> Value {
    json!({
        "path": note.get("path").and_then(|v| v.as_str()).unwrap_or(""),
        "title": note.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        "folder": note.get("folder").and_then(|v| v.as_str()).unwrap_or(""),
        "tags": note.get("tags").cloned().unwrap_or_else(|| json!([])),
        "updatedAt": note
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .or_else(|| note.get("updated_at").and_then(|v| v.as_str()))
            .unwrap_or(""),
    })
}

/// ¿La nota pasa los filtros opcionales de `list`?
fn matches_filters(
    note: &Value,
    folder: Option<&str>,
    subpath: Option<&str>,
    tag: Option<&str>,
) -> bool {
    if let Some(folder) = folder {
        let note_folder = note.get("folder").and_then(|v| v.as_str()).unwrap_or("");
        if !note_folder.eq_ignore_ascii_case(folder) {
            return false;
        }
    }
    if let Some(subpath) = subpath {
        let path = note.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if !path.to_lowercase().contains(&subpath.to_lowercase()) {
            return false;
        }
    }
    if let Some(tag) = tag {
        let has_tag = note
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|tags| {
                tags.iter()
                    .filter_map(|t| t.as_str())
                    .any(|t| t.eq_ignore_ascii_case(tag))
            })
            .unwrap_or(false);
        if !has_tag {
            return false;
        }
    }
    true
}

/// Herramienta multioperación sobre el vault de ZenNotes.
pub struct ZennotesTool {
    access: ZennotesAccess,
}

impl ZennotesTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            access: ZennotesAccess::new(db),
        }
    }

    async fn list(&self, args: Value) -> Result<ToolResult, ToolError> {
        // Todo lo que no necesita red se valida antes de resolver la config.
        let limit = parse_limit(&args)?;
        let folder = optional_str(&args, "folder");
        let subpath = optional_str(&args, "subpath");
        let tag = optional_str(&args, "tag");

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        let value = client
            .list_notes(&config.token)
            .await
            .map_err(zennotes_error)?;

        let filtered: Vec<Value> = notes_array(&value)
            .into_iter()
            .filter(|note| {
                matches_filters(note, folder.as_deref(), subpath.as_deref(), tag.as_deref())
            })
            .collect();
        let count = filtered.len();
        let notes: Vec<Value> = filtered.iter().take(limit).map(note_summary).collect();

        Ok(ToolResult {
            success: true,
            data: json!({ "count": count, "notes": notes }),
            message: Some(format!("{count} notas")),
        })
    }

    async fn search(&self, args: Value) -> Result<ToolResult, ToolError> {
        // Validación local antes de tocar la red.
        let query = required_str(&args, "query")?.to_string();
        let limit = parse_limit(&args)?;
        let mode = parse_mode(&args)?;

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());

        match mode {
            SearchMode::Text => {
                let value = client
                    .search_text(&config.token, &query)
                    .await
                    .map_err(zennotes_error)?;
                let all = value.as_array().cloned().unwrap_or_default();
                let count = all.len();
                let matches: Vec<Value> = all
                    .iter()
                    .take(limit)
                    .map(|hit| {
                        json!({
                            "path": hit.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                            "title": hit.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                            "lineNumber": hit.get("lineNumber").cloned().unwrap_or(Value::Null),
                            "lineText": hit.get("lineText").and_then(|v| v.as_str()).unwrap_or(""),
                        })
                    })
                    .collect();
                Ok(ToolResult {
                    success: true,
                    data: json!({ "count": count, "matches": matches }),
                    message: Some(format!("{count} coincidencias")),
                })
            }
            SearchMode::Title | SearchMode::Tag => {
                let value = client
                    .list_notes(&config.token)
                    .await
                    .map_err(zennotes_error)?;
                let needle = query.to_lowercase();
                let filtered: Vec<Value> = notes_array(&value)
                    .into_iter()
                    .filter(|note| match mode {
                        SearchMode::Title => note
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(|title| title.to_lowercase().contains(&needle))
                            .unwrap_or(false),
                        SearchMode::Tag => note
                            .get("tags")
                            .and_then(|v| v.as_array())
                            .map(|tags| {
                                tags.iter()
                                    .filter_map(|t| t.as_str())
                                    .any(|t| t.to_lowercase().contains(&needle))
                            })
                            .unwrap_or(false),
                        SearchMode::Text => false,
                    })
                    .collect();
                let count = filtered.len();
                let matches: Vec<Value> = filtered.iter().take(limit).map(note_summary).collect();
                Ok(ToolResult {
                    success: true,
                    data: json!({ "count": count, "matches": matches }),
                    message: Some(format!("{count} coincidencias")),
                })
            }
        }
    }

    async fn read(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        let note = client
            .read_note(&config.token, &path)
            .await
            .map_err(zennotes_error)?;

        let raw_body = note.get("body").and_then(|v| v.as_str()).unwrap_or("");
        let (body, truncated) = truncate_chars(raw_body, MAX_BODY_CHARS);

        Ok(ToolResult {
            success: true,
            data: json!({
                "path": note.get("path").and_then(|v| v.as_str()).unwrap_or(path.as_str()),
                "title": note.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "body": body,
                "truncated": truncated,
            }),
            message: None,
        })
    }

    async fn create(&self, args: Value) -> Result<ToolResult, ToolError> {
        let title = required_str(&args, "title")?.to_string();
        let body = optional_str(&args, "body");
        // `folder` es un enum restringido en el servidor: se valida antes de la
        // red para no provocar un error de servidor.
        let folder = parse_folder(&args, "folder", DEFAULT_CREATE_FOLDER)?;

        let config = self.access.resolve().await?;
        // El ajuste `zennotes_folder` (Valet) es la **subcarpeta** por defecto,
        // no la carpeta: la nota nace en `inbox/Valet/`.
        let subpath = optional_str(&args, "subpath").unwrap_or_else(|| config.folder.clone());
        let client = Zennotes::new(config.base_url.clone());

        let created = match body {
            Some(body) => {
                client
                    .create_with_body(&config.token, &folder, &title, Some(&subpath), &body)
                    .await
            }
            None => {
                client
                    .create(&config.token, &folder, &title, Some(&subpath))
                    .await
            }
        }
        .map_err(zennotes_error)?;

        let path = created
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| format!("{folder}/{subpath}"));
        Ok(ToolResult {
            success: true,
            data: created,
            message: Some(format!("Nota creada en {path}")),
        })
    }

    async fn append(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();
        let text = required_raw(&args, "text")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .append(&config.token, &path, &text)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Texto añadido a la nota".to_string()),
        })
    }

    async fn replace(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();
        let find = required_str(&args, "find")?.to_string();
        // Un `replace` vacío es una borradura literal: se admite.
        let with = args.get("replace").and_then(|v| v.as_str()).unwrap_or("");
        let occurrence = parse_occurrence(&args)?;

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .replace(&config.token, &path, &find, with, occurrence)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Texto reemplazado en la nota".to_string()),
        })
    }

    async fn write(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();
        let body = required_raw(&args, "body")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .write(&config.token, &path, &body)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Cuerpo de la nota sobrescrito".to_string()),
        })
    }

    async fn move_note(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();
        let target_folder = required_str(&args, "target_folder")?.to_string();
        if !VALID_FOLDERS.contains(&target_folder.as_str()) {
            return Err(ToolError::InvalidArguments(format!(
                "target_folder inválido: {target_folder}; debe ser inbox, quick o archive"
            )));
        }
        let target_subpath = optional_str(&args, "target_subpath");

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .move_note(
                &config.token,
                &path,
                &target_folder,
                target_subpath.as_deref(),
            )
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some(format!("Nota movida a {target_folder}")),
        })
    }

    async fn archive(&self, args: Value, archived: bool) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        if archived {
            client
                .archive(&config.token, &path)
                .await
                .map_err(zennotes_error)?;
        } else {
            client
                .unarchive(&config.token, &path)
                .await
                .map_err(zennotes_error)?;
        }

        let message = if archived {
            "Nota archivada"
        } else {
            "Nota desarchivada"
        };
        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some(message.to_string()),
        })
    }

    async fn rename(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();
        let new_title = required_str(&args, "new_title")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .rename(&config.token, &path, &new_title)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path, "title": new_title }),
            message: Some("Nota renombrada".to_string()),
        })
    }

    async fn trash(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .trash(&config.token, &path)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Nota enviada a la papelera".to_string()),
        })
    }

    async fn restore(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .restore(&config.token, &path)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Nota restaurada".to_string()),
        })
    }

    async fn delete(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = required_str(&args, "path")?.to_string();

        let config = self.access.resolve().await?;
        let client = Zennotes::new(config.base_url.clone());
        client
            .delete(&config.token, &path)
            .await
            .map_err(zennotes_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({ "path": path }),
            message: Some("Nota borrada definitivamente".to_string()),
        })
    }
}

#[async_trait]
impl Tool for ZennotesTool {
    fn name(&self) -> &'static str {
        "zennotes"
    }

    fn description(&self) -> &'static str {
        "Lee y escribe en el vault de ZenNotes (mis notas, artículos y apuntes \
         del servidor autoalojado). Multioperación: lee con `list` (metadatos, \
         sin cuerpo), `search` (por texto, título o etiqueta) y `read` (cuerpo \
         completo recortado a 20 000 caracteres); escribe con `create`, `append`, \
         `replace` y `move`; y gestiona el ciclo de vida con `archive`, \
         `unarchive`, `rename`, `trash`, `restore` y `delete`. `write` sobrescribe \
         el cuerpo entero. Al crear, la nota va a `inbox/Valet` por defecto \
         (carpeta `inbox` y subcarpeta del ajuste `zennotes_folder`)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "list",
                        "search",
                        "read",
                        "create",
                        "append",
                        "replace",
                        "write",
                        "move",
                        "archive",
                        "unarchive",
                        "rename",
                        "trash",
                        "restore",
                        "delete"
                    ],
                    "description": "Operación a ejecutar en el vault de ZenNotes"
                },
                "path": {
                    "type": "string",
                    "description": "Ruta relativa de la nota dentro del vault (obligatoria en read, append, replace, write, move, archive, unarchive, rename, trash, restore y delete)"
                },
                "query": {
                    "type": "string",
                    "description": "Texto o término a buscar (obligatorio en search)"
                },
                "mode": {
                    "type": "string",
                    "enum": ["text", "title", "tag"],
                    "description": "Modo de búsqueda de search (por defecto text)"
                },
                "title": {
                    "type": "string",
                    "description": "Título de la nota (obligatorio en create)"
                },
                "body": {
                    "type": "string",
                    "description": "Cuerpo en markdown de la nota (opcional en create, obligatorio en write)"
                },
                "text": {
                    "type": "string",
                    "description": "Texto a añadir al final del cuerpo (obligatorio en append)"
                },
                "find": {
                    "type": "string",
                    "description": "Texto literal a buscar (obligatorio en replace)"
                },
                "replace": {
                    "type": "string",
                    "description": "Texto por el que sustituir en replace; vacío para borrar"
                },
                "occurrence": {
                    "type": "string",
                    "enum": ["first", "all"],
                    "description": "Cuántas coincidencias sustituir en replace (por defecto first)"
                },
                "folder": {
                    "type": "string",
                    "enum": ["inbox", "quick", "archive"],
                    "description": "Carpeta del vault: filtro en list y carpeta destino en create (por defecto `inbox`). Solo se admiten `inbox`, `quick` y `archive`"
                },
                "subpath": {
                    "type": "string",
                    "description": "Subcarpeta dentro de la carpeta: filtro en list y subcarpeta destino en create (por defecto el ajuste `zennotes_folder`, `Valet`), de modo que la nota nace en `inbox/Valet/`"
                },
                "tag": {
                    "type": "string",
                    "description": "Etiqueta por la que filtrar en list"
                },
                "limit": {
                    "type": "integer",
                    "minimum": MIN_LIMIT,
                    "maximum": MAX_LIMIT,
                    "description": format!("Número máximo de notas o coincidencias a devolver ({DEFAULT_LIMIT} por defecto, entre {MIN_LIMIT} y {MAX_LIMIT})")
                },
                "target_folder": {
                    "type": "string",
                    "enum": ["inbox", "quick", "archive"],
                    "description": "Carpeta destino en move"
                },
                "target_subpath": {
                    "type": "string",
                    "description": "Subcarpeta destino en move (opcional)"
                },
                "new_title": {
                    "type": "string",
                    "description": "Nuevo título de la nota (obligatorio en rename)"
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|v| v.as_str()) {
            // Lecturas: no modifican nada.
            Some("list") | Some("search") | Some("read") => Permission::NoConfirm,
            // Escrituras aditivas o reversibles: avisan.
            Some("create") | Some("append") | Some("replace") | Some("move") | Some("archive")
            | Some("unarchive") | Some("rename") | Some("restore") => Permission::Notify,
            // Sobrescritura y borrado: irreversibles, aprobación explícita.
            Some("write") | Some("trash") | Some("delete") => Permission::ExplicitApproval,
            // Operación desconocida o ausente: fail-closed. `execute` la
            // rechazará antes de tocar la red.
            _ => Permission::ExplicitApproval,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("list") => self.list(args).await,
            Some("search") => self.search(args).await,
            Some("read") => self.read(args).await,
            Some("create") => self.create(args).await,
            Some("append") => self.append(args).await,
            Some("replace") => self.replace(args).await,
            Some("write") => self.write(args).await,
            Some("move") => self.move_note(args).await,
            Some("archive") => self.archive(args, true).await,
            Some("unarchive") => self.archive(args, false).await,
            Some("rename") => self.rename(args).await,
            Some("trash") => self.trash(args).await,
            Some("restore") => self.restore(args).await,
            Some("delete") => self.delete(args).await,
            Some(other) => Err(ToolError::InvalidArguments(format!(
                "Operación desconocida: {other}"
            ))),
            None => Err(ToolError::InvalidArguments(
                "Falta el argumento obligatorio `operation`".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    /// Config apuntando al servidor `wiremock` con la carpeta por defecto `Valet`.
    fn config_for(server: &MockServer) -> ZennotesConfig {
        ZennotesConfig {
            base_url: server.uri(),
            token: "test-token".to_string(),
            folder: "Valet".to_string(),
        }
    }

    /// Herramienta atada al servidor `wiremock`.
    async fn tool_for(server: &MockServer) -> ZennotesTool {
        let db = setup_db().await;
        ZennotesTool {
            access: ZennotesAccess::with_config(db, config_for(server)),
        }
    }

    /// Nota de metadatos tal como la devuelve `GET /api/notes`.
    fn note_json(path: &str, title: &str, folder: &str, tags: &[&str]) -> Value {
        json!({
            "path": path,
            "title": title,
            "folder": folder,
            "tags": tags,
            "updatedAt": "2026-10-26T10:00:00Z",
            "body": "no debería aparecer",
        })
    }

    // -----------------------------------------------------------------------
    // Superficie: nombre, esquema y permisos por operación
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_name() {
        let db = setup_db().await;
        assert_eq!(ZennotesTool::new(db).name(), "zennotes");
    }

    /// Scenario: `parameters()` declara el enum completo y `operation` required.
    #[tokio::test]
    async fn test_parameters_declares_full_operation_enum_and_required() {
        let db = setup_db().await;
        let params = ZennotesTool::new(db).parameters();

        assert_eq!(params["type"], "object");
        let ops: Vec<String> = params["properties"]["operation"]["enum"]
            .as_array()
            .expect("operation must declare an enum")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        for op in [
            "list",
            "search",
            "read",
            "create",
            "append",
            "replace",
            "write",
            "move",
            "archive",
            "unarchive",
            "rename",
            "trash",
            "restore",
            "delete",
        ] {
            assert!(
                ops.contains(&op.to_string()),
                "falta la operación `{op}`: {ops:?}"
            );
        }
        assert_eq!(
            ops.len(),
            14,
            "el enum debe tener catorce operaciones: {ops:?}"
        );
        assert_eq!(params["required"], json!(["operation"]));

        let properties = params["properties"].as_object().unwrap();
        for field in [
            "path",
            "query",
            "mode",
            "title",
            "body",
            "text",
            "find",
            "replace",
            "occurrence",
            "folder",
            "subpath",
            "tag",
            "limit",
            "target_folder",
            "target_subpath",
            "new_title",
        ] {
            assert!(
                properties.contains_key(field),
                "falta el argumento `{field}`"
            );
        }
    }

    /// Scenario: la tabla completa de permisos por operación.
    #[tokio::test]
    async fn test_permission_table() {
        let db = setup_db().await;
        let tool = ZennotesTool::new(db);

        for op in ["list", "search", "read"] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::NoConfirm,
                "`{op}` es de solo lectura"
            );
        }
        for op in [
            "create",
            "append",
            "replace",
            "move",
            "archive",
            "unarchive",
            "rename",
            "restore",
        ] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::Notify,
                "`{op}` es una escritura aditiva o reversible"
            );
        }
        for op in ["write", "trash", "delete"] {
            assert_eq!(
                tool.permission(&json!({ "operation": op })),
                Permission::ExplicitApproval,
                "`{op}` es irreversible"
            );
        }
        // Operación desconocida o ausente: fail-closed.
        assert_eq!(
            tool.permission(&json!({ "operation": "empty-trash" })),
            Permission::ExplicitApproval
        );
        assert_eq!(tool.permission(&json!({})), Permission::ExplicitApproval);
    }

    // -----------------------------------------------------------------------
    // list — recorte obligatorio
    // -----------------------------------------------------------------------

    /// Scenario: el listado va acotado, con campos mínimos y sin `body`.
    #[tokio::test]
    async fn test_list_is_bounded_and_has_minimal_fields() {
        let server = MockServer::start().await;
        let notes: Vec<Value> = (0..25)
            .map(|i| {
                note_json(
                    &format!("Valet/{i}.md"),
                    &format!("Nota {i}"),
                    "Valet",
                    &["x"],
                )
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/api/notes"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(notes)))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "list", "limit": 5 }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["count"], 25, "count es el total tras filtro");
        let notes = result.data["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 5, "como mucho `limit` notas");
        for note in notes {
            assert!(note.get("path").is_some());
            assert!(note.get("title").is_some());
            assert!(note.get("folder").is_some());
            assert!(note.get("tags").is_some());
            assert!(note.get("updatedAt").is_some());
            assert!(
                note.get("body").is_none(),
                "el listado nunca incluye el body"
            );
        }
    }

    /// Scenario: el `limit` por defecto es 20.
    #[tokio::test]
    async fn test_list_default_limit_is_twenty() {
        let server = MockServer::start().await;
        let notes: Vec<Value> = (0..25)
            .map(|i| note_json(&format!("Valet/{i}.md"), &format!("Nota {i}"), "Valet", &[]))
            .collect();
        Mock::given(method("GET"))
            .and(path("/api/notes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(notes)))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool.execute(json!({ "operation": "list" })).await.unwrap();

        assert_eq!(result.data["count"], 25);
        assert_eq!(result.data["notes"].as_array().unwrap().len(), 20);
    }

    /// Scenario: un limit fuera de rango es inválido y no sale a la red.
    #[tokio::test]
    async fn test_list_invalid_limit_without_request() {
        let server = MockServer::start().await;
        let tool = tool_for(&server).await;

        for limit in [0, 500] {
            let err = tool
                .execute(json!({ "operation": "list", "limit": limit }))
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un limit inválido no debe enviar ninguna petición"
        );
    }

    /// `list` filtra por carpeta localmente.
    #[tokio::test]
    async fn test_list_filters_by_folder() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                note_json("Valet/A.md", "A", "Valet", &[]),
                note_json("inbox/B.md", "B", "inbox", &[]),
            ])))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "list", "folder": "inbox" }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], 1);
        assert_eq!(result.data["notes"][0]["title"], "B");
    }

    // -----------------------------------------------------------------------
    // read — recorte del cuerpo
    // -----------------------------------------------------------------------

    /// Scenario: el cuerpo se recorta a 20 000 caracteres y se indica.
    #[tokio::test]
    async fn test_read_truncates_body_to_limit() {
        let server = MockServer::start().await;
        let long_body = "á".repeat(25_000);
        Mock::given(method("GET"))
            .and(path("/api/notes/read"))
            .and(query_param("path", "Valet/Larga.md"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "path": "Valet/Larga.md",
                "title": "Larga",
                "body": long_body,
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "read", "path": "Valet/Larga.md" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["title"], "Larga");
        assert_eq!(
            result.data["body"].as_str().unwrap().chars().count(),
            MAX_BODY_CHARS
        );
        assert_eq!(result.data["truncated"], true);
    }

    /// Un cuerpo corto no se recorta ni indica truncamiento.
    #[tokio::test]
    async fn test_read_short_body_is_not_truncated() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes/read"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "path": "Valet/Corta.md",
                "title": "Corta",
                "body": "# Hola",
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "read", "path": "Valet/Corta.md" }))
            .await
            .unwrap();

        assert_eq!(result.data["body"], "# Hola");
        assert_eq!(result.data["truncated"], false);
    }

    // -----------------------------------------------------------------------
    // create — carpeta por defecto y dos viajes
    // -----------------------------------------------------------------------

    /// Scenario: create sin destino va a `inbox` con el subpath `Valet` (el
    /// ajuste `zennotes_folder`), porque el servidor solo acepta
    /// `folder` ∈ {inbox, quick, archive} y `Valet` es en realidad un subpath.
    #[tokio::test]
    async fn test_create_without_folder_uses_inbox_and_valet_subpath() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(body_json(json!({
                "folder": "inbox", "title": "Bitácora", "subpath": "Valet"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "path": "Valet/Bitácora.md", "title": "Bitácora"
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "create", "title": "Bitácora" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["path"], "Valet/Bitácora.md");
    }

    /// Un `folder` fuera del enum del servidor (`inbox`|`quick`|`archive`) se
    /// rechaza **sin salir a la red**; el servidor devolvería un 500.
    #[tokio::test]
    async fn test_create_invalid_folder_is_invalid_without_request() {
        let server = MockServer::start().await;
        let tool = tool_for(&server).await;

        // `Valet` es el subpath por defecto, no un `folder` válido.
        for folder in ["Valet", "Papelera", "trash"] {
            let err = tool
                .execute(json!({ "operation": "create", "title": "X", "folder": folder }))
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un folder inválido no debe enviar ninguna petición"
        );
    }

    /// Un `subpath` explícito manda sobre el ajuste `zennotes_folder`.
    #[tokio::test]
    async fn test_create_explicit_subpath_wins_over_default() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(body_json(json!({
                "folder": "inbox", "title": "X", "subpath": "web/articulos"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "path": "web/articulos/X.md", "title": "X"
            })))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({
                "operation": "create", "title": "X", "subpath": "web/articulos"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["path"], "web/articulos/X.md");
    }

    /// Scenario: create con cuerpo hace dos viajes (create + write).
    #[tokio::test]
    async fn test_create_with_body_makes_two_trips() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(body_json(json!({
                "folder": "inbox", "title": "X", "subpath": "Valet"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "path": "Valet/X.md", "title": "X"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/notes/write"))
            .and(body_json(json!({ "path": "Valet/X.md", "body": "# Hola" })))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "create", "title": "X", "body": "# Hola" }))
            .await
            .unwrap();

        assert!(result.success);
        let paths: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| r.url.path().to_string())
            .collect();
        assert_eq!(paths.len(), 2, "create con cuerpo son create + write");
        assert!(paths.contains(&"/api/notes/create".to_string()));
        assert!(paths.contains(&"/api/notes/write".to_string()));
    }

    // -----------------------------------------------------------------------
    // search — texto
    // -----------------------------------------------------------------------

    /// Scenario: una búsqueda de texto devuelve coincidencias con `lineText`.
    #[tokio::test]
    async fn test_search_text_returns_line_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search/text"))
            .and(query_param("q", "rust"))
            .and(query_param("backend", "auto"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "path": "Valet/Rust.md",
                    "title": "Rust",
                    "folder": "Valet",
                    "lineNumber": 3,
                    "offset": 12,
                    "lineText": "me gusta rust"
                }
            ])))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "search", "query": "rust" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["count"], 1);
        let matches = result.data["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["lineText"], "me gusta rust");
        assert_eq!(matches[0]["lineNumber"], 3);
        assert_eq!(matches[0]["path"], "Valet/Rust.md");
    }

    /// Scenario: `search` con `query` en blanco es inválido y no sale a la red.
    #[tokio::test]
    async fn test_search_blank_query_without_request() {
        let server = MockServer::start().await;
        let tool = tool_for(&server).await;

        for args in [
            json!({ "operation": "search" }),
            json!({ "operation": "search", "query": "  " }),
        ] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un query inválido no debe enviar ninguna petición"
        );
    }

    /// `search` con `mode=title` filtra localmente sobre el listado.
    #[tokio::test]
    async fn test_search_title_filters_locally() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/notes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                note_json("Valet/Rust.md", "Rust en serio", "Valet", &[]),
                note_json("Valet/Otro.md", "Otra cosa", "Valet", &[]),
            ])))
            .mount(&server)
            .await;

        let tool = tool_for(&server).await;
        let result = tool
            .execute(json!({ "operation": "search", "query": "rust", "mode": "title" }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], 1);
        assert_eq!(result.data["matches"][0]["title"], "Rust en serio");
        let requests = server.received_requests().await.unwrap();
        assert!(
            requests.iter().all(|r| r.url.path() == "/api/notes"),
            "mode=title no debe llamar al endpoint de búsqueda de texto"
        );
    }

    // -----------------------------------------------------------------------
    // Errores de argumentos — sin red
    // -----------------------------------------------------------------------

    /// Scenario: operación desconocida y argumentos ausentes → InvalidArguments.
    #[tokio::test]
    async fn test_unknown_operation_and_missing_args_are_invalid() {
        let server = MockServer::start().await;
        let tool = tool_for(&server).await;

        for args in [
            json!({ "operation": "empty-trash" }),
            json!({ "operation": "read" }),
            json!({ "operation": "write", "path": "Valet/X.md" }),
            json!({ "operation": "create" }),
            json!({ "operation": "rename", "path": "Valet/X.md" }),
            json!({}),
        ] {
            let err = tool.execute(args).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments(_)), "{err:?}");
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "los argumentos inválidos no deben enviar ninguna petición"
        );
    }
}
