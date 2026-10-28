//! Herramienta `tasks`: gestión de tareas con **dos backends**.
//!
//! Si hay token de OpenTodo configurado (`opentodo_token` o `OPENTODO_TOKEN`),
//! todas las operaciones van contra la API de OpenTodo a través de
//! [`crate::services::opentodo`]. Si no lo hay, la herramienta conserva
//! exactamente el comportamiento local vigente sobre la tabla `tasks` (con el
//! `profile_id` que inyecta el orquestador), de modo que desplegar la
//! integración sin configurar el token no cambia nada.
//!
//! El servidor de OpenTodo **no filtra**: `GET /api/v1/tasks` devuelve la lista
//! completa y las vistas del GTD se recalculan en Rust con
//! [`crate::services::opentodo::matches_view_as`]. Por eso las lecturas
//! devuelven como mucho `limit` tareas (20 por defecto, tope 50) con una
//! proyección mínima más un resumen agregado, y nunca la lista cruda.
//!
//! El modelo no maneja nunca ids de etiqueta: `set_label` recibe grupo y valor y
//! la fusión la hace el servicio.

use async_trait::async_trait;
use chrono::{Local, NaiveDate, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

use crate::db::repos::tasks::{Task, TasksRepo};
use crate::services::opentodo as ot;
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// `limit` por defecto de `list_tasks` y `search_tasks`.
const DEFAULT_LIMIT: usize = 20;

/// Cota inferior aceptada para `limit`.
const MIN_LIMIT: u64 = 1;

/// Cota superior aceptada para `limit`.
const MAX_LIMIT: u64 = 50;

/// «Hoy» en la hora local del servidor: las vistas del GTD se calculan contra el
/// día del usuario, no contra UTC.
fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Valida el `limit` opcional (`1..=50`, 20 si se omite). No toca la red.
fn parse_limit(args: &Value) -> Result<usize, ToolError> {
    let Some(raw) = args.get("limit") else {
        return Ok(DEFAULT_LIMIT);
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

/// Parsea la vista opcional (`all` por defecto). No toca la red.
///
/// Un `view` que no sea cadena es un argumento inválido, no «sin filtro».
fn parse_view(args: &Value) -> Result<ot::View, ToolError> {
    match args.get("view") {
        None | Some(Value::Null) => Ok(ot::View::All),
        Some(Value::String(raw)) => ot::View::parse(raw).ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "view inválido: {raw}; debe ser all, inbox, today, upcoming, anytime, \
                 someday, doing o waiting"
            ))
        }),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "view inválido: {other}; debe ser una cadena"
        ))),
    }
}

/// Parsea la prioridad opcional (`default` si se omite). No toca la red.
fn parse_priority(args: &Value, default: ot::Priority) -> Result<ot::Priority, ToolError> {
    match args.get("priority") {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(raw)) => ot::Priority::parse(raw).ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "priority inválida: {raw}; debe ser critical, high, medium o low"
            ))
        }),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "priority inválida: {other}; debe ser una cadena"
        ))),
    }
}

/// Parsea el `start` de `update_task`, distinguindo las tres intenciones:
/// ausente = no se toca, `null` = se limpia, cadena no vacía = se fija.
///
/// Cualquier otra cosa (incluida la cadena vacía) es un argumento inválido: hoy
/// `{"start": true}` o `{"start": ""}` borraban el campo en silencio.
fn parse_start(args: &Value) -> Result<Option<Option<String>>, ToolError> {
    match args.get("start") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(raw)) if !raw.trim().is_empty() => {
            Ok(Some(Some(raw.trim().to_string())))
        }
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "start inválido: {other}; usa null para limpiarlo o una cadena no vacía \
             (por ejemplo «someday»)"
        ))),
    }
}

/// Parsea el `due` con la misma semántica que [`parse_start`]: ausente, `null`
/// para limpiarlo o una cadena (ISO o expresión natural) para fijarlo.
fn parse_due_arg(args: &Value) -> Result<Option<Option<&str>>, ToolError> {
    match args.get("due") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(raw)) if !raw.trim().is_empty() => Ok(Some(Some(raw.trim()))),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "due inválido: {other}; usa null para limpiarlo, una fecha ISO (YYYY-MM-DD) o una \
             expresión como «el viernes a las 10»"
        ))),
    }
}

/// Parsea el par `group` + `value` de `set_label`, validando que el valor
/// pertenezca al grupo. Un `value` ausente o `null` significa quitarla. No toca
/// la red: un valor de otro grupo es un argumento inválido.
fn parse_group_and_value(args: &Value) -> Result<(ot::LabelGroup, Option<String>), ToolError> {
    let raw_group = args
        .get("group")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            ToolError::InvalidArguments(
                "`group` es obligatorio para `set_label`: state o scope".to_string(),
            )
        })?;
    let group = ot::LabelGroup::parse(raw_group).ok_or_else(|| {
        ToolError::InvalidArguments(format!(
            "group inválido: {raw_group}; debe ser state o scope"
        ))
    })?;

    match args.get("value") {
        None | Some(Value::Null) => Ok((group, None)),
        Some(value) => {
            let raw = value.as_str().ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "value inválido: {value}; debe ser una cadena o null"
                ))
            })?;
            let value = raw.trim();
            if !group.accepts(value) {
                return Err(ToolError::InvalidArguments(format!(
                    "value inválido: {value}; el grupo `{}` admite {}",
                    group.as_str(),
                    group.labels().join(" o ")
                )));
            }
            Ok((group, Some(value.to_string())))
        }
    }
}

/// Valida los argumentos que **no** deben llegar a la red y que valen para los
/// dos backends: el `limit` y el tipo de `view`/`project` de las lecturas, más
/// el par grupo/valor de `set_label`.
///
/// Lo específico del contrato de OpenTodo (`title`, `due`, `start`, la prioridad
/// de cuatro valores) se valida **dentro** de cada operación de OpenTodo, antes
/// de su primer viaje: el backend local conserva su contrato vigente
/// (`content`, `status`, `scope`, `due_date`, `profile_id`) y no puede verse
/// afectado por estas reglas.
fn validate_args(operation: &str, args: &Value) -> Result<(), ToolError> {
    match operation {
        "list_tasks" => {
            parse_limit(args)?;
            parse_view(args)?;
            optional_str(args, "project")?;
        }
        "search_tasks" => {
            parse_limit(args)?;
            required_str(args, "query")?;
        }
        "set_label" => {
            parse_group_and_value(args)?;
        }
        _ => {}
    }
    Ok(())
}

/// Extrae un argumento de cadena obligatorio y no en blanco tras `trim`.
///
/// Distingue «falta» de «tipo equivocado»: `{"title": 7}` no es un título
/// ausente, es un argumento inválido.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => {
            Err(ToolError::InvalidArguments(format!("{key} es obligatorio")))
        }
        Some(Value::String(raw)) if !raw.trim().is_empty() => Ok(raw.trim()),
        Some(Value::String(_)) => Err(ToolError::InvalidArguments(format!(
            "{key} es obligatorio y no puede estar en blanco"
        ))),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "{key} inválido: {other}; debe ser una cadena"
        ))),
    }
}

/// Extrae un argumento de cadena opcional: ausente, `null` o vacío → `None`,
/// cadena → su valor recortado, cualquier otro tipo → argumento inválido.
///
/// No tratar un tipo equivocado como ausente es justamente lo que evita que
/// `{"project": 7}` se resuelva como «sin filtro».
fn optional_str(args: &Value, key: &str) -> Result<Option<String>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            Ok(Some(raw.trim().to_string()).filter(|value| !value.is_empty()))
        }
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "{key} inválido: {other}; debe ser una cadena"
        ))),
    }
}

/// Traduce un [`ot::OpentodoError`] a un [`ToolError`] legible.
///
/// El detalle crudo de [`ot::OpentodoError::Upstream`] se descarta a propósito:
/// el mensaje del `400` ya viene redactado por el servicio (nunca contiene el
/// token) y lo demás no aporta nada al modelo.
fn opentodo_error(error: ot::OpentodoError) -> ToolError {
    match error {
        ot::OpentodoError::NotConfigured => ToolError::ExecutionError(
            "No hay token de OpenTodo configurado. Añádelo en Ajustes → Integraciones → \
             OpenTodo, o define la variable de entorno OPENTODO_TOKEN."
                .to_string(),
        ),
        ot::OpentodoError::Unauthorized => ToolError::ExecutionError(
            "El token de OpenTodo no es válido o ha caducado.".to_string(),
        ),
        ot::OpentodoError::NotFound => {
            ToolError::NotFound("La tarea indicada no existe en OpenTodo.".to_string())
        }
        ot::OpentodoError::InvalidInput(detail) => {
            ToolError::InvalidArguments(format!("OpenTodo rechazó la petición: {detail}"))
        }
        ot::OpentodoError::Upstream(_) => {
            ToolError::ExecutionError("OpenTodo no responde en este momento.".to_string())
        }
    }
}

/// Id del usuario autenticado dentro de `GET /api/v1/me`.
///
/// El servicio devuelve ese JSON sin tipar, así que se aceptan las dos formas
/// razonables: `{"id": …}` y `{"user": {"id": …}}`.
fn user_id_from_me(me: &Value) -> Option<String> {
    me.get("id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            me.get("user")
                .and_then(|user| user.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(str::to_string)
}

/// Mapa `project_id` → nombre.
fn project_names(projects: &[ot::Project]) -> BTreeMap<String, String> {
    projects
        .iter()
        .map(|project| (project.id.clone(), project.name.clone()))
        .collect()
}

/// `due` como texto para el modelo: `YYYY-MM-DD` o `YYYY-MM-DDTHH:MM`.
fn due_text(due: Option<&ot::Due>) -> Option<String> {
    due.map(|due| match &due.time {
        Some(time) => format!("{}T{}", due.date, time),
        None => due.date.clone(),
    })
}

/// Nombres de las etiquetas de una tarea, **solo** los que se pueden resolver:
/// los que el servicio resolvió desde el catálogo (`label_names`) y los que la
/// instancia devuelve ya como nombre de una etiqueta del catálogo.
///
/// Un id que no resuelve se descarta: nunca se devuelve un id como si fuera un
/// nombre.
fn task_label_names(task: &ot::Task, known_labels: &BTreeSet<String>) -> Vec<String> {
    let mut names = task.label_names.clone();
    for label in &task.labels {
        if known_labels.contains(label) && !names.contains(label) {
            names.push(label.clone());
        }
    }
    names
}

/// Proyección mínima de una tarea: ni ids de etiqueta ni la lista cruda.
///
/// `known_project` se usa cuando quien llama ya conoce el nombre del proyecto
/// (por ejemplo tras resolverlo al crearla) y evita una resolución extra.
fn task_json(
    task: &ot::Task,
    projects: &BTreeMap<String, String>,
    known_labels: &BTreeSet<String>,
    known_project: Option<&str>,
) -> Value {
    let project = match known_project {
        Some(name) => Some(name.to_string()),
        None => task
            .project_id
            .as_deref()
            .and_then(|id| projects.get(id))
            .cloned(),
    };

    json!({
        "id": task.id,
        "title": task.title,
        "project": project,
        "due": due_text(task.due.as_ref()),
        "priority": task.priority_label(),
        "start": task.start,
        "labels": task_label_names(task, known_labels),
    })
}

/// Lo que necesitan las lecturas: el listado con los nombres de etiqueta ya
/// resueltos, el catálogo de proyectos y los nombres de etiqueta que existen.
struct Snapshot {
    tasks: Vec<ot::Task>,
    projects: Vec<ot::Project>,
    known_labels: BTreeSet<String>,
}

impl Snapshot {
    /// Mapa `project_id` → nombre, listo para la proyección.
    fn names(&self) -> BTreeMap<String, String> {
        project_names(&self.projects)
    }
}

pub struct TasksTool {
    db: SqlitePool,
}

impl TasksTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Cliente de OpenTodo con la configuración resuelta **en cada llamada**.
    async fn open_todo(&self) -> Result<ot::Opentodo, ToolError> {
        ot::Opentodo::from_settings(&self.db)
            .await
            .map_err(opentodo_error)
    }

    /// Id del usuario autenticado, para el predicado de `waiting`.
    ///
    /// Best-effort: si `GET /api/v1/me` falla, la lectura sigue funcionando y
    /// solo se pierde la condición de «asignada a otro».
    async fn current_user_id(client: &ot::Opentodo) -> Option<String> {
        match client.me().await {
            Ok(me) => user_id_from_me(&me),
            Err(error) => {
                tracing::warn!("no se pudo leer el perfil de OpenTodo: {error}");
                None
            }
        }
    }

    /// Listado completo + catálogo de etiquetas y proyectos.
    ///
    /// Son tres viajes (`tasks`, `labels`, `projects`) porque el servidor no
    /// filtra ni devuelve nombres: sin el catálogo no se pueden calcular las
    /// vistas ni componer una salida sin ids de etiqueta.
    async fn open_todo_snapshot(client: &ot::Opentodo) -> Result<Snapshot, ToolError> {
        let mut tasks = client.list_tasks().await.map_err(opentodo_error)?;
        let labels = client.list_labels().await.map_err(opentodo_error)?;
        let projects = client.list_projects().await.map_err(opentodo_error)?;

        let names: HashMap<String, String> = labels
            .into_iter()
            .map(|label| (label.id, label.name))
            .collect();
        // Los nombres del catálogo son la única referencia fiable para enseñar
        // etiquetas: un elemento de `labels` que no esté aquí no se proyecta.
        let known_labels: BTreeSet<String> = names.values().cloned().collect();
        ot::apply_label_names(&mut tasks, &names);

        Ok(Snapshot {
            tasks,
            projects,
            known_labels,
        })
    }

    /// `GET /api/v1/projects` del que sale el `id` de un nombre de proyecto.
    /// Un nombre desconocido no inventa un id: se devuelven los reales.
    async fn resolve_project_id(client: &ot::Opentodo, name: &str) -> Result<String, ToolError> {
        let projects = client.list_projects().await.map_err(opentodo_error)?;

        projects
            .iter()
            .find(|project| project.name.eq_ignore_ascii_case(name))
            .map(|project| project.id.clone())
            .ok_or_else(|| {
                let available: Vec<&str> = projects
                    .iter()
                    .map(|project| project.name.as_str())
                    .collect();
                let available = if available.is_empty() {
                    "(no hay ninguno)".to_string()
                } else {
                    available.join(", ")
                };
                ToolError::InvalidArguments(format!(
                    "el proyecto «{name}» no existe en OpenTodo; proyectos disponibles: {available}"
                ))
            })
    }

    /// Resuelve un `due`: ISO directo y, si no, delegado en `quick-add/parse`.
    async fn resolve_due(
        client: &ot::Opentodo,
        raw: Option<&str>,
    ) -> Result<Option<ot::Due>, ToolError> {
        let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
            return Ok(None);
        };

        if let Some(due) = ot::parse_iso_due(raw) {
            return Ok(Some(due));
        }

        client
            .parse_natural_due(raw)
            .await
            .map_err(opentodo_error)?
            .ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "no se pudo interpretar la fecha «{raw}»; usa una fecha ISO (YYYY-MM-DD) \
                     o una expresión como «el viernes a las 10»"
                ))
            })
            .map(Some)
    }

    /// `list_tasks` contra OpenTodo: clasifica la vista en Rust, filtra por
    /// proyecto y acota la salida.
    async fn open_todo_list_tasks(
        &self,
        args: &Value,
        limit: usize,
        view: ot::View,
    ) -> Result<ToolResult, ToolError> {
        // El tipo del filtro se comprueba antes de tocar la red.
        let project_filter = optional_str(args, "project")?;

        let client = self.open_todo().await?;
        let snapshot = Self::open_todo_snapshot(&client).await?;
        let names = snapshot.names();
        let today = today();
        let me_id = Self::current_user_id(&client).await;

        // El resumen describe la cuenta entera: es lo que permite al agente
        // decidir qué vista mirar después.
        let summary = ot::summarize(&snapshot.tasks, today, me_id.as_deref(), &snapshot.projects);

        let selected: Vec<&ot::Task> = snapshot
            .tasks
            .iter()
            .filter(|task| ot::matches_view_as(task, view, today, me_id.as_deref()))
            .filter(|task| match &project_filter {
                None => true,
                Some(name) => task
                    .project_id
                    .as_deref()
                    .and_then(|id| names.get(id))
                    .is_some_and(|project| project.eq_ignore_ascii_case(name)),
            })
            .collect();

        let tasks: Vec<Value> = selected
            .iter()
            .take(limit)
            .map(|task| task_json(task, &names, &snapshot.known_labels, None))
            .collect();

        Ok(ToolResult {
            success: true,
            data: json!({ "count": selected.len(), "tasks": tasks, "summary": summary }),
            message: None,
        })
    }

    /// `search_tasks`: no hay endpoint de búsqueda, así que se filtra en Rust
    /// sobre el listado y se acota igual que `list_tasks`.
    async fn open_todo_search_tasks(
        &self,
        args: &Value,
        limit: usize,
    ) -> Result<ToolResult, ToolError> {
        let query = required_str(args, "query")?;
        let needle = query.to_lowercase();

        let client = self.open_todo().await?;
        let snapshot = Self::open_todo_snapshot(&client).await?;
        let names = snapshot.names();
        let today = today();
        let me_id = Self::current_user_id(&client).await;
        let summary = ot::summarize(&snapshot.tasks, today, me_id.as_deref(), &snapshot.projects);

        let matches: Vec<&ot::Task> = snapshot
            .tasks
            .iter()
            .filter(|task| task.is_open())
            .filter(|task| {
                task.title.to_lowercase().contains(&needle)
                    || task.description.to_lowercase().contains(&needle)
            })
            .collect();

        let tasks: Vec<Value> = matches
            .iter()
            .take(limit)
            .map(|task| task_json(task, &names, &snapshot.known_labels, None))
            .collect();

        Ok(ToolResult {
            success: true,
            data: json!({ "count": matches.len(), "tasks": tasks, "summary": summary }),
            message: Some(format!(
                "{} tarea(s) que coinciden con «{query}»",
                matches.len()
            )),
        })
    }

    /// `add_task` contra OpenTodo: prioridad siempre explícita, proyecto
    /// resuelto por nombre y fecha delegada en quick-add cuando no es ISO.
    ///
    /// Todos los argumentos se validan **antes** de cualquier viaje de red: un
    /// tipo equivocado es un argumento inválido, nunca un campo ignorado.
    async fn open_todo_add_task(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let title = required_str(args, "title")?;
        let priority = parse_priority(args, ot::Priority::Low)?;
        let description = optional_str(args, "description")?;
        // En `add_task`, `start` solo se fija: `null` y la cadena vacía no
        // significan nada aquí, así que se leen como «sin start».
        let start = optional_str(args, "start")?;
        let project = optional_str(args, "project")?;
        let due_arg = match parse_due_arg(args)? {
            Some(Some(raw)) => Some(raw),
            _ => None,
        };

        let client = self.open_todo().await?;

        let project_id = match &project {
            None => None,
            Some(name) => Some(Self::resolve_project_id(&client, name).await?),
        };
        let due = Self::resolve_due(&client, due_arg).await?;

        let task = ot::NewTask {
            title: title.to_string(),
            description,
            project_id,
            priority: Some(priority),
            due,
            labels: None,
            start,
        };

        let outcome = client.create_task(&task).await.map_err(opentodo_error)?;

        let data = match &outcome.task {
            Some(created) => task_json(
                created,
                &BTreeMap::new(),
                &BTreeSet::new(),
                project.as_deref(),
            ),
            None => json!({ "id": Value::Null, "title": title, "project": project }),
        };
        let message = applied_message("Tarea creada en OpenTodo", outcome.already_applied);

        Ok(ToolResult {
            success: true,
            data,
            message: Some(message),
        })
    }

    /// `update_task` contra OpenTodo: solo los campos indicados.
    ///
    /// Semántica de los campos anulables: ausente = no se toca, `null` = se
    /// limpia, valor = se fija. Cualquier otro tipo es un argumento inválido.
    async fn open_todo_update_task(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let id = required_str(args, "id")?;
        let title = optional_str(args, "title")?;
        let description = optional_str(args, "description")?;
        let project = optional_str(args, "project")?;
        let priority = match args.get("priority") {
            None | Some(Value::Null) => None,
            Some(_) => Some(parse_priority(args, ot::Priority::Low)?),
        };
        let start = parse_start(args)?;
        let due_arg = parse_due_arg(args)?;

        let client = self.open_todo().await?;

        let project_id = match &project {
            None => None,
            Some(name) => Some(Some(Self::resolve_project_id(&client, name).await?)),
        };
        let due = match due_arg {
            None => None,
            Some(None) => Some(None),
            Some(Some(raw)) => Some(Self::resolve_due(&client, Some(raw)).await?),
        };

        let patch = ot::TaskPatch {
            title,
            description,
            project_id,
            priority,
            due,
            start,
            labels: None,
        };

        let outcome = client
            .update_task(id, &patch)
            .await
            .map_err(opentodo_error)?;
        let message = applied_message(
            &format!("Tarea {id} actualizada en OpenTodo"),
            outcome.already_applied,
        );

        Ok(ToolResult {
            success: true,
            data: json!({ "id": id }),
            message: Some(message),
        })
    }

    /// `complete_task` contra OpenTodo.
    async fn open_todo_complete_task(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let id = required_str(args, "id")?;

        let client = self.open_todo().await?;
        let outcome = client.complete_task(id).await.map_err(opentodo_error)?;
        let message = applied_message(
            &format!("Tarea {id} completada en OpenTodo"),
            outcome.already_applied,
        );

        Ok(ToolResult {
            success: true,
            data: json!({ "id": id }),
            message: Some(message),
        })
    }

    /// `delete_task` contra OpenTodo: `purge` usa la purga definitiva.
    async fn open_todo_delete_task(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let id = required_str(args, "id")?;
        let purge = args
            .get("purge")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        let client = self.open_todo().await?;
        let outcome = client
            .delete_task(id, purge)
            .await
            .map_err(opentodo_error)?;

        let message = if purge {
            format!("Tarea {id} borrada definitivamente de OpenTodo")
        } else {
            format!("Tarea {id} borrada de OpenTodo")
        };

        Ok(ToolResult {
            success: true,
            data: json!({ "id": id, "purged": purge }),
            message: Some(applied_message(&message, outcome.already_applied)),
        })
    }

    /// `set_label` contra OpenTodo: grupo y valor, nunca ids de etiqueta.
    async fn open_todo_set_label(&self, args: &Value) -> Result<ToolResult, ToolError> {
        let id = required_str(args, "id")?;
        // La validación del par grupo/valor ya pasó en `validate_args`, antes de
        // cualquier viaje de red; aquí se vuelve a parsear para tener los valores.
        let (group, value) = parse_group_and_value(args)?;

        let client = self.open_todo().await?;
        let outcome = client
            .set_label(id, group, value.as_deref())
            .await
            .map_err(opentodo_error)?;

        let mut message = match &value {
            Some(value) => format!(
                "Tarea {id}: etiqueta `{value}` del grupo `{}`",
                group.as_str()
            ),
            None => format!("Tarea {id}: sin etiqueta del grupo `{}`", group.as_str()),
        };
        if let Some(created) = &outcome.created_label {
            message.push_str(&format!("; se creó la etiqueta `{created}`"));
        }

        Ok(ToolResult {
            success: true,
            data: json!({
                "id": id,
                "group": group.as_str(),
                "value": value,
                "created_label": outcome.created_label,
            }),
            message: Some(applied_message(&message, outcome.already_applied)),
        })
    }

    // -- backend local (comportamiento vigente, sin token) -------------------

    async fn list_tasks(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;

        let tasks = TasksRepo::list(
            &self.db,
            profile_id,
            args.get("status").and_then(|v| v.as_str()),
            args.get("priority").and_then(|v| v.as_str()),
            args.get("project").and_then(|v| v.as_str()),
            args.get("scope").and_then(|v| v.as_str()),
        )
        .await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(tasks).unwrap_or_default(),
            message: None,
        })
    }

    async fn add_task(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing profile_id".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing content".into()))?;

        let now = Utc::now().to_rfc3339();
        let task = Task {
            id: Uuid::new_v4().to_string(),
            profile_id: profile_id.to_string(),
            content: content.to_string(),
            status: "inbox".to_string(),
            priority: args
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("medium")
                .to_string(),
            project: args
                .get("project")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            due_date: args
                .get("due_date")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            scope: args
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("shared")
                .to_string(),
            created_at: now.clone(),
            updated_at: now,
        };

        TasksRepo::create(&self.db, &task).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&task).unwrap_or_default(),
            message: Some("Task created successfully".into()),
        })
    }

    async fn update_task(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing id".into()))?;

        TasksRepo::update(
            &self.db,
            id,
            args.get("content").and_then(|v| v.as_str()),
            args.get("status").and_then(|v| v.as_str()),
            args.get("priority").and_then(|v| v.as_str()),
            args.get("project").and_then(|v| v.as_str()),
            args.get("due_date").and_then(|v| v.as_str()),
            args.get("scope").and_then(|v| v.as_str()),
        )
        .await?;

        let updated = TasksRepo::find_by_id(&self.db, id).await?.ok_or_else(|| {
            ToolError::ExecutionError(format!("Task not found after update: {}", id))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&updated).unwrap_or_default(),
            message: Some("Task updated successfully".into()),
        })
    }

    async fn complete_task(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing id".into()))?;

        TasksRepo::complete(&self.db, id).await?;

        let updated = TasksRepo::find_by_id(&self.db, id).await?.ok_or_else(|| {
            ToolError::ExecutionError(format!("Task not found after complete: {}", id))
        })?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&updated).unwrap_or_default(),
            message: Some("Task completed successfully".into()),
        })
    }

    async fn delete_task(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing id".into()))?;

        let task = TasksRepo::find_by_id(&self.db, id)
            .await?
            .ok_or_else(|| ToolError::ExecutionError(format!("No task found with id: {}", id)))?;

        TasksRepo::delete(&self.db, id).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&task).unwrap_or_default(),
            message: Some("Task deleted successfully".into()),
        })
    }
}

/// Mensaje que añade «ya estaba aplicada» cuando el servidor respondió `409`.
fn applied_message(message: &str, already_applied: bool) -> String {
    if already_applied {
        format!("{message} (ya estaba aplicada)")
    } else {
        message.to_string()
    }
}

#[async_trait]
impl Tool for TasksTool {
    fn name(&self) -> &'static str {
        "tasks"
    }

    fn description(&self) -> &'static str {
        "Gestión de tareas (OpenTodo): listar por vista, buscar, crear, actualizar, \
         completar, borrar y etiquetar tareas"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": [
                        "list_tasks",
                        "search_tasks",
                        "add_task",
                        "update_task",
                        "complete_task",
                        "delete_task",
                        "set_label"
                    ],
                    "description": "Acción a realizar. `title` es obligatorio para `add_task`; \
                        `id` para `update_task`, `complete_task`, `delete_task` y `set_label`; \
                        `query` para `search_tasks`. En el backend local (sin token de OpenTodo) \
                        `add_task` usa `content`."
                },
                "profile_id": { "type": "string", "description": "ID del perfil (solo backend local)" },
                "content": { "type": "string", "description": "Descripción o título de la tarea (solo backend local)" },
                "status": {
                    "type": "string",
                    "enum": ["inbox", "todo", "doing", "waiting", "someday", "done"],
                    "description": "Estado GTD de la tarea (solo backend local)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["critical", "high", "medium", "low"],
                    "description": "Prioridad de la tarea; sin indicarla se usa `low` en OpenTodo"
                },
                "project": { "type": "string", "description": "Proyecto asociado, por nombre" },
                "due_date": { "type": "string", "description": "Fecha límite ISO 8601 o YYYY-MM-DD (solo backend local)" },
                "due": { "type": "string", "description": "Fecha límite: YYYY-MM-DD, YYYY-MM-DDTHH:MM o una expresión como «el viernes a las 10»" },
                "start": { "type": "string", "description": "Inicio de la tarea; `someday` la aparca en «Algún día»" },
                "scope": {
                    "type": "string",
                    "enum": ["shared", "personal"],
                    "description": "Ámbito de la tarea (solo backend local)"
                },
                "view": {
                    "type": "string",
                    "enum": ["all", "inbox", "today", "upcoming", "anytime", "someday", "doing", "waiting"],
                    "description": "Vista de OpenTodo por la que filtrar (por defecto `all`)"
                },
                "query": { "type": "string", "description": "Texto a buscar en el título y la descripción (`search_tasks`)" },
                "title": { "type": "string", "description": "Título de la tarea (`add_task`)" },
                "description": { "type": "string", "description": "Descripción larga de la tarea" },
                "limit": {
                    "type": "integer",
                    "minimum": MIN_LIMIT,
                    "maximum": MAX_LIMIT,
                    "default": DEFAULT_LIMIT,
                    "description": "Número máximo de tareas a devolver (1..=50)"
                },
                "purge": { "type": "boolean", "default": false, "description": "Borra definitivamente la tarea (irreversible)" },
                "group": {
                    "type": "string",
                    "enum": ["state", "scope"],
                    "description": "Grupo de la etiqueta: `state` (doing/waiting) o `scope` (shared/personal)"
                },
                "value": {
                    "type": ["string", "null"],
                    "enum": ["doing", "waiting", "shared", "personal"],
                    "description": "Valor de la etiqueta del grupo; `null` quita la del grupo"
                },
                "id": { "type": "string", "description": "ID de la tarea" }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, args: &Value) -> Permission {
        match args.get("operation").and_then(|v| v.as_str()) {
            Some("list_tasks") | Some("search_tasks") => Permission::NoConfirm,
            Some("add_task") | Some("update_task") | Some("complete_task") | Some("set_label") => {
                Permission::Notify
            }
            Some("delete_task") => Permission::ExplicitApproval,
            _ => Permission::NoConfirm,
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("");

        // Validaciones que no deben tocar la red, para cualquier backend.
        validate_args(operation, &args)?;

        if ot::Opentodo::is_configured(&self.db).await {
            return match operation {
                "list_tasks" => {
                    self.open_todo_list_tasks(&args, parse_limit(&args)?, parse_view(&args)?)
                        .await
                }
                "search_tasks" => {
                    self.open_todo_search_tasks(&args, parse_limit(&args)?)
                        .await
                }
                "add_task" => self.open_todo_add_task(&args).await,
                "update_task" => self.open_todo_update_task(&args).await,
                "complete_task" => self.open_todo_complete_task(&args).await,
                "delete_task" => self.open_todo_delete_task(&args).await,
                "set_label" => self.open_todo_set_label(&args).await,
                other => Err(ToolError::InvalidArguments(format!(
                    "Unknown operation: {other}"
                ))),
            };
        }

        match operation {
            "list_tasks" => self.list_tasks(args).await,
            "add_task" => self.add_task(args).await,
            "update_task" => self.update_task(args).await,
            "complete_task" => self.complete_task(args).await,
            "delete_task" => self.delete_task(args).await,
            // Operaciones que solo existen en OpenTodo: sin token se dice claro.
            "search_tasks" | "set_label" => Err(ToolError::ExecutionError(format!(
                "`{operation}` requiere OpenTodo: configura el token en Ajustes → \
                 Integraciones → OpenTodo, o define OPENTODO_TOKEN."
            ))),
            op => Err(ToolError::InvalidArguments(format!(
                "Unknown operation: {}",
                op
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::SettingsRepo;
    use crate::db::schema::run_migrations;
    use chrono::{Duration, Local};
    use serde_json::json;
    use serial_test::serial;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::BTreeSet;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn setup() -> Result<(SqlitePool, TasksTool), sqlx::Error> {
        // H5: el backend se elige leyendo `OPENTODO_TOKEN`, así que un token
        // exportado en la shell del desarrollador convertiría estos tests
        // locales en llamadas a la instancia real. Limpiarlo aquí es inocuo: los
        // tests de OpenTodo siembran en `settings`, que gana al entorno.
        std::env::remove_var("OPENTODO_TOKEN");
        std::env::remove_var("OPENTODO_BASE_URL");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await?;
        let tool = TasksTool::new(pool.clone());
        Ok((pool, tool))
    }

    #[tokio::test]
    #[serial]
    async fn test_list_empty() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_tasks",
                "profile_id": "profile-1"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data.as_array().unwrap().len(), 0);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_add_task() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "add_task",
                "profile_id": "profile-1",
                "content": "Comprar leche",
                "priority": "high",
                "project": "Casa"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["content"], "Comprar leche");
        assert_eq!(result.data["priority"], "high");
        assert_eq!(result.data["project"], "Casa");
        assert_eq!(result.data["status"], "inbox");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_complete_task_returns_full_object() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create a task with specific content and priority
        let created = tool
            .execute(serde_json::json!({
                "operation": "add_task",
                "profile_id": "profile-1",
                "content": "Tarea para completar",
                "priority": "high"
            }))
            .await
            .unwrap();
        let task_id = created.data["id"].as_str().unwrap().to_string();

        // Complete it
        let result = tool
            .execute(serde_json::json!({
                "operation": "complete_task",
                "id": task_id
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], task_id);
        assert_eq!(result.data["content"], "Tarea para completar");
        assert_eq!(result.data["status"], "done");
        assert_eq!(result.data["priority"], "high");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_invalid_operation() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "nonexistent"
            }))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_update_task() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Add a task first
        let created = tool
            .execute(serde_json::json!({
                "operation": "add_task",
                "profile_id": "profile-1",
                "content": "Tarea original",
                "priority": "low"
            }))
            .await
            .unwrap();
        let task_id = created.data["id"].as_str().unwrap().to_string();

        // Update it
        let result = tool
            .execute(serde_json::json!({
                "operation": "update_task",
                "id": task_id,
                "content": "Tarea actualizada",
                "priority": "high"
            }))
            .await
            .unwrap();
        assert!(result.success);

        // Verify via list
        let list_result = tool
            .execute(serde_json::json!({
                "operation": "list_tasks",
                "profile_id": "profile-1"
            }))
            .await
            .unwrap();
        let tasks = list_result.data.as_array().unwrap();
        let updated = tasks.iter().find(|t| t["id"] == task_id).unwrap();
        assert_eq!(updated["content"], "Tarea actualizada");
        assert_eq!(updated["priority"], "high");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_update_task_returns_full_object() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create a task first
        let created = tool
            .execute(serde_json::json!({
                "operation": "add_task",
                "profile_id": "profile-1",
                "content": "Tarea original",
                "priority": "low"
            }))
            .await
            .unwrap();
        let task_id = created.data["id"].as_str().unwrap().to_string();

        // Update it
        let result = tool
            .execute(serde_json::json!({
                "operation": "update_task",
                "id": task_id,
                "content": "Tarea actualizada",
                "priority": "high"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], task_id);
        assert_eq!(result.data["content"], "Tarea actualizada");
        assert_eq!(result.data["priority"], "high");
        assert_eq!(result.data["status"], "inbox");
        assert!(
            result.data["project"].is_null() || result.data.get("project").is_none(),
            "project should be null or missing"
        );
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_task() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Create a task first
        let created = tool
            .execute(serde_json::json!({
                "operation": "add_task",
                "profile_id": "profile-1",
                "content": "Tarea para eliminar"
            }))
            .await
            .unwrap();
        let task_id = created.data["id"].as_str().unwrap().to_string();
        let original_content = created.data["content"].as_str().unwrap().to_string();

        // Delete it
        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_task",
                "id": task_id
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], task_id);
        assert_eq!(result.data["content"], original_content);

        // Verify it's gone from listing
        let list_result = tool
            .execute(serde_json::json!({
                "operation": "list_tasks",
                "profile_id": "profile-1"
            }))
            .await
            .unwrap();
        let tasks = list_result.data.as_array().unwrap();
        assert!(!tasks.iter().any(|t| t["id"] == task_id));
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_task_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_task",
                "id": "nonexistent-id"
            }))
            .await;
        assert!(matches!(
            result,
            Err(ToolError::ExecutionError(ref msg)) if msg.contains("No task found")
        ));
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_list_tasks_with_filters() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        // Add two tasks with different projects
        tool.execute(serde_json::json!({
            "operation": "add_task",
            "profile_id": "profile-1",
            "content": "Tarea Alpha",
            "project": "Alpha"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "operation": "add_task",
            "profile_id": "profile-1",
            "content": "Tarea Beta",
            "project": "Beta"
        }))
        .await
        .unwrap();

        // Filter by project
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_tasks",
                "profile_id": "profile-1",
                "project": "Alpha"
            }))
            .await
            .unwrap();
        let tasks = result.data.as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["project"], "Alpha");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: the `operation` description documents the
    // required fields per action.
    // -----------------------------------------------------------------------

    /// Scenario: La operación documenta los obligatorios
    #[tokio::test]
    #[serial]
    async fn test_tasks_operation_description_documents_required_fields(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let params = tool.parameters();

        let desc = params["properties"]["operation"]["description"]
            .as_str()
            .unwrap_or("");
        assert!(
            !desc.is_empty(),
            "the `operation` parameter must carry a description"
        );
        assert!(
            desc.contains("`content`"),
            "the `operation` description must mention `content` for add_task, got: {desc}"
        );
        assert!(
            desc.contains("`id`"),
            "the `operation` description must mention `id` for update/complete/delete, got: {desc}"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: todas las descripciones de parámetros deben
    // estar en español (requisito transversal de `tools/registry`).
    // -----------------------------------------------------------------------

    /// Scenario: Las definiciones de herramientas están en español y documentan
    /// los obligatorios; aquí se cubre `tasks`.
    #[tokio::test]
    #[serial]
    async fn test_tasks_parameter_descriptions_are_in_spanish(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup().await?;
        let params = tool.parameters();
        let properties = params["properties"]
            .as_object()
            .expect("`properties` must be an object");

        // Toda propiedad debe llevar una descripción no vacía y en español.
        for (name, schema) in properties {
            let desc = schema["description"].as_str().unwrap_or("");
            assert!(
                !desc.is_empty(),
                "la descripción de `{name}` no debe estar vacía"
            );
        }

        assert!(
            properties["content"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("tarea"),
            "`content` debe describir la tarea"
        );
        assert!(
            properties["id"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("ID"),
            "`id` debe describirse como «ID de la tarea»"
        );
        assert!(
            properties["due_date"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("Fecha"),
            "`due_date` debe describirse como fecha límite"
        );
        assert!(
            properties["priority"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("Prioridad"),
            "`priority` debe describirse en español"
        );
        assert!(
            properties["status"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("Estado"),
            "`status` debe describirse en español"
        );
        assert!(
            properties["project"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("Proyecto"),
            "`project` debe describirse en español"
        );
        assert!(
            properties["scope"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("Ámbito"),
            "`scope` debe describirse en español"
        );
        assert!(
            properties["profile_id"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("ID del perfil"),
            "`profile_id` debe describirse como «ID del perfil»"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // open-todo redux: contrato de la herramienta (`parameters` y `permission`)
    // -----------------------------------------------------------------------

    /// Scenario: la operación declara el enum de siete valores y `operation` es
    /// el único obligatorio, con los obligatorios documentados.
    #[tokio::test]
    #[serial]
    async fn test_parameters_declare_the_seven_operations() {
        let (_, tool) = setup().await.unwrap();
        let params = tool.parameters();

        let operations: Vec<&str> = params["properties"]["operation"]["enum"]
            .as_array()
            .expect("`operation` debe declarar un enum")
            .iter()
            .map(|value| value.as_str().expect("valores de texto"))
            .collect();
        assert_eq!(
            operations,
            [
                "list_tasks",
                "search_tasks",
                "add_task",
                "update_task",
                "complete_task",
                "delete_task",
                "set_label"
            ],
            "el enum de operaciones"
        );

        let required: Vec<&str> = params["required"]
            .as_array()
            .expect("`required` debe ser un array")
            .iter()
            .map(|value| value.as_str().expect("valores de texto"))
            .collect();
        assert_eq!(required, ["operation"], "solo `operation` es obligatorio");

        let description = params["properties"]["operation"]["description"]
            .as_str()
            .unwrap_or("");
        for needle in ["`title`", "`id`", "`query`"] {
            assert!(
                description.contains(needle),
                "la descripción de `operation` debe documentar {needle}: {description}"
            );
        }
    }

    /// Scenario: el resto del contrato —enums, `limit` y `purge`— está declarado.
    #[tokio::test]
    #[serial]
    async fn test_parameters_declare_the_rest_of_the_contract() {
        let (_, tool) = setup().await.unwrap();
        let params = tool.parameters();
        let properties = params["properties"]
            .as_object()
            .expect("`properties` debe ser un objeto");

        for (name, expected) in [
            (
                "view",
                vec![
                    "all", "inbox", "today", "upcoming", "anytime", "someday", "doing", "waiting",
                ],
            ),
            ("priority", vec!["critical", "high", "medium", "low"]),
            ("group", vec!["state", "scope"]),
            ("value", vec!["doing", "waiting", "shared", "personal"]),
        ] {
            let values: Vec<&str> = properties[name]["enum"]
                .as_array()
                .unwrap_or_else(|| panic!("`{name}` debe declarar un enum"))
                .iter()
                .map(|value| value.as_str().expect("valores de texto"))
                .collect();
            assert_eq!(values, expected, "el enum de `{name}`");
        }

        assert_eq!(properties["limit"]["default"], json!(20));
        assert_eq!(properties["limit"]["minimum"], json!(1));
        assert_eq!(properties["limit"]["maximum"], json!(50));
        assert_eq!(properties["purge"]["type"], json!("boolean"));
        assert_eq!(properties["purge"]["default"], json!(false));
        // `value: null` quita la etiqueta del grupo.
        assert_eq!(properties["value"]["type"], json!(["string", "null"]));

        for key in [
            "project",
            "query",
            "title",
            "description",
            "due",
            "start",
            "id",
        ] {
            assert!(properties.contains_key(key), "falta el parámetro `{key}`");
        }
    }

    /// Scenario: el modelo no puede mandar ids de etiqueta. El conjunto de
    /// parámetros es exactamente el declarado: ni `label_ids` ni nada parecido.
    #[tokio::test]
    #[serial]
    async fn test_parameters_do_not_accept_label_ids() {
        let (_, tool) = setup().await.unwrap();
        let params = tool.parameters();
        let properties = params["properties"]
            .as_object()
            .expect("`properties` debe ser un objeto");

        let names: BTreeSet<&str> = properties.keys().map(|name| name.as_str()).collect();
        let expected: BTreeSet<&str> = [
            "operation",
            "profile_id",
            "content",
            "status",
            "priority",
            "project",
            "due_date",
            "scope",
            "id",
            "view",
            "query",
            "limit",
            "title",
            "description",
            "due",
            "start",
            "purge",
            "group",
            "value",
        ]
        .into_iter()
        .collect();
        assert_eq!(names, expected, "el conjunto exacto de parámetros");

        for name in &names {
            assert!(
                !name.contains("label") && !name.contains("tag"),
                "no debe existir un parámetro de etiquetas por id: `{name}`"
            );
        }
    }

    /// Scenario: los permisos por operación de la tabla de la spec.
    #[tokio::test]
    #[serial]
    async fn test_permission_table() {
        let (_, tool) = setup().await.unwrap();

        for (operation, expected) in [
            ("list_tasks", Permission::NoConfirm),
            ("search_tasks", Permission::NoConfirm),
            ("add_task", Permission::Notify),
            ("update_task", Permission::Notify),
            ("complete_task", Permission::Notify),
            ("set_label", Permission::Notify),
            ("delete_task", Permission::ExplicitApproval),
        ] {
            assert_eq!(
                tool.permission(&json!({ "operation": operation })),
                expected,
                "permiso de `{operation}`"
            );
        }

        assert_eq!(
            tool.permission(&json!({ "operation": "delete_task", "purge": true })),
            Permission::ExplicitApproval,
            "la purga definitiva también exige aprobación explícita"
        );
    }

    // -----------------------------------------------------------------------
    // open-todo redux: el doble backend
    // -----------------------------------------------------------------------

    /// Token que viajan los tests contra el `wiremock`.
    const TOKEN: &str = "test-token";

    /// Pool local + servidor de prueba + herramienta con OpenTodo configurado en
    /// `settings` (**no** en el entorno: el servicio resuelve la configuración en
    /// cada llamada y `settings` gana, así que los tests no dependen de variables
    /// de proceso).
    async fn open_todo_setup() -> (SqlitePool, MockServer, TasksTool) {
        let (pool, _) = setup().await.unwrap();
        let server = MockServer::start().await;
        SettingsRepo::set(&pool, "opentodo_token", TOKEN)
            .await
            .unwrap();
        SettingsRepo::set(&pool, "opentodo_base_url", &server.uri())
            .await
            .unwrap();
        let tool = TasksTool::new(pool.clone());
        (pool, server, tool)
    }

    /// Forma real de una tarea de OpenTodo, con los campos que se necesiten.
    fn ot_task(
        id: &str,
        title: &str,
        description: &str,
        project_id: Option<&str>,
        due: Option<&str>,
        labels: &[&str],
        completed_at: Option<i64>,
    ) -> Value {
        json!({
            "id": id,
            "title": title,
            "description": description,
            "project_id": project_id,
            "parent_id": null,
            "priority": 4,
            "due": due.map(|date| json!({ "date": date })),
            "labels": labels,
            "fields": {},
            "position": 1,
            "created_at": 1,
            "completed_at": completed_at,
            "deleted_at": null,
            "origin": null
        })
    }

    async fn mount_me(server: &MockServer, id: &str) {
        Mock::given(method("GET"))
            .and(path("/api/v1/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": id })))
            .mount(server)
            .await;
    }

    async fn mount_labels(server: &MockServer, labels: &[(&str, &str)]) {
        let body = json!({
            "labels": labels
                .iter()
                .map(|(id, name)| json!({ "id": id, "name": name }))
                .collect::<Vec<_>>()
        });
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    async fn mount_projects(server: &MockServer, projects: &[(&str, &str)]) {
        let body = json!({
            "projects": projects
                .iter()
                .map(|(id, name)| json!({ "id": id, "name": name }))
                .collect::<Vec<_>>()
        });
        Mock::given(method("GET"))
            .and(path("/api/v1/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    async fn mount_list(server: &MockServer, tasks: Value) {
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "tasks": tasks })))
            .mount(server)
            .await;
    }

    /// El mínimo que necesita un `list_tasks`: usuario, etiquetas y proyectos.
    async fn mount_catalog(server: &MockServer) {
        mount_me(server, "u1").await;
        mount_labels(server, &[]).await;
        mount_projects(server, &[]).await;
    }

    /// Crea una tarea en la tabla local: no debe leerse cuando hay token.
    async fn seed_local_task(pool: &SqlitePool, id: &str, content: &str) {
        let task = Task {
            id: id.to_string(),
            profile_id: "profile-1".to_string(),
            content: content.to_string(),
            status: "inbox".to_string(),
            priority: "medium".to_string(),
            project: None,
            due_date: None,
            scope: "shared".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        TasksRepo::create(pool, &task).await.unwrap();
    }

    /// Rutas golpeadas en el `wiremock`, en orden.
    async fn requested_paths(server: &MockServer) -> Vec<String> {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| request.url.path().to_string())
            .collect()
    }

    /// Scenario: con token, `list_tasks` va a OpenTodo y nunca a la tabla local.
    #[tokio::test]
    #[serial]
    async fn test_list_tasks_goes_to_opentodo_and_not_to_the_local_table() {
        let (pool, server, tool) = open_todo_setup().await;
        seed_local_task(&pool, "local-1", "LOCAL-ONLY").await;

        let tasks: Vec<Value> = (1..=25)
            .map(|i| {
                ot_task(
                    &format!("t{i}"),
                    &format!("Título {i}"),
                    "",
                    None,
                    None,
                    &[],
                    None,
                )
            })
            .collect();
        mount_catalog(&server).await;
        mount_list(&server, json!(tasks)).await;

        let result = tool
            .execute(json!({ "operation": "list_tasks" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            result.data["count"],
            json!(25),
            "`count` es el total tras el filtro, no el recortado"
        );

        let listed = result.data["tasks"]
            .as_array()
            .expect("`tasks` es un array");
        assert_eq!(listed.len(), 20, "el tope por defecto es `limit: 20`");
        assert!(
            listed
                .iter()
                .all(|task| task["title"] != json!("LOCAL-ONLY")),
            "con token no se lee la tabla local: {listed:?}"
        );

        let keys: BTreeSet<&str> = listed[0]
            .as_object()
            .expect("cada tarea es un objeto")
            .keys()
            .map(|key| key.as_str())
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["id", "title", "project", "due", "priority", "start", "labels"]),
            "la proyección mínima de cada tarea"
        );

        assert_eq!(
            result.data["summary"]["by_view"]["inbox"],
            json!(25),
            "el resumen agrega el listado completo"
        );
        assert_eq!(
            result.data["summary"]["by_project"]["(sin proyecto)"],
            json!(25)
        );

        assert!(
            requested_paths(&server)
                .await
                .contains(&"/api/v1/tasks".to_string()),
            "debe haber pedido el listado a OpenTodo"
        );
    }

    /// Scenario: un `limit` fuera de rango es un argumento inválido **sin**
    /// salir a la red.
    #[tokio::test]
    #[serial]
    async fn test_limit_out_of_range_is_rejected_without_network() {
        let (_pool, server, tool) = open_todo_setup().await;

        for limit in [json!(0), json!(500)] {
            let error = tool
                .execute(json!({ "operation": "list_tasks", "limit": limit }))
                .await
                .expect_err("un limit fuera de 1..=50 debe rechazarse");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "limit {limit}: {error:?}"
            );
            let message = error.to_string();
            assert!(
                message.contains("limit"),
                "el motivo debe ser el `limit`, no otra cosa (limit {limit}): {message}"
            );
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un limit inválido no debe enviar ninguna petición"
        );
    }

    /// Scenario: la vista se calcula en Rust, no se delega en el servidor. Una
    /// tarea vencida sale en `today`; la futura y la completada, no.
    #[tokio::test]
    #[serial]
    async fn test_view_today_is_computed_in_rust() {
        let (_pool, server, tool) = open_todo_setup().await;
        let today = Local::now().date_naive();
        let yesterday = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        let next_month = (today + Duration::days(30)).format("%Y-%m-%d").to_string();

        mount_catalog(&server).await;
        mount_list(
            &server,
            json!([
                ot_task("vencida", "Vencida", "", None, Some(&yesterday), &[], None),
                ot_task("futura", "Futura", "", None, Some(&next_month), &[], None),
                ot_task(
                    "hecha",
                    "Hecha",
                    "",
                    None,
                    Some(&yesterday),
                    &[],
                    Some(1_793_082_844_171i64)
                ),
            ]),
        )
        .await;

        let result = tool
            .execute(json!({ "operation": "list_tasks", "view": "today" }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], json!(1));
        let listed = result.data["tasks"].as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["id"], json!("vencida"));

        let requests = server.received_requests().await.unwrap();
        let list_request = requests
            .iter()
            .find(|request| request.url.path() == "/api/v1/tasks")
            .expect("debe pedir el listado");
        assert!(
            list_request.url.query().is_none(),
            "no se envía ningún filtro de fecha al servidor: {}",
            list_request.url
        );
    }

    /// Scenario: `search_tasks` busca en título y descripción en Rust, porque el
    /// servidor no tiene endpoint de búsqueda.
    #[tokio::test]
    #[serial]
    async fn test_search_tasks_matches_title_and_description() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_catalog(&server).await;
        mount_list(
            &server,
            json!([
                ot_task("t1", "Preparar informe", "", None, None, &[], None),
                ot_task(
                    "t2",
                    "Otra cosa",
                    "Enviar el informe el lunes",
                    None,
                    None,
                    &[],
                    None
                ),
                ot_task("t3", "Comprar pan", "", None, None, &[], None),
            ]),
        )
        .await;

        let result = tool
            .execute(json!({ "operation": "search_tasks", "query": "informe" }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], json!(2));
        let listed = result.data["tasks"].as_array().unwrap();
        assert_eq!(listed.len(), 2);
    }

    // -----------------------------------------------------------------------
    // open-todo redux: escrituras
    // -----------------------------------------------------------------------

    /// Scenario: sin prioridad se envía `4`; con `critical`, `1`.
    #[tokio::test]
    #[serial]
    async fn test_add_task_sends_the_priority_explicitly() {
        for (priority, code) in [(None, 4), (Some("critical"), 1)] {
            let (_pool, server, tool) = open_todo_setup().await;
            let mut args = json!({ "operation": "add_task", "title": "Comprar pan" });
            if let Some(priority) = priority {
                args["priority"] = json!(priority);
            }
            // El cuerpo lleva siempre la prioridad explícita, en su código 1..4.
            Mock::given(method("POST"))
                .and(path("/api/v1/tasks"))
                .and(body_json(
                    json!({ "title": "Comprar pan", "priority": code }),
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                    "new",
                    "Comprar pan",
                    "",
                    None,
                    None,
                    &[],
                    None,
                )))
                .expect(1)
                .mount(&server)
                .await;

            let result = tool.execute(args).await.unwrap();
            assert!(result.success);
            assert_eq!(result.data["id"], json!("new"));
        }
    }

    /// Scenario: una fecha ISO no pasa por quick-add; una expresión natural sí.
    #[tokio::test]
    #[serial]
    async fn test_add_task_due_delegation() {
        // ISO: nada de quick-add.
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/tasks"))
            .and(body_json(json!({
                "title": "Pagar la luz", "priority": 4, "due": { "date": "2026-10-30" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "new",
                "Pagar la luz",
                "",
                None,
                Some("2026-10-30"),
                &[],
                None,
            )))
            .mount(&server)
            .await;

        tool.execute(json!({
            "operation": "add_task", "title": "Pagar la luz", "due": "2026-10-30"
        }))
        .await
        .unwrap();

        assert!(
            !requested_paths(&server)
                .await
                .contains(&"/api/v1/quick-add/parse".to_string()),
            "una fecha ISO no debe pasar por quick-add"
        );

        // Natural: se delega en el servidor y se usa su `due`.
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/quick-add/parse"))
            .and(body_json(json!({ "text": "el viernes a las 10" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "due": { "date": "2026-10-30", "time": "10:00" }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/tasks"))
            .and(body_json(json!({
                "title": "Médico", "priority": 4, "due": { "date": "2026-10-30", "time": "10:00" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "new",
                "Médico",
                "",
                None,
                Some("2026-10-30"),
                &[],
                None,
            )))
            .mount(&server)
            .await;

        tool.execute(json!({
            "operation": "add_task", "title": "Médico", "due": "el viernes a las 10"
        }))
        .await
        .unwrap();
    }

    /// Scenario: un proyecto inexistente no se inventa; se responden los reales.
    #[tokio::test]
    #[serial]
    async fn test_add_task_with_unknown_project_lists_the_real_ones() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_projects(&server, &[("p1", "Casa"), ("p2", "Trabajo")]).await;

        let error = tool
            .execute(json!({
                "operation": "add_task", "title": "Regar plantas", "project": "Inexistente"
            }))
            .await
            .expect_err("un proyecto inexistente debe rechazarse");

        let message = error.to_string();
        assert!(
            message.contains("Casa") && message.contains("Trabajo"),
            "el mensaje debe listar los proyectos reales: {message}"
        );
        assert!(
            !requested_paths(&server)
                .await
                .contains(&"/api/v1/tasks".to_string()),
            "no debe crear la tarea con un id inventado"
        );
    }

    /// Un proyecto real se resuelve a su id y viaja en el cuerpo.
    #[tokio::test]
    #[serial]
    async fn test_add_task_resolves_the_project_name() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_projects(&server, &[("p1", "Casa")]).await;
        Mock::given(method("POST"))
            .and(path("/api/v1/tasks"))
            .and(body_json(json!({
                "title": "Regar plantas", "priority": 4, "project_id": "p1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "new",
                "Regar plantas",
                "",
                Some("p1"),
                None,
                &[],
                None,
            )))
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({
                "operation": "add_task", "title": "Regar plantas", "project": "Casa"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            result.data["project"],
            json!("Casa"),
            "la salida lleva el nombre del proyecto, no el id"
        );
    }

    /// Scenario: `delete_task` con `purge` usa la ruta de purga definitiva.
    #[tokio::test]
    #[serial]
    async fn test_delete_task_with_purge_uses_the_purge_route() {
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/tasks/t1/purge"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({ "operation": "delete_task", "id": "t1", "purge": true }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            requested_paths(&server).await,
            vec!["/api/v1/tasks/t1/purge".to_string()]
        );
    }

    /// `complete_task` marca la tarea y devuelve su proyección.
    #[tokio::test]
    #[serial]
    async fn test_complete_task_patches_the_task() {
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "t1",
                "Terminada",
                "",
                None,
                None,
                &[],
                Some(1_793_082_844_171i64),
            )))
            .expect(1)
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({ "operation": "complete_task", "id": "t1" }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["id"], json!("t1"));
    }

    /// Scenario: `set_label` valida el valor del grupo **antes** de salir a la red.
    #[tokio::test]
    #[serial]
    async fn test_set_label_rejects_a_foreign_value_without_network() {
        let (_pool, server, tool) = open_todo_setup().await;

        let error = tool
            .execute(json!({
                "operation": "set_label", "id": "t1", "group": "state", "value": "personal"
            }))
            .await
            .expect_err("`personal` no pertenece al grupo `state`");

        assert!(
            matches!(error, ToolError::InvalidArguments(_)),
            "un valor fuera del grupo es un argumento inválido: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("personal") && message.contains("state"),
            "el motivo debe ser el valor y su grupo: {message}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un valor inválido no debe enviar ninguna petición"
        );
    }

    /// Scenario: poner un estado no borra las demás etiquetas: el `PATCH` manda
    /// el array completo.
    #[tokio::test]
    #[serial]
    async fn test_set_label_keeps_the_other_labels() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_list(
            &server,
            json!([ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &["lbl-shared"],
                None
            )]),
        )
        .await;
        mount_labels(
            &server,
            &[
                ("lbl-shared", "shared"),
                ("lbl-doing", "doing"),
                ("lbl-waiting", "waiting"),
            ],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(json!({ "labels": ["lbl-shared", "lbl-doing"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &["lbl-shared", "lbl-doing"],
                None,
            )))
            .expect(1)
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({
                "operation": "set_label", "id": "t1", "group": "state", "value": "doing"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.data["value"], json!("doing"));
    }

    /// `value: null` (o ausente) quita la etiqueta del grupo.
    #[tokio::test]
    #[serial]
    async fn test_set_label_without_value_removes_it() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_list(
            &server,
            json!([ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &["lbl-doing", "lbl-personal"],
                None
            )]),
        )
        .await;
        mount_labels(
            &server,
            &[("lbl-doing", "doing"), ("lbl-personal", "personal")],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(json!({ "labels": ["lbl-personal"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &["lbl-personal"],
                None,
            )))
            .expect(1)
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({
                "operation": "set_label", "id": "t1", "group": "state", "value": null
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.data["value"].is_null());
    }

    /// H2 — un `409` en `set_label` significa «ya estaba aplicada»: lo dice el
    /// mensaje, igual que en las otras cuatro escrituras, y no es un error.
    #[tokio::test]
    #[serial]
    async fn test_set_label_reports_an_already_applied_conflict() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_list(
            &server,
            json!([ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &["lbl-shared"],
                None
            )]),
        )
        .await;
        mount_labels(&server, &[("lbl-shared", "shared"), ("lbl-doing", "doing")]).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(409).set_body_string("idempotency key reusada"))
            .expect(1)
            .mount(&server)
            .await;

        let result = tool
            .execute(json!({
                "operation": "set_label", "id": "t1", "group": "state", "value": "doing"
            }))
            .await
            .expect("un 409 no es un error del turno");

        assert!(result.success);
        let message = result
            .message
            .expect("la herramienta siempre informa del resultado");
        assert!(
            message.contains("ya estaba aplicada"),
            "el 409 debe reflejarse en el mensaje: {message}"
        );
        assert_eq!(result.data["value"], json!("doing"));
        assert_eq!(result.data["id"], json!("t1"));
    }

    // -----------------------------------------------------------------------
    // open-todo redux: errores y camino local
    // -----------------------------------------------------------------------

    /// Scenario: un fallo del servicio externo no rompe el turno y el mensaje no
    /// filtra el token.
    #[tokio::test]
    #[serial]
    async fn test_opentodo_failure_is_a_readable_error_without_the_token() {
        for status in [503u16, 401, 404] {
            let (_pool, server, tool) = open_todo_setup().await;
            Mock::given(method("GET"))
                .and(path("/api/v1/tasks"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let error = tool
                .execute(json!({ "operation": "list_tasks" }))
                .await
                .expect_err("un fallo de OpenTodo debe ser un ToolError");

            let message = error.to_string();
            assert!(
                message.contains("OpenTodo"),
                "el mensaje debe decir que OpenTodo no responde (status {status}): {message}"
            );
            assert!(
                !message.contains(TOKEN),
                "el mensaje no debe contener el token (status {status}): {message}"
            );
        }
    }

    /// Scenario: sin token, el comportamiento local se conserva.
    #[tokio::test]
    #[serial]
    async fn test_local_backend_is_preserved_without_a_token() {
        std::env::remove_var("OPENTODO_TOKEN");
        std::env::remove_var("OPENTODO_BASE_URL");
        let (pool, tool) = setup().await.unwrap();

        // `profile_id` se sigue exigiendo.
        let error = tool
            .execute(json!({ "operation": "add_task", "content": "Preparar informe" }))
            .await
            .expect_err("sin profile_id el camino local debe rechazar la llamada");
        assert!(
            matches!(error, ToolError::InvalidArguments(_)),
            "err: {error:?}"
        );

        // Y la tarea se crea en la tabla local con sus valores por defecto.
        let result = tool
            .execute(json!({
                "operation": "add_task", "profile_id": "profile-1", "content": "Preparar informe"
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["content"], json!("Preparar informe"));
        assert_eq!(result.data["status"], json!("inbox"));
        assert_eq!(result.data["priority"], json!("medium"));
        assert_eq!(result.data["scope"], json!("shared"));

        let local = TasksRepo::list(&pool, "profile-1", None, None, None, None)
            .await
            .unwrap();
        assert_eq!(local.len(), 1, "la tarea vive en la tabla local");
        assert_eq!(local[0].content, "Preparar informe");
    }

    /// Las operaciones que solo existen en OpenTodo lo dicen claramente cuando
    /// no hay token, en vez de fallar por dentro.
    #[tokio::test]
    #[serial]
    async fn test_opentodo_only_operations_explain_they_need_a_token() {
        std::env::remove_var("OPENTODO_TOKEN");
        std::env::remove_var("OPENTODO_BASE_URL");
        let (_, tool) = setup().await.unwrap();

        for args in [
            json!({ "operation": "search_tasks", "query": "informe" }),
            json!({ "operation": "set_label", "id": "t1", "group": "state", "value": "doing" }),
        ] {
            let error = tool.execute(args.clone()).await.expect_err("debe fallar");
            let message = error.to_string();
            assert!(
                message.contains("OpenTodo"),
                "el mensaje debe explicar que hace falta configurar OpenTodo: {message}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Auditoría de opentodo-integration — H5, H6, H7, H9 y H10
    // -----------------------------------------------------------------------

    /// H5 — una variable de entorno exportada por el desarrollador no puede
    /// sacar a los tests locales de su backend (y mucho menos apuntarlos a la
    /// instancia real).
    #[tokio::test]
    #[serial]
    async fn test_local_backend_ignores_a_stray_environment_token() {
        std::env::set_var("OPENTODO_TOKEN", "SECRET");
        // Dirección que no responde: si alguien acaba ahí, falla en el acto.
        std::env::set_var("OPENTODO_BASE_URL", "http://127.0.0.1:1");

        let (pool, tool) = setup().await.unwrap();
        let result = tool
            .execute(json!({
                "operation": "add_task", "profile_id": "profile-1", "content": "Local"
            }))
            .await;

        std::env::remove_var("OPENTODO_TOKEN");
        std::env::remove_var("OPENTODO_BASE_URL");

        let result = result.expect("`setup()` aísla el entorno: el camino local debe responder");
        assert!(result.success);
        assert_eq!(result.data["status"], json!("inbox"));

        let local = TasksRepo::list(&pool, "profile-1", None, None, None, None)
            .await
            .unwrap();
        assert_eq!(local.len(), 1, "la tarea se crea en la tabla local");
    }

    /// H6 — `start: null` limpia el campo, una cadena no vacía lo fija y
    /// cualquier otro tipo (incluida la cadena vacía) es un argumento inválido.
    #[tokio::test]
    #[serial]
    async fn test_update_task_start_semantics() {
        for (value, expected_body) in [
            (json!(null), json!({ "start": null })),
            (json!("someday"), json!({ "start": "someday" })),
        ] {
            let (_pool, server, tool) = open_todo_setup().await;
            Mock::given(method("PATCH"))
                .and(path("/api/v1/tasks/t1"))
                .and(body_json(expected_body.clone()))
                .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                    "t1",
                    "Tarea",
                    "",
                    None,
                    None,
                    &[],
                    None,
                )))
                .expect(1)
                .mount(&server)
                .await;

            tool.execute(json!({ "operation": "update_task", "id": "t1", "start": value }))
                .await
                .unwrap_or_else(|error| panic!("start {value}: {error:?}"));
        }

        for value in [json!(true), json!(""), json!(7)] {
            let (_pool, server, tool) = open_todo_setup().await;
            let error = tool
                .execute(json!({ "operation": "update_task", "id": "t1", "start": value }))
                .await
                .expect_err("un `start` que no sea cadena no vacía debe rechazarse");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "start {value}: {error:?}"
            );
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "start {value} no debe salir a la red"
            );
        }
    }

    /// H6 — `due: null` limpia la fecha; un `due` de otro tipo es inválido, en
    /// `update_task` y en `add_task`.
    #[tokio::test]
    #[serial]
    async fn test_due_semantics() {
        // `null` → se limpia.
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(json!({ "due": null })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "t1",
                "Tarea",
                "",
                None,
                None,
                &[],
                None,
            )))
            .expect(1)
            .mount(&server)
            .await;
        tool.execute(json!({ "operation": "update_task", "id": "t1", "due": null }))
            .await
            .unwrap();

        // Cadena ISO → se fija sin pasar por quick-add.
        let (_pool, server, tool) = open_todo_setup().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(json!({ "due": { "date": "2026-10-30" } })))
            .respond_with(ResponseTemplate::new(200).set_body_json(ot_task(
                "t1",
                "Tarea",
                "",
                None,
                Some("2026-10-30"),
                &[],
                None,
            )))
            .expect(1)
            .mount(&server)
            .await;
        tool.execute(json!({ "operation": "update_task", "id": "t1", "due": "2026-10-30" }))
            .await
            .unwrap();

        // Otro tipo → argumento inválido y sin red.
        for args in [
            json!({ "operation": "update_task", "id": "t1", "due": 5 }),
            json!({ "operation": "update_task", "id": "t1", "due": true }),
            json!({ "operation": "add_task", "title": "X", "due": 5 }),
        ] {
            let (_pool, server, tool) = open_todo_setup().await;
            let error = tool
                .execute(args.clone())
                .await
                .expect_err("un `due` que no sea cadena debe rechazarse");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "{args}: {error:?}"
            );
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "{args} no debe salir a la red"
            );
        }
    }

    /// H7 — la proyección nunca devuelve un id de etiqueta sin resolver.
    #[tokio::test]
    #[serial]
    async fn test_list_tasks_never_projects_unresolved_label_ids() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_me(&server, "u1").await;
        mount_projects(&server, &[]).await;
        mount_labels(&server, &[("lbl-shared", "shared")]).await;
        mount_list(
            &server,
            json!([
                ot_task(
                    "t1",
                    "Con id resuelto",
                    "",
                    None,
                    None,
                    &["lbl-shared", "lbl-huerfano"],
                    None
                ),
                ot_task(
                    "t2",
                    "Con id desconocido",
                    "",
                    None,
                    None,
                    &["lbl-huerfano"],
                    None
                ),
            ]),
        )
        .await;

        let result = tool
            .execute(json!({ "operation": "list_tasks" }))
            .await
            .unwrap();

        let listed = result.data["tasks"].as_array().unwrap();
        assert_eq!(
            listed[0]["labels"],
            json!(["shared"]),
            "solo sale lo que se resuelve a un nombre"
        );
        assert_eq!(
            listed[1]["labels"],
            json!([]),
            "un id que no resuelve no se devuelve como si fuera un nombre"
        );
    }

    /// H9 — un tipo que no sea cadena no se trata como ausente.
    #[tokio::test]
    #[serial]
    async fn test_rejects_non_string_arguments() {
        let (_pool, server, tool) = open_todo_setup().await;

        for args in [
            json!({ "operation": "list_tasks", "view": 5 }),
            json!({ "operation": "list_tasks", "project": 7 }),
            json!({ "operation": "list_tasks", "limit": "20" }),
            json!({ "operation": "search_tasks", "query": 7 }),
            json!({ "operation": "add_task", "title": 7 }),
            json!({ "operation": "add_task", "priority": 3 }),
            json!({ "operation": "update_task", "id": 7 }),
        ] {
            let error = tool
                .execute(args.clone())
                .await
                .expect_err("un tipo incorrecto es un argumento inválido");
            assert!(
                matches!(error, ToolError::InvalidArguments(_)),
                "{args}: {error:?}"
            );
        }

        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "ningún argumento mal tipado debe salir a la red"
        );
    }

    /// H10 — el filtro por **nombre** de proyecto por el camino de OpenTodo.
    #[tokio::test]
    #[serial]
    async fn test_list_tasks_filters_by_project_name() {
        let (_pool, server, tool) = open_todo_setup().await;
        let today = Local::now().date_naive();
        let yesterday = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        let next_month = (today + Duration::days(30)).format("%Y-%m-%d").to_string();

        mount_me(&server, "u1").await;
        mount_labels(&server, &[]).await;
        mount_projects(&server, &[("p1", "Casa"), ("p2", "Trabajo")]).await;
        mount_list(
            &server,
            json!([
                ot_task(
                    "casa-vencida",
                    "Casa vencida",
                    "",
                    Some("p1"),
                    Some(&yesterday),
                    &[],
                    None
                ),
                ot_task(
                    "trabajo-vencida",
                    "Trabajo vencida",
                    "",
                    Some("p2"),
                    Some(&yesterday),
                    &[],
                    None
                ),
                ot_task(
                    "trabajo-futura",
                    "Trabajo futura",
                    "",
                    Some("p2"),
                    Some(&next_month),
                    &[],
                    None
                ),
            ]),
        )
        .await;

        let result = tool
            .execute(json!({
                "operation": "list_tasks", "view": "today", "project": "Trabajo"
            }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], json!(1));
        let listed = result.data["tasks"].as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["id"], json!("trabajo-vencida"));
        assert_eq!(
            listed[0]["project"],
            json!("Trabajo"),
            "la salida lleva el nombre del proyecto"
        );
        // El resumen sigue describiendo la cuenta entera.
        assert_eq!(result.data["summary"]["by_view"]["today"], json!(2));
    }

    /// H10 — la vista `waiting` usa el `me_id` de `GET /api/v1/me`: lo asignado
    /// a otro usuario espera, lo mío no.
    #[tokio::test]
    #[serial]
    async fn test_waiting_view_uses_the_authenticated_user() {
        let (_pool, server, tool) = open_todo_setup().await;
        mount_me(&server, "u1").await;
        mount_labels(&server, &[]).await;
        mount_projects(&server, &[]).await;

        let mut mine = ot_task("mio", "Mía", "", None, None, &[], None);
        mine["assignee_id"] = json!("u1");
        let mut theirs = ot_task("ajeno", "Ajena", "", None, None, &[], None);
        theirs["assignee_id"] = json!("u2");
        let blocked = ot_task("bloqueada", "Bloqueada", "", None, None, &[], None);
        let mut blocked = blocked;
        blocked["blocked_by"] = json!(["t0"]);

        mount_list(&server, json!([mine, theirs, blocked])).await;

        let result = tool
            .execute(json!({ "operation": "list_tasks", "view": "waiting" }))
            .await
            .unwrap();

        assert_eq!(result.data["count"], json!(2));
        let ids: Vec<&str> = result.data["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|task| task["id"].as_str())
            .collect();
        assert!(ids.contains(&"ajeno"), "lo asignado a otro espera: {ids:?}");
        assert!(ids.contains(&"bloqueada"), "lo bloqueado espera: {ids:?}");
        assert!(!ids.contains(&"mio"), "lo mío no espera: {ids:?}");
    }
}
