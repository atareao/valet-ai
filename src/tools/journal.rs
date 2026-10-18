//! La herramienta `journal` del agente: la bitácora personal.
//!
//! Cinco operaciones sobre la tabla `journal_entries`, siempre delegando en
//! [`JournalRepo`]: leer el día (`get_entry`), apuntarlo o sobrescribirlo
//! (`write_entry`), corregirlo en parte (`update_entry`), listar un rango
//! (`list_entries`) y borrarlo (`delete_entry`).
//!
//! Dos reglas mandan sobre el resto:
//!
//! - **El día es la clave.** `date` es obligatoria para escribir, corregir y
//!   borrar; en `get_entry` es opcional y, sin ella, se lee **el día de hoy** en
//!   la zona del usuario (`settings.timezone`, `Europe/Madrid` por defecto).
//! - **Se valida antes de tocar la base.** Una fecha que no encaje en
//!   `YYYY-MM-DD` o un ánimo fuera del conjunto cerrado son
//!   [`ToolError::InvalidArguments`] y no llegan a consultar ni a modificar la
//!   bitácora.
//!
//! Borrar es lo único destructivo —es el diario del usuario—, así que solo
//! `delete_entry` declara [`Permission::ExplicitApproval`]; el resto va sin
//! confirmación.

use async_trait::async_trait;
use chrono::Utc;
use chrono_tz::Tz;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::str::FromStr;

use crate::db::repos::journal::JournalRepo;
use crate::db::repos::settings::SettingsRepo;
use crate::models::journal::{JournalEntry, Mood};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Zona horaria con la que se resuelve «hoy» cuando el perfil no declara
/// ninguna, como en `get_current_time`.
const DEFAULT_TIMEZONE: &str = "Europe/Madrid";

/// Cuántas entradas devuelve `list_entries` cuando no se indica `limit`.
const DEFAULT_LIST_LIMIT: i64 = 50;

/// La herramienta `journal`.
pub struct JournalTool {
    db: SqlitePool,
}

impl JournalTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// El día de «hoy» en la zona del usuario: la entrada de la bitácora se
    /// direcciona por día local, no por día UTC.
    ///
    /// Una zona horaria inválida cae a UTC en vez de tumbar la lectura, como en
    /// el resto del proyecto.
    async fn today(&self) -> String {
        let timezone = SettingsRepo::get(&self.db, "timezone")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string());
        let tz = Tz::from_str(&timezone).unwrap_or(chrono_tz::UTC);
        Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string()
    }

    /// `get_entry` — la entrada del día, o el aviso de que ese día no está
    /// escrito. Un día sin entrada no es un error.
    async fn get_entry(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = profile_id(&args);
        let date = match optional_str(&args, "date") {
            Some(raw) => parse_date(raw)?,
            None => self.today().await,
        };

        match JournalRepo::get(&self.db, &profile_id, &date).await? {
            Some(entry) => Ok(ToolResult {
                success: true,
                data: serde_json::to_value(&entry).map_err(serialize_error)?,
                message: Some(format!("Entrada de la bitácora del {date}")),
            }),
            None => Ok(ToolResult {
                success: true,
                data: Value::Null,
                message: Some(format!(
                    "No existe ninguna entrada de la bitácora para el {date}"
                )),
            }),
        }
    }

    /// `write_entry` — crea la entrada del día o **sobrescribe** la que hubiera:
    /// el «apúntame esto».
    async fn write_entry(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = profile_id(&args);
        let date = required_date(&args, "date")?;
        let content = required_content(&args, "write_entry")?;
        let mood = optional_mood(&args)?;
        let achievements = optional_string_list(&args, "achievements")?.unwrap_or_default();
        let struggles = optional_string_list(&args, "struggles")?.unwrap_or_default();
        let tags = optional_string_list(&args, "tags")?.unwrap_or_default();

        // El `id` es interno y los sellos de tiempo los pone la base de datos:
        // aquí solo viaja lo que el usuario ha contado.
        let entry = JournalEntry {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id,
            date,
            mood,
            content,
            achievements,
            struggles,
            tags,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let stored = JournalRepo::upsert(&self.db, &entry).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&stored).map_err(serialize_error)?,
            message: Some(format!(
                "Entrada de la bitácora del {} guardada",
                stored.date
            )),
        })
    }

    /// `update_entry` — **parcial**: solo cambia los campos que le llegan y deja
    /// intactos los demás. Un día sin entrada no se puede corregir: no hay nada
    /// que tocar.
    async fn update_entry(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = profile_id(&args);
        let date = required_date(&args, "date")?;

        let mut entry = JournalRepo::get(&self.db, &profile_id, &date)
            .await?
            .ok_or_else(|| {
                ToolError::NotFound(format!(
                    "No existe ninguna entrada de la bitácora para el {date}"
                ))
            })?;

        if let Some(raw) = args.get("content").and_then(|value| value.as_str()) {
            let content = raw.trim();
            if content.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "`content` no puede estar vacío en `update_entry`".to_string(),
                ));
            }
            entry.content = content.to_string();
        }
        if let Some(mood) = optional_mood(&args)? {
            entry.mood = Some(mood);
        }
        if let Some(achievements) = optional_string_list(&args, "achievements")? {
            entry.achievements = achievements;
        }
        if let Some(struggles) = optional_string_list(&args, "struggles")? {
            entry.struggles = struggles;
        }
        if let Some(tags) = optional_string_list(&args, "tags")? {
            entry.tags = tags;
        }

        let stored = JournalRepo::upsert(&self.db, &entry).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&stored).map_err(serialize_error)?,
            message: Some(format!(
                "Entrada de la bitácora del {} actualizada",
                stored.date
            )),
        })
    }

    /// `list_entries` — el rango inclusivo `[from, to]`, de la entrada más
    /// reciente a la más antigua y como mucho `limit` entradas.
    async fn list_entries(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = profile_id(&args);
        let from = required_date(&args, "from")?;
        let to = required_date(&args, "to")?;
        let limit = optional_limit(&args)?;

        let entries = JournalRepo::list(&self.db, &profile_id, &from, &to, limit).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&entries).map_err(serialize_error)?,
            message: Some(format!(
                "{} entradas de la bitácora entre el {from} y el {to}",
                entries.len()
            )),
        })
    }

    /// `delete_entry` — borra la entrada del día. Un día sin entrada no es un
    /// error: se informa de que no existe, como en el timeline.
    async fn delete_entry(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = profile_id(&args);
        let date = required_date(&args, "date")?;

        let deleted = JournalRepo::delete(&self.db, &profile_id, &date).await?;

        Ok(ToolResult {
            success: true,
            data: json!({ "date": date, "deleted": deleted }),
            message: Some(if deleted {
                format!("Entrada de la bitácora del {date} borrada")
            } else {
                format!("No existe ninguna entrada de la bitácora para el {date}")
            }),
        })
    }
}

#[async_trait]
impl Tool for JournalTool {
    fn name(&self) -> &'static str {
        "journal"
    }

    fn description(&self) -> &'static str {
        "Lee, apunta, corrige, lista y borra las entradas de tu bitácora personal (el diario): una \
         entrada por día, con un relato y, si se quiere, un ánimo, logros, dificultades y etiquetas."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["get_entry", "write_entry", "update_entry", "delete_entry", "list_entries"],
                    "description": "Acción a realizar. `date` es obligatoria para `write_entry`, \
                        `update_entry` y `delete_entry`; en `get_entry` es opcional y sin ella se \
                        lee el día de hoy. `content` es obligatorio para `write_entry`. `from` y \
                        `to` son obligatorios para `list_entries`."
                },
                "date": {
                    "type": "string",
                    "description": "Fecha de la entrada, en formato YYYY-MM-DD"
                },
                "content": {
                    "type": "string",
                    "description": "El relato del día (obligatorio en write_entry)"
                },
                "mood": {
                    "type": "string",
                    "enum": ["great", "good", "neutral", "bad", "terrible"],
                    "description": "Ánimo del día (opcional)"
                },
                "achievements": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Logros del día (opcional)"
                },
                "struggles": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Dificultades del día (opcional)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Etiquetas del día (opcional)"
                },
                "from": {
                    "type": "string",
                    "description": "Fecha inicial del rango, en formato YYYY-MM-DD (obligatoria en list_entries)"
                },
                "to": {
                    "type": "string",
                    "description": "Fecha final del rango, en formato YYYY-MM-DD (obligatoria en list_entries)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Número máximo de entradas a devolver (por defecto 50)"
                },
                "profile_id": {
                    "type": "string",
                    "description": "ID del perfil (por defecto 'default')"
                }
            },
            "required": ["operation"]
        })
    }

    /// Borrar el diario es lo único destructivo: pide aprobación explícita. Leer
    /// y escribir no.
    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|value| value.as_str()) {
            Some("delete_entry") => Permission::ExplicitApproval,
            _ => Permission::NoConfirm,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let operation = args
            .get("operation")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        match operation {
            "get_entry" => self.get_entry(args).await,
            "write_entry" => self.write_entry(args).await,
            "update_entry" => self.update_entry(args).await,
            "delete_entry" => self.delete_entry(args).await,
            "list_entries" => self.list_entries(args).await,
            other => Err(ToolError::InvalidArguments(format!(
                "Operación desconocida: '{other}'; debe ser una de get_entry, write_entry, \
                 update_entry, delete_entry o list_entries"
            ))),
        }
    }
}

/// El `profile_id` del turno: opcional, `default` cuando falta o viene en
/// blanco, como en las herramientas vecinas.
fn profile_id(args: &Value) -> String {
    optional_str(args, "profile_id")
        .unwrap_or("default")
        .to_string()
}

/// Un campo de texto presente y no vacío, o `None`.
fn optional_str<'a>(args: &'a Value, field: &str) -> Option<&'a str> {
    args.get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Una fecha **obligatoria**: ausente, en blanco o mal formada es un argumento
/// inválido, sin llegar a la base de datos.
fn required_date(args: &Value, field: &str) -> Result<String, ToolError> {
    match optional_str(args, field) {
        Some(raw) => parse_date(raw),
        None => Err(ToolError::InvalidArguments(format!(
            "`{field}` es obligatoria y debe ser una fecha ISO en formato YYYY-MM-DD"
        ))),
    }
}

/// Valida una fecha `YYYY-MM-DD` de forma **estricta**: diez caracteres, los
/// guiones en su sitio y un día que exista de verdad.
///
/// La comprobación de forma es a mano a propósito: `%m`/`%d` de `chrono`
/// aceptan uno o dos dígitos, así que `2026-1-1` pasaría el parseo y luego
/// rompería el orden lexicográfico —que para esta tabla *es* el orden
/// cronológico—.
fn parse_date(raw: &str) -> Result<String, ToolError> {
    let bytes = raw.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    let exists = shaped && chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_ok();

    if !exists {
        return Err(ToolError::InvalidArguments(format!(
            "`date` inválida: '{raw}'; debe ser una fecha ISO en formato YYYY-MM-DD"
        )));
    }
    Ok(raw.to_string())
}

/// El ánimo, si viene: `null` y ausente significan «sin ánimo», y cualquier otra
/// cosa que no sea uno de los cinco valores del conjunto cerrado es un argumento
/// inválido.
fn optional_mood(args: &Value) -> Result<Option<Mood>, ToolError> {
    match args.get("mood") {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => serde_json::from_value::<Mood>(raw.clone())
            .map(Some)
            .map_err(|_| {
                ToolError::InvalidArguments(format!(
                    "`mood` inválido: {raw}; debe ser uno de great, good, neutral, bad o terrible"
                ))
            }),
    }
}

/// Una lista de textos opcional: ausente o `null` es «no la toques», una lista
/// de cadenas es el valor nuevo, y cualquier otra forma es un argumento
/// inválido.
fn optional_string_list(args: &Value, field: &str) -> Result<Option<Vec<String>>, ToolError> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(|text| text.to_string()).ok_or_else(|| {
                    ToolError::InvalidArguments(format!(
                        "`{field}` debe ser una lista de textos; contiene {item}"
                    ))
                })
            })
            .collect::<Result<Vec<String>, ToolError>>()
            .map(Some),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "`{field}` debe ser una lista de textos, no {other}"
        ))),
    }
}

/// El relato obligatorio de `write_entry`: presente y con algo que contar.
fn required_content(args: &Value, operation: &str) -> Result<String, ToolError> {
    match optional_str(args, "content") {
        Some(content) => Ok(content.to_string()),
        None => Err(ToolError::InvalidArguments(format!(
            "`content` es obligatorio para `{operation}` y no puede estar vacío"
        ))),
    }
}

/// El tope de `list_entries`: `50` por defecto, y un valor que no sea un entero
/// positivo se rechaza en vez de recortar el resultado en silencio.
fn optional_limit(args: &Value) -> Result<i64, ToolError> {
    match args.get("limit") {
        None | Some(Value::Null) => Ok(DEFAULT_LIST_LIMIT),
        Some(value) => match value.as_i64() {
            Some(limit) if limit > 0 => Ok(limit),
            _ => Err(ToolError::InvalidArguments(format!(
                "`limit` debe ser un entero positivo, no {value}"
            ))),
        },
    }
}

/// Un fallo al serializar la entrada para el modelo. No debería ocurrir —el
/// modelo serializa a JSON por construcción—, pero se informa como error de
/// ejecución en vez de tragarse el dato.
fn serialize_error(error: serde_json::Error) -> ToolError {
    ToolError::ExecutionError(format!("no se pudo serializar la entrada: {error}"))
}
