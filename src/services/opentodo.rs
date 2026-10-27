//! Cliente de la API HTTP v1 de `OpenTodo`.
//!
//! Encapsula la resolución de la configuración (ajustes → entorno → valor por
//! defecto solo para la URL) y las operaciones que consumirá la herramienta
//! `tasks`. El token viaja en la cabecera `Authorization: Bearer <token>` y
//! **nunca** debe aparecer ni en un `Debug`, ni en un log, ni en el mensaje de
//! un [`OpentodoError`].
//!
//! Es el molde de [`crate::services::zennotes`]: misma resolución de
//! credenciales en cada llamada y misma traducción de errores. La diferencia
//! importante es que `GET /api/v1/tasks` **no filtra**: las vistas del GTD son
//! predicados del cliente y se recalculan en Rust con [`matches_view`].

use crate::db::repos::settings::SettingsRepo;
use chrono::{NaiveDate, Utc};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap, HashSet};

/// URL base por defecto de OpenTodo cuando ni el ajuste ni la variable de
/// entorno la definen.
pub const DEFAULT_BASE_URL: &str = "https://todo.territoriolinux.es";

/// Etiquetas del grupo «estado» (son mutuamente excluyentes entre sí).
pub const STATE_LABELS: &[&str] = &["doing", "waiting"];

/// Etiquetas del grupo «ámbito» (son mutuamente excluyentes entre sí).
pub const SCOPE_LABELS: &[&str] = &["shared", "personal"];

/// User-Agent propio del cliente.
const USER_AGENT: &str = "valet/1.0";

/// Timeout de cada petición.
///
/// Forma parte del contrato del cliente (10 s), por eso es público: un cambio
/// aquí tiene que ser deliberado.
pub const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Errores del cliente de OpenTodo.
///
/// [`OpentodoError::NotConfigured`] (no hay token configurado) es una variante
/// **distinta** de [`OpentodoError::Unauthorized`] (el token no vale): la
/// primera indica dónde configurarlo y la segunda que ha caducado o es
/// incorrecto.
#[derive(Debug)]
pub enum OpentodoError {
    /// No hay token ni en `settings` ni en el entorno.
    NotConfigured,
    /// `401`/`403` — el token no es válido, ha caducado o no tiene permiso.
    Unauthorized,
    /// `404` — la tarea (o el recurso) no existe.
    NotFound,
    /// `400` — el servidor rechazó la petición; el mensaje es el suyo (ya
    /// redactado, nunca contiene el token).
    InvalidInput(String),
    /// Fallo de red o `5xx` — OpenTodo no está disponible.
    Upstream(String),
}

impl std::fmt::Display for OpentodoError {
    /// Mensajes accionables en español. **Ninguno** incluye el token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpentodoError::NotConfigured => write!(
                f,
                "no hay token de OpenTodo configurado: defínelo en Ajustes → Integraciones → OpenTodo o en OPENTODO_TOKEN"
            ),
            OpentodoError::Unauthorized => {
                write!(f, "el token de OpenTodo no es válido o ha caducado")
            }
            OpentodoError::NotFound => write!(f, "la tarea solicitada no existe en OpenTodo"),
            OpentodoError::InvalidInput(detail) => {
                write!(f, "OpenTodo rechazó la petición: {detail}")
            }
            OpentodoError::Upstream(detail) => {
                write!(f, "OpenTodo no está disponible: {detail}")
            }
        }
    }
}

impl std::error::Error for OpentodoError {}

/// Configuración resuelta de OpenTodo.
#[derive(Clone, PartialEq)]
pub struct OpentodoConfig {
    pub base_url: String,
    pub token: String,
}

impl std::fmt::Debug for OpentodoConfig {
    /// Redacta el token: nunca debe aparecer en un `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpentodoConfig")
            .field("base_url", &self.base_url)
            .field("token", &"***")
            .finish()
    }
}

/// Prioridad de una tarea: `1` es la más alta (`critical`) y `4` la más baja
/// (`low`, el valor con el que el servidor nace sus tareas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Priority {
    /// Código numérico que viaja en la API (`1..=4`).
    pub fn code(self) -> u8 {
        match self {
            Priority::Critical => 1,
            Priority::High => 2,
            Priority::Medium => 3,
            Priority::Low => 4,
        }
    }

    /// Prioridad a partir de su código numérico; fuera de `1..=4`, `None`.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Priority::Critical),
            2 => Some(Priority::High),
            3 => Some(Priority::Medium),
            4 => Some(Priority::Low),
            _ => None,
        }
    }

    /// Prioridad a partir de su nombre textual (`critical`…`low`).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "critical" => Some(Priority::Critical),
            "high" => Some(Priority::High),
            "medium" => Some(Priority::Medium),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }

    /// Nombre textual (`critical`…`low`).
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }
}

/// Fecha de OpenTodo: `{date, time?, tz?}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Due {
    /// `YYYY-MM-DD`.
    pub date: String,
    /// `HH:MM`, si la tarea tiene hora.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Zona horaria declarada por el usuario, si la hay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
}

impl Due {
    /// Fecha del `due` como `NaiveDate`, o `None` si está vacía o mal formada.
    pub fn as_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(self.date.trim(), "%Y-%m-%d").ok()
    }
}

/// Tarea tal y como la devuelve `GET /api/v1/tasks`.
///
/// Todos los campos son tolerantes: `#[serde(default)]` a nivel de estructura,
/// de modo que una respuesta con campos ausentes o desconocidos deserializa sin
/// error.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    /// `1..=4`.
    pub priority: Option<u8>,
    pub due: Option<Due>,
    pub deadline: Option<Due>,
    /// `"someday"` para las tareas de «Algún día».
    pub start: Option<String>,
    /// Ids (o nombres, según la instancia) de las etiquetas de la tarea.
    pub labels: Vec<String>,
    /// Milisegundos Unix; `None` mientras la tarea está abierta.
    pub completed_at: Option<i64>,
    pub deleted_at: Option<i64>,
    pub blocked_by: Vec<String>,
    pub assignee_id: Option<String>,
    /// Nombres resueltos de las etiquetas. **No viene del servidor**: lo rellena
    /// [`apply_label_names`] cuando `labels` trae ids.
    #[serde(skip)]
    pub label_names: Vec<String>,
}

impl Task {
    /// `true` si la tarea no está completada (definición de «abierta» del GTD).
    pub fn is_open(&self) -> bool {
        self.completed_at.is_none()
    }

    /// `true` si la tarea lleva la etiqueta `name`, ya venga como nombre en
    /// `labels` o como id resuelto en `label_names`.
    pub fn has_label(&self, name: &str) -> bool {
        self.labels.iter().any(|label| label == name)
            || self.label_names.iter().any(|label| label == name)
    }

    /// Prioridad como texto (`critical`…`low`), o `None` si no la trae.
    pub fn priority_label(&self) -> Option<&'static str> {
        self.priority
            .and_then(Priority::from_code)
            .map(Priority::as_str)
    }
}

/// Etiqueta de OpenTodo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct Label {
    pub id: String,
    pub name: String,
}

/// Proyecto de OpenTodo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub struct Project {
    pub id: String,
    pub name: String,
}

/// Grupo de etiquetas mutuamente excluyente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelGroup {
    /// `doing` / `waiting`.
    State,
    /// `shared` / `personal`.
    Scope,
}

impl LabelGroup {
    /// Grupo a partir de su nombre (`state`|`scope`).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "state" => Some(LabelGroup::State),
            "scope" => Some(LabelGroup::Scope),
            _ => None,
        }
    }

    /// Nombre del grupo (`state`|`scope`).
    pub fn as_str(self) -> &'static str {
        match self {
            LabelGroup::State => "state",
            LabelGroup::Scope => "scope",
        }
    }

    /// Etiquetas que componen el grupo (la primera es la de `state`/`scope`).
    pub fn labels(self) -> &'static [&'static str] {
        match self {
            LabelGroup::State => STATE_LABELS,
            LabelGroup::Scope => SCOPE_LABELS,
        }
    }

    /// `true` si `value` pertenece a este grupo.
    pub fn accepts(self, value: &str) -> bool {
        self.labels().contains(&value)
    }
}

/// Vista del GTD, calculada en Rust porque el servidor no filtra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    All,
    Inbox,
    Today,
    Upcoming,
    Anytime,
    Someday,
    Doing,
    Waiting,
}

impl View {
    /// Las ocho vistas admitidas.
    pub const ALL: [View; 8] = [
        View::All,
        View::Inbox,
        View::Today,
        View::Upcoming,
        View::Anytime,
        View::Someday,
        View::Doing,
        View::Waiting,
    ];

    /// Las siete vistas que se agregan en [`Summary::by_view`] (`all` queda
    /// fuera porque no aporta información sobre el resto).
    pub const SUMMARY_VIEWS: [View; 7] = [
        View::Inbox,
        View::Today,
        View::Upcoming,
        View::Anytime,
        View::Someday,
        View::Doing,
        View::Waiting,
    ];

    /// Vista a partir de su nombre (`all`, `inbox`, …).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "all" => Some(View::All),
            "inbox" => Some(View::Inbox),
            "today" => Some(View::Today),
            "upcoming" => Some(View::Upcoming),
            "anytime" => Some(View::Anytime),
            "someday" => Some(View::Someday),
            "doing" => Some(View::Doing),
            "waiting" => Some(View::Waiting),
            _ => None,
        }
    }

    /// Nombre de la vista.
    pub fn as_str(self) -> &'static str {
        match self {
            View::All => "all",
            View::Inbox => "inbox",
            View::Today => "today",
            View::Upcoming => "upcoming",
            View::Anytime => "anytime",
            View::Someday => "someday",
            View::Doing => "doing",
            View::Waiting => "waiting",
        }
    }
}

/// Resumen agregado de un listado: cuántas tareas caen en cada vista y en cada
/// proyecto.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    /// Totales por vista (las siete de [`View::SUMMARY_VIEWS`]).
    pub by_view: BTreeMap<String, usize>,
    /// Totales por nombre de proyecto (`(sin proyecto)` para la bandeja).
    pub by_project: BTreeMap<String, usize>,
}

/// Resultado de una escritura.
#[derive(Debug, Clone, Default)]
pub struct WriteOutcome {
    /// Tarea devuelta por el servidor, si la devolvió.
    pub task: Option<Task>,
    /// `true` si el servidor respondió `409` a la `Idempotency-Key`: la
    /// operación **ya estaba aplicada** y no se reintenta.
    pub already_applied: bool,
}

/// Datos para crear una tarea.
#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    /// Si se omite se envía `low` (`4`), el valor por defecto del servidor.
    pub priority: Option<Priority>,
    pub due: Option<Due>,
    pub labels: Option<Vec<String>>,
    /// `"someday"` para «Algún día».
    pub start: Option<String>,
}

/// Campos modificables de una tarea. `None` = no se toca; `Some(None)` en los
/// campos anulables = se limpia en el servidor.
#[derive(Debug, Clone, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub project_id: Option<Option<String>>,
    pub priority: Option<Priority>,
    pub due: Option<Option<Due>>,
    pub start: Option<Option<String>>,
    pub labels: Option<Vec<String>>,
}

/// Resultado de [`Opentodo::set_label`].
#[derive(Debug, Clone, Default)]
pub struct SetLabelOutcome {
    /// Nombre de la etiqueta creada en la llamada, si hubo que crearla.
    pub created_label: Option<String>,
    /// Ids de etiqueta enviados en el `PATCH` (array completo).
    pub labels: Vec<String>,
    /// `true` si el `PATCH` respondió `409` (ya aplicado).
    pub already_applied: bool,
}

/// Cliente de OpenTodo atado a una configuración.
#[derive(Debug, Clone)]
pub struct Opentodo {
    http: reqwest::Client,
    config: OpentodoConfig,
}

/// Respuesta HTTP ya normalizada.
enum Reply {
    /// Cuerpo JSON interpretable.
    Body(serde_json::Value),
    /// Éxito sin cuerpo (`204`).
    Empty,
    /// `409` de la `Idempotency-Key`: la operación ya estaba aplicada.
    AlreadyApplied,
}

impl Opentodo {
    /// Cliente con la configuración dada, `user_agent` propio y timeout de 10 s.
    ///
    /// La base es **inyectable** para poder apuntar los tests a un servidor de
    /// prueba con `wiremock`.
    ///
    /// La construcción es falible: si `reqwest` no puede inicializar su backend
    /// TLS se devuelve [`OpentodoError::Upstream`] en vez de caer a un cliente
    /// sin `user_agent` ni timeout.
    pub fn new(config: OpentodoConfig) -> Result<Self, OpentodoError> {
        let token = config.token.trim().to_string();
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|error| {
                let detail = format!("no se pudo inicializar el cliente HTTP: {error}");
                let detail = if token.is_empty() {
                    detail
                } else {
                    detail.replace(&token, "***")
                };
                OpentodoError::Upstream(detail)
            })?;

        Ok(Self {
            http,
            config: OpentodoConfig {
                base_url: config.base_url.trim_end_matches('/').to_string(),
                token,
            },
        })
    }

    /// URL base resuelta (settings → entorno → [`DEFAULT_BASE_URL`]). Nunca
    /// falla: un error de lectura de settings se trata como ausencia.
    pub async fn resolve_base_url(pool: &SqlitePool) -> String {
        setting_or_env(pool, "opentodo_base_url", "OPENTODO_BASE_URL")
            .await
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
    }

    /// Token resuelto (settings → entorno), o `None` si no hay ninguno.
    pub async fn resolve_token(pool: &SqlitePool) -> Option<String> {
        setting_or_env(pool, "opentodo_token", "OPENTODO_TOKEN").await
    }

    /// `true` si hay token configurado, sin construir el cliente.
    pub async fn is_configured(pool: &SqlitePool) -> bool {
        Self::resolve_token(pool).await.is_some()
    }

    /// Resuelve la configuración y construye el cliente. Sin token devuelve
    /// [`OpentodoError::NotConfigured`]. Las claves se leen en **cada llamada**,
    /// de modo que editarlas surte efecto sin reiniciar. Un fallo al construir
    /// el cliente HTTP se propaga tal cual (nunca entra en pánico).
    pub async fn from_settings(pool: &SqlitePool) -> Result<Self, OpentodoError> {
        let base_url = Self::resolve_base_url(pool).await;

        match Self::resolve_token(pool).await {
            Some(token) => Self::new(OpentodoConfig { base_url, token }),
            None => Err(OpentodoError::NotConfigured),
        }
    }

    /// URL base resuelta.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// `GET /api/v1/me` — perfil del usuario autenticado, tal cual.
    pub async fn me(&self) -> Result<serde_json::Value, OpentodoError> {
        self.get_json("/api/v1/me").await
    }

    /// `GET /api/v1/tasks` — lista cruda, **sin filtrar** (el servidor no sabe
    /// filtrar por vista).
    pub async fn list_tasks(&self) -> Result<Vec<Task>, OpentodoError> {
        let value = self.get_json("/api/v1/tasks").await?;
        let raw = value.get("tasks").cloned().unwrap_or(value);
        serde_json::from_value::<Vec<Task>>(raw)
            .map_err(|error| self.upstream(&format!("lista de tareas ilegible: {error}")))
    }

    /// `GET /api/v1/projects` — proyectos de la cuenta.
    pub async fn list_projects(&self) -> Result<Vec<Project>, OpentodoError> {
        let value = self.get_json("/api/v1/projects").await?;
        let raw = value.get("projects").cloned().unwrap_or(value);
        serde_json::from_value::<Vec<Project>>(raw)
            .map_err(|error| self.upstream(&format!("lista de proyectos ilegible: {error}")))
    }

    /// `GET /api/v1/labels` — etiquetas de la cuenta.
    pub async fn list_labels(&self) -> Result<Vec<Label>, OpentodoError> {
        let value = self.get_json("/api/v1/labels").await?;
        let raw = value.get("labels").cloned().unwrap_or(value);
        serde_json::from_value::<Vec<Label>>(raw)
            .map_err(|error| self.upstream(&format!("lista de etiquetas ilegible: {error}")))
    }

    /// `POST /api/v1/tasks` — crea la tarea con la prioridad **siempre**
    /// explícita (`low` si no se indica).
    pub async fn create_task(&self, task: &NewTask) -> Result<WriteOutcome, OpentodoError> {
        let mut payload = serde_json::Map::new();
        payload.insert("title".to_string(), serde_json::json!(task.title));
        payload.insert(
            "priority".to_string(),
            serde_json::json!(task.priority.unwrap_or(Priority::Low).code()),
        );
        insert_optional(&mut payload, "description", task.description.as_deref());
        insert_optional(&mut payload, "project_id", task.project_id.as_deref());
        insert_optional(&mut payload, "start", task.start.as_deref());
        if let Some(due) = &task.due {
            payload.insert("due".to_string(), serde_json::json!(due));
        }
        if let Some(labels) = &task.labels {
            payload.insert("labels".to_string(), serde_json::json!(labels));
        }

        let request = self
            .idempotent(self.http.post(self.url("/api/v1/tasks")))
            .json(&serde_json::Value::Object(payload));
        self.write(request).await
    }

    /// `PATCH /api/v1/tasks/{id}` — modifica solo los campos presentes.
    pub async fn update_task(
        &self,
        id: &str,
        patch: &TaskPatch,
    ) -> Result<WriteOutcome, OpentodoError> {
        let mut payload = serde_json::Map::new();
        insert_optional(&mut payload, "title", patch.title.as_deref());
        insert_optional(&mut payload, "description", patch.description.as_deref());
        if let Some(project_id) = &patch.project_id {
            payload.insert("project_id".to_string(), serde_json::json!(project_id));
        }
        if let Some(priority) = patch.priority {
            payload.insert("priority".to_string(), serde_json::json!(priority.code()));
        }
        if let Some(due) = &patch.due {
            payload.insert("due".to_string(), serde_json::json!(due));
        }
        if let Some(start) = &patch.start {
            payload.insert("start".to_string(), serde_json::json!(start));
        }
        if let Some(labels) = &patch.labels {
            payload.insert("labels".to_string(), serde_json::json!(labels));
        }

        let request = self
            .idempotent(self.http.patch(self.url(&format!("/api/v1/tasks/{id}"))))
            .json(&serde_json::Value::Object(payload));
        self.write(request).await
    }

    /// `PATCH /api/v1/tasks/{id}` con `completed_at` al instante actual.
    pub async fn complete_task(&self, id: &str) -> Result<WriteOutcome, OpentodoError> {
        let payload = serde_json::json!({ "completed_at": Utc::now().timestamp_millis() });
        let request = self
            .idempotent(self.http.patch(self.url(&format!("/api/v1/tasks/{id}"))))
            .json(&payload);
        self.write(request).await
    }

    /// `DELETE /api/v1/tasks/{id}` o `DELETE /api/v1/tasks/{id}/purge`.
    pub async fn delete_task(&self, id: &str, purge: bool) -> Result<WriteOutcome, OpentodoError> {
        let endpoint = if purge {
            format!("/api/v1/tasks/{id}/purge")
        } else {
            format!("/api/v1/tasks/{id}")
        };
        let request = self.idempotent(self.http.delete(self.url(&endpoint)));
        self.write(request).await
    }

    /// Resuelve un `due`: si ya es ISO lo devuelve tal cual (sin red) y, si es
    /// una expresión natural, delega en `POST /api/v1/quick-add/parse`.
    pub async fn parse_natural_due(&self, text: &str) -> Result<Option<Due>, OpentodoError> {
        if let Some(due) = parse_iso_due(text) {
            return Ok(Some(due));
        }

        let payload = serde_json::json!({ "text": text });
        let request = self
            .http
            .post(self.url("/api/v1/quick-add/parse"))
            .json(&payload);

        match self.send(request).await? {
            Reply::Body(value) => {
                let raw = value.get("due").cloned().unwrap_or(value);
                Ok(serde_json::from_value::<Due>(raw).ok())
            }
            Reply::Empty | Reply::AlreadyApplied => Ok(None),
        }
    }

    /// Fija (o quita, con `value: None`) la etiqueta de un grupo sin tocar las
    /// demás: lee la tarea, resuelve los ids del grupo por nombre exacto, crea
    /// la etiqueta si falta y manda el **array completo** en un único `PATCH`.
    pub async fn set_label(
        &self,
        id: &str,
        group: LabelGroup,
        value: Option<&str>,
    ) -> Result<SetLabelOutcome, OpentodoError> {
        if let Some(value) = value {
            if !group.accepts(value) {
                return Err(OpentodoError::InvalidInput(format!(
                    "`{value}` no pertenece al grupo `{}` (valores: {})",
                    group.as_str(),
                    group.labels().join(", ")
                )));
            }
        }

        let task = self
            .list_tasks()
            .await?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or(OpentodoError::NotFound)?;
        let catalog = self.list_labels().await?;

        // «Lo que hay que quitar»: las etiquetas del grupo, identificadas por su
        // **nombre** (una instancia puede devolver nombres en `labels`) y por su
        // **id** en el catálogo. Comparar solo contra los ids dejaba el nombre
        // en la lista y, con `value: None`, se reenviaba el array intacto
        // informando de un éxito que no había ocurrido.
        let mut group_marks: HashSet<&str> = group.labels().iter().copied().collect();
        for label in &catalog {
            if group.accepts(&label.name) {
                group_marks.insert(label.id.as_str());
            }
        }

        // «Actuales − las del grupo»: el orden de las que sobreviven se respeta,
        // y la nueva se añade al final.
        let mut labels: Vec<String> = task
            .labels
            .iter()
            .filter(|label| !group_marks.contains(label.as_str()))
            .cloned()
            .collect();

        let mut created_label = None;
        if let Some(value) = value {
            let label_id = match catalog.iter().find(|label| label.name == value) {
                Some(existing) => existing.id.clone(),
                None => {
                    let (label, created) = self.create_label(value).await?;
                    if created {
                        created_label = Some(value.to_string());
                    }
                    label.id
                }
            };
            labels.push(label_id);
        }

        let payload = serde_json::json!({ "labels": labels });
        let request = self
            .idempotent(self.http.patch(self.url(&format!("/api/v1/tasks/{id}"))))
            .json(&payload);

        let already_applied = matches!(self.send(request).await?, Reply::AlreadyApplied);

        Ok(SetLabelOutcome {
            created_label,
            labels,
            already_applied,
        })
    }

    // -- infraestructura HTTP ------------------------------------------------

    /// Crea una etiqueta en el espacio por defecto. Devuelve la etiqueta y si
    /// hubo que crearla de verdad (`false` cuando un `409` revela que ya
    /// existía y se adopta la del catálogo).
    async fn create_label(&self, name: &str) -> Result<(Label, bool), OpentodoError> {
        let payload = serde_json::json!({ "name": name });
        let request = self
            .idempotent(self.http.post(self.url("/api/v1/labels")))
            .json(&payload);

        match self.send(request).await? {
            Reply::Body(value) => {
                let raw = value.get("label").cloned().unwrap_or(value);
                let label = serde_json::from_value::<Label>(raw).map_err(|error| {
                    self.upstream(&format!("etiqueta creada ilegible: {error}"))
                })?;
                Ok((label, true))
            }
            Reply::AlreadyApplied => {
                let adopted = self
                    .list_labels()
                    .await?
                    .into_iter()
                    .find(|label| label.name == name)
                    .ok_or_else(|| self.upstream(&format!("la etiqueta `{name}` ya existía")))?;
                Ok((adopted, false))
            }
            Reply::Empty => Err(self.upstream("la creación de la etiqueta no devolvió cuerpo")),
        }
    }

    /// URL absoluta de un endpoint.
    fn url(&self, endpoint: &str) -> String {
        format!("{}{endpoint}", self.config.base_url)
    }

    /// Rechaza un token vacío o en blanco **antes** de tocar la red.
    fn require_token(&self) -> Result<(), OpentodoError> {
        if self.config.token.trim().is_empty() {
            Err(OpentodoError::NotConfigured)
        } else {
            Ok(())
        }
    }

    /// Borra el token de cualquier texto que vaya a salir en un error.
    fn redact(&self, text: &str) -> String {
        let token = self.config.token.trim();
        if token.is_empty() {
            text.to_string()
        } else {
            text.replace(token, "***")
        }
    }

    /// Embudo único de los errores con **texto ajeno**: siempre pasa por
    /// [`Opentodo::redact`].
    ///
    /// Es obligatorio usarlo para cualquier detalle que venga del servidor o de
    /// una librería, porque los errores de `serde_json` citan el valor recibido
    /// (`invalid type: string "<token>", expected u8`) y el token viaja en el
    /// cuerpo de la petición.
    fn upstream(&self, detail: &str) -> OpentodoError {
        OpentodoError::Upstream(self.redact(detail))
    }

    /// Igual que [`Opentodo::upstream`] para los errores de validación que
    /// llevan un mensaje del servidor.
    fn invalid_input(&self, detail: &str) -> OpentodoError {
        OpentodoError::InvalidInput(self.redact(detail))
    }

    /// Marca una petición de escritura con una `Idempotency-Key` **nueva**.
    ///
    /// Sin esta cabecera un `409` no puede significar «ya aplicado» y el aviso
    /// sería una mentira; con ella, un reenvío accidental se detecta en el
    /// servidor y se traduce a [`Reply::AlreadyApplied`].
    fn idempotent(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
    }

    /// Envía la petición con el `Bearer` y normaliza la respuesta.
    async fn send(&self, request: reqwest::RequestBuilder) -> Result<Reply, OpentodoError> {
        self.require_token()?;

        let response = request
            .bearer_auth(self.config.token.trim())
            .send()
            .await
            .map_err(|error| self.upstream(&format!("fallo de red: {error}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| self.upstream(&format!("respuesta ilegible: {error}")))?;

        match status {
            409 => Ok(Reply::AlreadyApplied),
            status if (200..300).contains(&status) => {
                if body.trim().is_empty() {
                    Ok(Reply::Empty)
                } else {
                    serde_json::from_str::<serde_json::Value>(&body)
                        .map(Reply::Body)
                        .map_err(|error| self.upstream(&format!("respuesta no JSON: {error}")))
                }
            }
            status => Err(self.error_from_status(status, &body)),
        }
    }

    /// `GET` que espera un objeto JSON.
    async fn get_json(&self, endpoint: &str) -> Result<serde_json::Value, OpentodoError> {
        match self.send(self.http.get(self.url(endpoint))).await? {
            Reply::Body(value) => Ok(value),
            Reply::Empty => Ok(serde_json::Value::Null),
            Reply::AlreadyApplied => Err(self.upstream("respuesta 409 inesperada")),
        }
    }

    /// `write` de una petición de escritura: `409` es «ya aplicado», no error.
    async fn write(&self, request: reqwest::RequestBuilder) -> Result<WriteOutcome, OpentodoError> {
        match self.send(request).await? {
            Reply::Body(value) => Ok(WriteOutcome {
                task: decode_task(&value),
                already_applied: false,
            }),
            Reply::Empty => Ok(WriteOutcome::default()),
            Reply::AlreadyApplied => Ok(WriteOutcome {
                task: None,
                already_applied: true,
            }),
        }
    }

    /// Traduce un estado de error a su [`OpentodoError`], redactando el cuerpo.
    fn error_from_status(&self, status: u16, body: &str) -> OpentodoError {
        match status {
            400 => self.invalid_input(&self.server_message(body)),
            401 | 403 => OpentodoError::Unauthorized,
            404 => OpentodoError::NotFound,
            500..=599 => self.upstream(&self.server_message(body)),
            status => self.upstream(&format!("OpenTodo respondió {status}")),
        }
    }

    /// Extrae el mensaje del servidor de un cuerpo de error, **sin** redactar:
    /// de la redacción se encargan [`Opentodo::upstream`] y
    /// [`Opentodo::invalid_input`].
    fn server_message(&self, body: &str) -> String {
        let detail = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|value| {
                ["error", "message", "detail", "title"]
                    .iter()
                    .find_map(|key| value.get(*key).and_then(|v| v.as_str()))
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
}

/// Inserta un campo opcional de texto en el cuerpo de la petición.
fn insert_optional(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        payload.insert(key.to_string(), serde_json::json!(value));
    }
}

/// Deserializa una tarea de la respuesta, admitiendo `{...}` o `{"task":{...}}`.
fn decode_task(value: &serde_json::Value) -> Option<Task> {
    if value.is_null() {
        return None;
    }
    let raw = value.get("task").cloned().unwrap_or_else(|| value.clone());
    serde_json::from_value(raw).ok()
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
                "no se pudo leer el ajuste de opentodo: {error}"
            );
        }
    }

    std::env::var(env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// `true` si la tarea cae en la vista, sin conocer el usuario autenticado: en
/// `waiting` la condición de `assignee_id` no se evalúa (no se puede afirmar que
/// el asignado no sea el usuario).
///
/// Atajo **solo para tests**: el código de producción debe usar
/// [`matches_view_as`] con el id real del usuario, porque si no la vista
/// `waiting` pierde su condición de asignación.
#[cfg(test)]
pub fn matches_view(task: &Task, view: View, today: NaiveDate) -> bool {
    matches_view_as(task, view, today, None)
}

/// `true` si la tarea cae en la vista. `me_id` es el id del usuario
/// autenticado (`GET /api/v1/me`), necesario para la condición de `waiting`.
pub fn matches_view_as(task: &Task, view: View, today: NaiveDate, me_id: Option<&str>) -> bool {
    if !task.is_open() {
        return false;
    }

    let due_on_or_before = |due: &Option<Due>| {
        due.as_ref()
            .and_then(Due::as_date)
            .is_some_and(|date| date <= today)
    };
    let due_after = |due: &Option<Due>| {
        due.as_ref()
            .and_then(Due::as_date)
            .is_some_and(|date| date > today)
    };

    match view {
        View::All => true,
        View::Inbox => task.project_id.is_none(),
        View::Today => due_on_or_before(&task.due) || due_on_or_before(&task.deadline),
        View::Upcoming => due_after(&task.due) || (task.due.is_none() && due_after(&task.deadline)),
        View::Anytime => task.due.is_none() && task.deadline.is_none() && task.start.is_none(),
        View::Someday => task.start.as_deref() == Some("someday"),
        View::Doing => task.has_label("doing"),
        View::Waiting => {
            task.has_label("waiting")
                || !task.blocked_by.is_empty()
                || matches!(
                    (me_id, task.assignee_id.as_deref()),
                    (Some(me), Some(assignee)) if assignee != me
                )
        }
    }
}

/// Agrega el listado: totales por vista y por proyecto de las tareas **abiertas**.
pub fn summarize(
    tasks: &[Task],
    today: NaiveDate,
    me_id: Option<&str>,
    projects: &[Project],
) -> Summary {
    let mut by_view = BTreeMap::new();
    for view in View::SUMMARY_VIEWS {
        let count = tasks
            .iter()
            .filter(|task| matches_view_as(task, view, today, me_id))
            .count();
        by_view.insert(view.as_str().to_string(), count);
    }

    let names: HashMap<&str, &str> = projects
        .iter()
        .map(|project| (project.id.as_str(), project.name.as_str()))
        .collect();
    let mut by_project = BTreeMap::new();
    for task in tasks.iter().filter(|task| task.is_open()) {
        let key = match task.project_id.as_deref() {
            None => "(sin proyecto)".to_string(),
            Some(id) => names
                .get(id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| id.to_string()),
        };
        *by_project.entry(key).or_insert(0) += 1;
    }

    Summary {
        by_view,
        by_project,
    }
}

/// Rellena `label_names` resolviendo los ids de `labels` en `map` (id → nombre).
pub fn apply_label_names(tasks: &mut [Task], map: &HashMap<String, String>) {
    for task in tasks.iter_mut() {
        task.label_names = task
            .labels
            .iter()
            .filter_map(|id| map.get(id).cloned())
            .collect();
    }
}

/// Interpreta un `due` ISO (`YYYY-MM-DD` o `YYYY-MM-DDTHH:MM`). Devuelve
/// `None` si no es ISO, que es la señal de que hay que delegar en quick-add.
pub fn parse_iso_due(text: &str) -> Option<Due> {
    let text = text.trim();

    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(Due {
            date: date.format("%Y-%m-%d").to_string(),
            time: None,
            tz: None,
        });
    }

    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M") {
        return Some(Due {
            date: datetime.date().format("%Y-%m-%d").to_string(),
            time: Some(datetime.time().format("%H:%M").to_string()),
            tz: None,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use wiremock::matchers::{body_json, header, method, path};
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

    fn client_for(server: &MockServer) -> Opentodo {
        Opentodo::new(OpentodoConfig {
            base_url: server.uri(),
            token: "test-token".to_string(),
        })
        .expect("cliente de test")
    }

    fn date(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").expect("fecha ISO")
    }

    /// Tarea mínima para los tests de vistas.
    fn task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            title: format!("tarea {id}"),
            ..Task::default()
        }
    }

    /// Forma real de una tarea (v0.16.0), con sus campos de más.
    fn task_json(id: &str, title: &str, priority: u8) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": title,
            "description": "",
            "project_id": null,
            "parent_id": null,
            "priority": priority,
            "due": { "date": "2026-10-27" },
            "labels": [],
            "fields": {},
            "position": 1793082844171_i64,
            "created_at": 1793082844171_i64,
            "completed_at": null,
            "completed_by": null,
            "deleted_at": null,
            "estimate_minutes": null,
            "origin": null
        })
    }

    fn labels_json(labels: &[(&str, &str)]) -> serde_json::Value {
        serde_json::json!({
            "labels": labels
                .iter()
                .map(|(id, name)| serde_json::json!({ "id": id, "name": name, "workspace_id": "w1" }))
                .collect::<Vec<_>>()
        })
    }

    // -----------------------------------------------------------------------
    // Configuración: settings → entorno → valor por defecto
    // -----------------------------------------------------------------------

    /// Scenario: La configuración sale de settings y cae al entorno.
    #[tokio::test]
    #[serial]
    async fn resolve_config_setting_wins_over_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "opentodo_base_url", "https://from-settings.test")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "opentodo_token", "setting-token")
            .await
            .unwrap();
        set_env("OPENTODO_BASE_URL", Some("https://from-env.test"));
        set_env("OPENTODO_TOKEN", Some("env-token"));

        let config = Opentodo::from_settings(&pool).await.unwrap();

        set_env("OPENTODO_BASE_URL", None);
        set_env("OPENTODO_TOKEN", None);

        assert_eq!(config.base_url(), "https://from-settings.test");
    }

    /// Scenario: sin ajuste (vacío o en blanco) se usa el entorno.
    #[tokio::test]
    #[serial]
    async fn resolve_config_falls_back_to_env() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "opentodo_base_url", "   ")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "opentodo_token", "")
            .await
            .unwrap();
        set_env("OPENTODO_BASE_URL", Some("https://from-env.test"));
        set_env("OPENTODO_TOKEN", Some("env-token"));

        let config = Opentodo::from_settings(&pool).await.unwrap();
        let token = Opentodo::resolve_token(&pool).await;

        set_env("OPENTODO_BASE_URL", None);
        set_env("OPENTODO_TOKEN", None);

        assert_eq!(config.base_url(), "https://from-env.test");
        assert_eq!(token.as_deref(), Some("env-token"));
    }

    /// Scenario: sin nada configurado la URL cae a [`DEFAULT_BASE_URL`].
    #[tokio::test]
    #[serial]
    async fn resolve_config_defaults_base_url() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "opentodo_base_url", "")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "opentodo_token", "only-token")
            .await
            .unwrap();
        set_env("OPENTODO_BASE_URL", None);
        set_env("OPENTODO_TOKEN", None);

        let base_url = Opentodo::resolve_base_url(&pool).await;
        let config = Opentodo::from_settings(&pool).await.unwrap();

        assert_eq!(base_url, DEFAULT_BASE_URL);
        assert_eq!(config.base_url(), DEFAULT_BASE_URL);
        assert_eq!(config.config.token, "only-token");
    }

    /// Scenario: sin token se distingue de un token inválido.
    #[tokio::test]
    #[serial]
    async fn missing_token_is_not_configured() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "opentodo_token", "")
            .await
            .unwrap();
        set_env("OPENTODO_TOKEN", None);

        let err = Opentodo::from_settings(&pool).await.unwrap_err();
        let configured = Opentodo::is_configured(&pool).await;

        assert!(matches!(err, OpentodoError::NotConfigured), "err: {err:?}");
        assert!(!configured, "sin token no está configurado");
        let message = err.to_string();
        assert!(
            message.contains("Ajustes → Integraciones → OpenTodo")
                && message.contains("OPENTODO_TOKEN"),
            "el mensaje debe decir dónde configurarlo: {message}"
        );
    }

    /// Scenario: con token, `is_configured` y el cliente se construyen.
    #[tokio::test]
    #[serial]
    async fn is_configured_with_token() {
        let pool = setup_pool().await;
        SettingsRepo::set(&pool, "opentodo_token", "tok")
            .await
            .unwrap();
        set_env("OPENTODO_TOKEN", None);

        assert!(Opentodo::is_configured(&pool).await);
    }

    /// Scenario: un error de lectura de `settings` se trata como ausencia.
    #[tokio::test]
    #[serial]
    async fn resolve_config_treats_settings_error_as_absent() {
        let pool = setup_pool().await;
        sqlx::query("DROP TABLE settings")
            .execute(&pool)
            .await
            .expect("drop settings para forzar el error de lectura");
        set_env("OPENTODO_TOKEN", Some("env-token"));
        set_env("OPENTODO_BASE_URL", Some("https://from-env.test"));

        let config = Opentodo::from_settings(&pool).await.unwrap();

        set_env("OPENTODO_TOKEN", None);
        set_env("OPENTODO_BASE_URL", None);

        assert_eq!(config.base_url(), "https://from-env.test");
        assert_eq!(config.config.token, "env-token");
    }

    // -----------------------------------------------------------------------
    // El token nunca aparece en un Debug ni en un mensaje de error
    // -----------------------------------------------------------------------

    /// Scenario: el `Debug` de la configuración no contiene el token.
    #[test]
    fn config_debug_redacts_the_token() {
        let config = OpentodoConfig {
            base_url: "https://example.test".to_string(),
            token: "SECRET".to_string(),
        };

        let debug = format!("{config:?}");
        assert!(
            !debug.contains("SECRET"),
            "el Debug filtra el token: {debug}"
        );
        assert!(debug.contains("***"), "el Debug debe redactar: {debug}");
    }

    /// Scenario: ningún `Display` de error contiene el token, ni siquiera cuando
    /// el servidor lo devuelve en el cuerpo del `400`.
    #[tokio::test]
    async fn error_display_never_contains_the_token() {
        const TOKEN: &str = "SECRET";

        for (status, kind) in [
            (400u16, "InvalidInput"),
            (401, "Unauthorized"),
            (403, "Unauthorized"),
            (404, "NotFound"),
            (500, "Upstream"),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v1/me"))
                .respond_with(
                    ResponseTemplate::new(status).set_body_string(format!("fallo con {TOKEN}")),
                )
                .mount(&server)
                .await;

            let client = Opentodo::new(OpentodoConfig {
                base_url: server.uri(),
                token: TOKEN.to_string(),
            })
            .expect("cliente de test");
            let err = client.me().await.unwrap_err();

            let label = match &err {
                OpentodoError::InvalidInput(_) => "InvalidInput",
                OpentodoError::Unauthorized => "Unauthorized",
                OpentodoError::NotFound => "NotFound",
                OpentodoError::Upstream(_) => "Upstream",
                OpentodoError::NotConfigured => "NotConfigured",
            };
            assert_eq!(label, kind, "status {status}: {err:?}");
            let debug = format!("{err:?}");
            let display = err.to_string();
            assert!(!debug.contains(TOKEN), "el Debug filtra el token: {debug}");
            assert!(
                !display.contains(TOKEN),
                "el Display filtra el token: {display}"
            );
        }

        // `NotConfigured` y `Upstream` sin servidor tampoco lo filtran.
        let not_configured = OpentodoError::NotConfigured;
        assert!(!not_configured.to_string().contains(TOKEN));
    }

    // -----------------------------------------------------------------------
    // Traducción de estados y red
    // -----------------------------------------------------------------------

    /// Scenario: 401/403 → `Unauthorized`, 404 → `NotFound`, 5xx → `Upstream`.
    #[tokio::test]
    async fn error_statuses_are_distinct() {
        for (status, expected) in [
            (401u16, "Unauthorized"),
            (403, "Unauthorized"),
            (404, "NotFound"),
            (500, "Upstream"),
            (503, "Upstream"),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v1/tasks"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;

            let err = client_for(&server).list_tasks().await.unwrap_err();
            let label = match &err {
                OpentodoError::Unauthorized => "Unauthorized",
                OpentodoError::NotFound => "NotFound",
                OpentodoError::Upstream(_) => "Upstream",
                other => panic!("inesperado para {status}: {other:?}"),
            };
            assert_eq!(label, expected, "status {status}");
        }
    }

    /// Scenario: un `400` es `InvalidInput` y lleva el mensaje del servidor.
    #[tokio::test]
    async fn error_400_carries_the_server_message() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "error": "prioridad inválida" })),
            )
            .mount(&server)
            .await;

        let err = client_for(&server).list_tasks().await.unwrap_err();

        match err {
            OpentodoError::InvalidInput(detail) => {
                assert!(
                    detail.contains("prioridad inválida"),
                    "el detalle pierde el mensaje del servidor: {detail}"
                );
            }
            other => panic!("se esperaba InvalidInput: {other:?}"),
        }
    }

    /// Un fallo de transporte es `Upstream`.
    #[tokio::test]
    async fn network_failure_is_upstream() {
        let client = Opentodo::new(OpentodoConfig {
            base_url: "http://127.0.0.1:1".to_string(),
            token: "t".to_string(),
        })
        .expect("cliente de test");

        let err = client.me().await.unwrap_err();

        assert!(matches!(err, OpentodoError::Upstream(_)), "err: {err:?}");
    }

    /// Sin token (vacío o en blanco) no se toca la red.
    #[tokio::test]
    async fn empty_token_sends_no_request() {
        let server = MockServer::start().await;
        let client = Opentodo::new(OpentodoConfig {
            base_url: server.uri(),
            token: "   ".to_string(),
        })
        .expect("cliente de test");

        let err = client.me().await.unwrap_err();

        assert!(matches!(err, OpentodoError::NotConfigured), "err: {err:?}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "sin token no debe enviar ninguna petición"
        );
    }

    // -----------------------------------------------------------------------
    // Cabeceras
    // -----------------------------------------------------------------------

    /// El token viaja en `Authorization: Bearer` y el User-Agent es propio.
    #[tokio::test]
    async fn token_travels_in_authorization_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/me"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "u1" })),
            )
            .mount(&server)
            .await;

        let value = client_for(&server).me().await.unwrap();

        assert_eq!(value["id"], "u1");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests[0]
                .headers
                .get("user-agent")
                .expect("debe llevar User-Agent")
                .to_str()
                .unwrap(),
            "valet/1.0"
        );
    }

    // -----------------------------------------------------------------------
    // Deserialización tolerante
    // -----------------------------------------------------------------------

    /// Scenario: una tarea con campos ausentes deserializa sin error.
    #[tokio::test]
    async fn task_with_missing_fields_deserializes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{ "id": "t1", "title": "Ejemplo", "priority": 4 }]
            })))
            .mount(&server)
            .await;

        let tasks = client_for(&server).list_tasks().await.unwrap();

        assert_eq!(tasks.len(), 1);
        let task = &tasks[0];
        assert_eq!(task.id, "t1");
        assert_eq!(task.title, "Ejemplo");
        assert_eq!(task.priority, Some(4));
        assert!(task.due.is_none());
        assert!(task.deadline.is_none());
        assert!(task.start.is_none());
        assert!(task.completed_at.is_none());
        assert!(task.blocked_by.is_empty());
        assert!(task.assignee_id.is_none());
    }

    /// Scenario: la forma real de una tarea (con campos de más) deserializa.
    #[tokio::test]
    async fn task_with_unknown_fields_deserializes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [task_json("t1", "Ejemplo", 2)]
            })))
            .mount(&server)
            .await;

        let tasks = client_for(&server).list_tasks().await.unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].priority_label(), Some("high"));
        assert_eq!(
            tasks[0].due.as_ref().map(|due| due.date.clone()),
            Some("2026-10-27".to_string())
        );
    }

    /// Scenario: la prioridad se envía numérica (1..4) y se lee de vuelta como
    /// texto.
    #[tokio::test]
    async fn priority_is_numeric_on_the_wire_and_text_back() {
        for (priority, code, label) in [
            (Priority::Critical, 1u8, "critical"),
            (Priority::High, 2, "high"),
            (Priority::Medium, 3, "medium"),
            (Priority::Low, 4, "low"),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/api/v1/tasks"))
                .and(body_json(serde_json::json!({
                    "title": "Comprar pan", "priority": code
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(task_json(
                    "t1",
                    "Comprar pan",
                    code,
                )))
                .mount(&server)
                .await;

            let outcome = client_for(&server)
                .create_task(&NewTask {
                    title: "Comprar pan".to_string(),
                    priority: Some(priority),
                    ..NewTask::default()
                })
                .await
                .unwrap();

            let task = outcome.task.expect("la respuesta trae la tarea");
            assert_eq!(task.priority, Some(code));
            assert_eq!(task.priority_label(), Some(label));
        }
    }

    /// Sin prioridad explícita se envía `4`, el valor por defecto del servidor.
    #[tokio::test]
    async fn create_task_defaults_priority_to_low() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/tasks"))
            .and(body_json(serde_json::json!({
                "title": "Sin prioridad", "priority": 4
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json(
                "t1",
                "Sin prioridad",
                4,
            )))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .create_task(&NewTask {
                title: "Sin prioridad".to_string(),
                ..NewTask::default()
            })
            .await
            .unwrap();

        assert!(!outcome.already_applied);
        assert_eq!(
            outcome.task.as_ref().and_then(|task| task.priority_label()),
            Some("low")
        );
    }

    // -----------------------------------------------------------------------
    // Escrituras e idempotencia
    // -----------------------------------------------------------------------

    /// Un `409` de `Idempotency-Key` es «ya aplicado»: ni error ni reintento.
    #[tokio::test]
    async fn idempotency_conflict_is_already_applied() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(409).set_body_string("idempotency key reusada"))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .update_task(
                "t1",
                &TaskPatch {
                    title: Some("Nuevo".to_string()),
                    ..TaskPatch::default()
                },
            )
            .await
            .unwrap();

        assert!(outcome.already_applied, "un 409 no es un error");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "un 409 no se reintenta");
    }

    /// `complete_task` manda `completed_at` en milisegundos.
    #[tokio::test]
    async fn complete_task_sets_completed_at() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "X", 4)))
            .mount(&server)
            .await;

        client_for(&server).complete_task("t1").await.unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(requests[0].body.as_slice()).expect("JSON");
        let completed_at = body["completed_at"]
            .as_i64()
            .expect("completed_at debe ser numérico");
        assert!(
            completed_at > 1_700_000_000_000,
            "debe ir en milisegundos: {completed_at}"
        );
    }

    /// `delete_task` usa la purga definitiva cuando se pide.
    #[tokio::test]
    async fn delete_task_uses_purge_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/tasks/t1/purge"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let outcome = client_for(&server).delete_task("t1", true).await.unwrap();

        assert!(!outcome.already_applied);
        assert!(outcome.task.is_none(), "un 204 no trae cuerpo");
    }

    /// `delete_task` sin purga borra con lápida.
    #[tokio::test]
    async fn delete_task_soft_deletes() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client_for(&server).delete_task("t1", false).await.unwrap();
    }

    // -----------------------------------------------------------------------
    // Fechas: ISO sin red, natural por quick-add
    // -----------------------------------------------------------------------

    /// Una fecha ISO no pasa por quick-add.
    #[tokio::test]
    async fn iso_due_skips_quick_add() {
        let server = MockServer::start().await;
        let client = client_for(&server);

        let due = client.parse_natural_due("2026-10-30").await.unwrap();
        let with_time = client.parse_natural_due("2026-10-30T10:00").await.unwrap();

        assert_eq!(
            due,
            Some(Due {
                date: "2026-10-30".to_string(),
                time: None,
                tz: None
            })
        );
        assert_eq!(
            with_time.and_then(|due| due.time),
            Some("10:00".to_string())
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "una fecha ISO no debe llamar a quick-add"
        );
    }

    /// Una expresión natural se delega en `POST /api/v1/quick-add/parse`.
    #[tokio::test]
    async fn natural_due_delegates_to_quick_add() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/quick-add/parse"))
            .and(body_json(
                serde_json::json!({ "text": "el viernes a las 10" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "due": { "date": "2026-10-30", "time": "10:00" }
            })))
            .mount(&server)
            .await;

        let due = client_for(&server)
            .parse_natural_due("el viernes a las 10")
            .await
            .unwrap()
            .expect("la expresión resuelve una fecha");

        assert_eq!(due.date, "2026-10-30");
        assert_eq!(due.time.as_deref(), Some("10:00"));
    }

    /// `parse_iso_due` reconoce los dos formatos ISO y rechaza el lenguaje
    /// natural.
    #[test]
    fn parse_iso_due_recognizes_only_iso() {
        assert_eq!(
            parse_iso_due("2026-10-30"),
            Some(Due {
                date: "2026-10-30".to_string(),
                time: None,
                tz: None
            })
        );
        assert_eq!(
            parse_iso_due("2026-10-30T10:00"),
            Some(Due {
                date: "2026-10-30".to_string(),
                time: Some("10:00".to_string()),
                tz: None
            })
        );
        assert!(parse_iso_due("el viernes a las 10").is_none());
        assert!(parse_iso_due("2026-13-45").is_none());
    }

    // -----------------------------------------------------------------------
    // Vistas — nada de esto lo filtra el servidor
    // -----------------------------------------------------------------------

    const TODAY: &str = "2026-10-27";

    /// Scenario: una tarea vencida aparece en hoy.
    #[test]
    fn view_today_includes_overdue() {
        let overdue = Task {
            due: Some(Due {
                date: "2026-10-26".to_string(),
                ..Due::default()
            }),
            ..task("t1")
        };
        let future = Task {
            due: Some(Due {
                date: "2026-11-10".to_string(),
                ..Due::default()
            }),
            ..task("t2")
        };
        let deadline_today = Task {
            deadline: Some(Due {
                date: TODAY.to_string(),
                ..Due::default()
            }),
            ..task("t3")
        };

        assert!(matches_view(&overdue, View::Today, date(TODAY)));
        assert!(!matches_view(&future, View::Today, date(TODAY)));
        assert!(matches_view(&deadline_today, View::Today, date(TODAY)));
    }

    /// Scenario: las completadas no aparecen en ninguna vista, tampoco en `all`.
    #[test]
    fn view_completed_is_in_no_view() {
        let completed = Task {
            completed_at: Some(1_793_082_844_171),
            due: Some(Due {
                date: "2026-10-26".to_string(),
                ..Due::default()
            }),
            start: Some("someday".to_string()),
            labels: vec!["doing".to_string()],
            blocked_by: vec!["t0".to_string()],
            ..task("t1")
        };

        for view in View::ALL {
            assert!(
                !matches_view(&completed, view, date(TODAY)),
                "una tarea completada no puede salir en {view:?}"
            );
        }
    }

    /// Scenario: la vista en espera suma etiqueta y bloqueo.
    #[test]
    fn view_waiting_adds_label_and_blocked_by() {
        let blocked = Task {
            blocked_by: vec!["t0".to_string()],
            ..task("t1")
        };
        let labelled = Task {
            labels: vec!["waiting".to_string()],
            ..task("t2")
        };
        let assigned_to_me = Task {
            assignee_id: Some("me".to_string()),
            ..task("t3")
        };
        let assigned_to_other = Task {
            assignee_id: Some("otro".to_string()),
            ..task("t4")
        };
        let plain = task("t5");

        assert!(matches_view_as(
            &blocked,
            View::Waiting,
            date(TODAY),
            Some("me")
        ));
        assert!(matches_view_as(
            &labelled,
            View::Waiting,
            date(TODAY),
            Some("me")
        ));
        assert!(!matches_view_as(
            &assigned_to_me,
            View::Waiting,
            date(TODAY),
            Some("me")
        ));
        assert!(matches_view_as(
            &assigned_to_other,
            View::Waiting,
            date(TODAY),
            Some("me")
        ));
        assert!(!matches_view_as(
            &plain,
            View::Waiting,
            date(TODAY),
            Some("me")
        ));
    }

    /// Scenario: una tarea puede estar en varias vistas.
    #[test]
    fn view_task_can_be_in_several_views() {
        let multipurpose = Task {
            due: Some(Due {
                date: TODAY.to_string(),
                ..Due::default()
            }),
            labels: vec!["doing".to_string()],
            ..task("t1")
        };

        assert!(matches_view(&multipurpose, View::Inbox, date(TODAY)));
        assert!(matches_view(&multipurpose, View::Today, date(TODAY)));
        assert!(matches_view(&multipurpose, View::Doing, date(TODAY)));
        assert!(matches_view(&multipurpose, View::All, date(TODAY)));
    }

    /// Tabla de las ocho vistas con vectores fijos.
    #[test]
    fn view_predicates_table() {
        let today = date(TODAY);

        // Bandeja: sin proyecto.
        let inbox = task("inbox");
        assert!(matches_view(&inbox, View::Inbox, today));
        assert!(matches_view(&inbox, View::Anytime, today));

        // Con proyecto: fuera de la bandeja.
        let projected = Task {
            project_id: Some("p1".to_string()),
            ..task("projected")
        };
        assert!(!matches_view(&projected, View::Inbox, today));

        // Próximo: due por delante; y sin due pero con deadline por delante.
        let future = Task {
            due: Some(Due {
                date: "2026-11-10".to_string(),
                ..Due::default()
            }),
            ..task("future")
        };
        assert!(matches_view(&future, View::Upcoming, today));
        assert!(!matches_view(&future, View::Anytime, today));

        let deadline_only = Task {
            deadline: Some(Due {
                date: "2026-11-10".to_string(),
                ..Due::default()
            }),
            ..task("deadline-only")
        };
        assert!(matches_view(&deadline_only, View::Upcoming, today));
        assert!(!matches_view(&deadline_only, View::Anytime, today));

        // Algún día: `start == "someday"`.
        let someday = Task {
            start: Some("someday".to_string()),
            ..task("someday")
        };
        assert!(matches_view(&someday, View::Someday, today));
        assert!(!matches_view(&someday, View::Anytime, today));

        // Con `start` normal tampoco es «en cualquier momento».
        let started = Task {
            start: Some("2026-10-28".to_string()),
            ..task("started")
        };
        assert!(!matches_view(&started, View::Anytime, today));
        assert!(!matches_view(&started, View::Someday, today));

        // `doing` por etiqueta.
        let doing = Task {
            labels: vec!["doing".to_string()],
            ..task("doing")
        };
        assert!(matches_view(&doing, View::Doing, today));
        assert!(!matches_view(&doing, View::Waiting, today));

        // Y todos los abiertos están en `all`.
        for tarea in [&inbox, &projected, &future, &someday, &doing] {
            assert!(matches_view(tarea, View::All, today), "{:?}", tarea.id);
        }
    }

    /// Las etiquetas por id se resuelven a nombre antes de evaluar las vistas.
    #[test]
    fn apply_label_names_resolves_ids() {
        let mut tasks = vec![Task {
            labels: vec!["lbl-doing".to_string()],
            ..task("t1")
        }];
        let map = HashMap::from([("lbl-doing".to_string(), "doing".to_string())]);

        apply_label_names(&mut tasks, &map);

        assert_eq!(tasks[0].label_names, vec!["doing".to_string()]);
        assert!(tasks[0].has_label("doing"));
        assert!(matches_view(&tasks[0], View::Doing, date(TODAY)));
    }

    /// El resumen agrega por vista y por proyecto.
    #[test]
    fn summary_aggregates_by_view_and_project() {
        let projects = vec![Project {
            id: "p1".to_string(),
            name: "Casa".to_string(),
        }];
        let tasks = vec![
            Task {
                project_id: Some("p1".to_string()),
                ..task("t1")
            },
            task("t2"),
            Task {
                due: Some(Due {
                    date: TODAY.to_string(),
                    ..Due::default()
                }),
                ..task("t3")
            },
            Task {
                completed_at: Some(1),
                ..task("t4")
            },
        ];

        let summary = summarize(&tasks, date(TODAY), None, &projects);

        assert_eq!(summary.by_view.get("inbox"), Some(&2));
        assert_eq!(summary.by_view.get("today"), Some(&1));
        // `t1` está en un proyecto, pero «en cualquier momento» solo mira fechas.
        assert_eq!(summary.by_view.get("anytime"), Some(&2));
        assert_eq!(summary.by_view.len(), 7, "las siete vistas del resumen");
        assert_eq!(summary.by_project.get("Casa"), Some(&1));
        assert_eq!(summary.by_project.get("(sin proyecto)"), Some(&2));
        assert_eq!(
            summary.by_project.get("(sin proyecto)"),
            Some(&2),
            "las completadas no cuentan"
        );
    }

    // -----------------------------------------------------------------------
    // set_label — el `PATCH` reemplaza el array entero
    // -----------------------------------------------------------------------

    /// Preparado común: la tarea `t1` con las etiquetas dadas y el catálogo de
    /// etiquetas de la cuenta.
    async fn mount_set_label(server: &MockServer, task_labels: &[&str], catalog: &[(&str, &str)]) {
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{
                    "id": "t1", "title": "Tarea", "priority": 4, "labels": task_labels
                }]
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(labels_json(catalog)))
            .mount(server)
            .await;
    }

    /// Scenario: poner `doing` no borra las demás etiquetas.
    #[tokio::test]
    async fn set_label_keeps_foreign_labels() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["lbl-shared", "lbl-casa"],
            &[
                ("lbl-shared", "shared"),
                ("lbl-casa", "casa"),
                ("lbl-doing", "doing"),
                ("lbl-waiting", "waiting"),
            ],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["lbl-shared", "lbl-casa", "lbl-doing"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        assert_eq!(outcome.created_label, None, "la etiqueta ya existía");
        assert!(outcome.labels.contains(&"lbl-casa".to_string()));
        assert!(outcome.labels.contains(&"lbl-doing".to_string()));
    }

    /// Scenario: los grupos son excluyentes.
    #[tokio::test]
    async fn set_label_groups_are_exclusive() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["lbl-doing", "lbl-personal"],
            &[
                ("lbl-doing", "doing"),
                ("lbl-waiting", "waiting"),
                ("lbl-personal", "personal"),
            ],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["lbl-personal", "lbl-waiting"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("waiting"))
            .await
            .unwrap();

        assert!(!outcome.labels.contains(&"lbl-doing".to_string()));
        assert!(outcome.labels.contains(&"lbl-personal".to_string()));
    }

    /// Scenario: quitar la etiqueta de un grupo.
    #[tokio::test]
    async fn set_label_none_removes_only_the_group() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["lbl-doing", "lbl-personal"],
            &[("lbl-doing", "doing"), ("lbl-personal", "personal")],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["lbl-personal"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, None)
            .await
            .unwrap();

        assert_eq!(outcome.labels, vec!["lbl-personal".to_string()]);
        assert_eq!(outcome.created_label, None);
    }

    /// Scenario: una etiqueta ausente se crea una sola vez.
    #[tokio::test]
    async fn set_label_creates_missing_label_once() {
        let server = MockServer::start().await;
        mount_set_label(&server, &["lbl-shared"], &[("lbl-shared", "shared")]).await;
        Mock::given(method("POST"))
            .and(path("/api/v1/labels"))
            .and(body_json(serde_json::json!({ "name": "doing" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "lbl-new", "name": "doing"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["lbl-shared", "lbl-new"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        assert_eq!(outcome.created_label.as_deref(), Some("doing"));
        assert!(outcome.labels.contains(&"lbl-new".to_string()));
    }

    /// Scenario: una etiqueta homónima del usuario se adopta.
    #[tokio::test]
    async fn set_label_adopts_homonymous_label() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["lbl-doing"],
            &[("lbl-doing", "doing"), ("lbl-shared-user", "shared")],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["lbl-doing", "lbl-shared-user"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::Scope, Some("shared"))
            .await
            .unwrap();

        assert_eq!(outcome.created_label, None);
        let posts = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|request| request.method.as_str() == "POST")
            .count();
        assert_eq!(posts, 0, "no debe crear una etiqueta duplicada");
    }

    /// Un valor fuera del grupo es `InvalidInput` y no toca la red.
    #[tokio::test]
    async fn set_label_rejects_foreign_value() {
        let server = MockServer::start().await;
        let err = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("personal"))
            .await
            .unwrap_err();

        assert!(
            matches!(err, OpentodoError::InvalidInput(_)),
            "err: {err:?}"
        );
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "un valor inválido no debe salir a la red"
        );
    }

    // -----------------------------------------------------------------------
    // Utilidades puras
    // -----------------------------------------------------------------------

    #[test]
    fn priority_codes_round_trip() {
        for (priority, code, name) in [
            (Priority::Critical, 1u8, "critical"),
            (Priority::High, 2, "high"),
            (Priority::Medium, 3, "medium"),
            (Priority::Low, 4, "low"),
        ] {
            assert_eq!(priority.code(), code);
            assert_eq!(Priority::from_code(code), Some(priority));
            assert_eq!(Priority::parse(name), Some(priority));
            assert_eq!(priority.as_str(), name);
        }
        assert_eq!(Priority::from_code(0), None);
        assert_eq!(Priority::from_code(5), None);
        assert_eq!(Priority::parse("urgente"), None);
    }

    #[test]
    fn groups_and_views_parse() {
        assert_eq!(LabelGroup::parse("state"), Some(LabelGroup::State));
        assert_eq!(LabelGroup::parse("scope"), Some(LabelGroup::Scope));
        assert_eq!(LabelGroup::parse("otro"), None);
        assert_eq!(LabelGroup::State.labels(), STATE_LABELS);
        assert_eq!(LabelGroup::Scope.labels(), SCOPE_LABELS);
        assert!(LabelGroup::State.accepts("doing"));
        assert!(!LabelGroup::State.accepts("personal"));

        for view in View::ALL {
            assert_eq!(View::parse(view.as_str()), Some(view));
        }
        assert_eq!(View::parse("next"), None);
    }

    /// `GET /api/v1/tasks` devuelve `{"tasks":[…]}` y los proyectos/labels sus
    /// propias claves.
    #[tokio::test]
    async fn list_wrappers_are_unwrapped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "projects": [{ "id": "p1", "name": "Casa" }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(labels_json(&[("l1", "casa")])))
            .mount(&server)
            .await;

        let client = client_for(&server);
        let projects = client.list_projects().await.unwrap();
        let labels = client.list_labels().await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Casa");
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].name, "casa");
    }

    /// Aserción de compilación: los futuros públicos deben ser `Send`.
    #[test]
    fn public_futures_are_send() {
        fn assert_send<T: Send>(_: T) {}
        let client = Opentodo::new(OpentodoConfig {
            base_url: "http://localhost:1".to_string(),
            token: "k".to_string(),
        })
        .expect("cliente de test");
        assert_send(client.me());
        assert_send(client.list_tasks());
        assert_send(client.list_projects());
        assert_send(client.list_labels());
        assert_send(client.create_task(&NewTask::default()));
        assert_send(client.update_task("t", &TaskPatch::default()));
        assert_send(client.complete_task("t"));
        assert_send(client.delete_task("t", true));
        assert_send(client.parse_natural_due("x"));
        assert_send(client.set_label("t", LabelGroup::State, Some("doing")));
    }

    // -----------------------------------------------------------------------
    // Auditoría de opentodo-integration — H1, H2 y H3
    // -----------------------------------------------------------------------

    /// H4 — la construcción devuelve `Result` (no puede entrar en pánico) y el
    /// cliente resultante conserva el timeout de 10 s del contrato.
    #[test]
    fn client_builds_without_panicking_and_keeps_the_timeout() {
        let client = Opentodo::new(OpentodoConfig {
            base_url: "https://todo.test/".to_string(),
            token: "SECRET".to_string(),
        })
        .expect("construir el cliente no debe entrar en pánico");

        assert_eq!(
            client.base_url(),
            "https://todo.test",
            "se recorta la barra"
        );
        assert_eq!(
            REQUEST_TIMEOUT_SECS, 10,
            "el timeout del contrato son 10 segundos"
        );
    }

    /// H1 — con `labels` en **nombres**, `state: null` tiene que quitar la del
    /// grupo: hoy se reenvía el array intacto y se informa de un éxito que no
    /// ocurrió.
    #[tokio::test]
    async fn set_label_none_removes_a_group_name_from_the_labels() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["doing", "personal"],
            &[("lbl-doing", "doing"), ("lbl-personal", "personal")],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({ "labels": ["personal"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, None)
            .await
            .unwrap();

        assert_eq!(
            outcome.labels,
            vec!["personal".to_string()],
            "el nombre del grupo debe salir del array"
        );
    }

    /// H1 — poner un valor no duplica la etiqueta del grupo cuando ya estaba
    /// como **nombre**.
    #[tokio::test]
    async fn set_label_does_not_duplicate_a_group_name_already_present() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["doing", "shared"],
            &[("lbl-doing", "doing"), ("lbl-shared", "shared")],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(
                serde_json::json!({ "labels": ["shared", "lbl-doing"] }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        assert_eq!(
            outcome.labels,
            vec!["shared".to_string(), "lbl-doing".to_string()],
            "el nombre viejo sale y la del grupo entra una sola vez"
        );
    }

    /// H1 — el caso del hallazgo: con `["shared", "casa"]` (nombres), poner
    /// `state: doing` conserva las ajenas y añade la del grupo sin duplicarla.
    #[tokio::test]
    async fn set_label_with_names_keeps_the_foreign_labels() {
        let server = MockServer::start().await;
        mount_set_label(
            &server,
            &["shared", "casa"],
            &[
                ("lbl-shared", "shared"),
                ("lbl-casa", "casa"),
                ("lbl-doing", "doing"),
            ],
        )
        .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .and(body_json(serde_json::json!({
                "labels": ["shared", "casa", "lbl-doing"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        assert_eq!(
            outcome
                .labels
                .iter()
                .filter(|label| *label == "doing" || *label == "lbl-doing")
                .count(),
            1,
            "`doing` no puede aparecer dos veces: {:?}",
            outcome.labels
        );
        assert!(outcome.labels.contains(&"casa".to_string()));
    }

    /// H2 — las escrituras llevan una `Idempotency-Key` nueva por petición, que
    /// es lo que da sentido al «409 = ya aplicado».
    ///
    /// Cubre las cinco escrituras del cliente: `POST /api/v1/tasks`,
    /// `PATCH /api/v1/tasks/{id}`, `DELETE /api/v1/tasks/{id}/purge` y las dos
    /// que añade `set_label` —su propio `PATCH /api/v1/tasks/{id}` y el
    /// `POST /api/v1/labels`—. El catálogo se monta **vacío** para que
    /// `set_label` tenga que crear la etiqueta y, de paso, quede ejercitada la
    /// creación.
    #[tokio::test]
    async fn writes_carry_a_fresh_idempotency_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json(
                "t1",
                "Comprar pan",
                4,
            )))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "X", 4)))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/tasks/t1/purge"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        // Las dos lecturas de `set_label` (`task` y catálogo vacío) y la
        // etiqueta que creará antes del `PATCH`.
        mount_set_label(&server, &[], &[]).await;
        Mock::given(method("POST"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "label": { "id": "lbl-doing", "name": "doing" }
            })))
            .mount(&server)
            .await;

        let client = client_for(&server);
        client
            .create_task(&NewTask {
                title: "Comprar pan".to_string(),
                ..NewTask::default()
            })
            .await
            .unwrap();
        client
            .update_task("t1", &TaskPatch::default())
            .await
            .unwrap();
        client.delete_task("t1", true).await.unwrap();
        client
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        // Solo cuentan las escrituras: las dos lecturas de `set_label` no
        // llevan (ni deben llevar) la cabecera.
        let keys: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| matches!(request.method.as_str(), "POST" | "PATCH" | "DELETE"))
            .map(|request| {
                request
                    .headers
                    .get("idempotency-key")
                    .expect("toda escritura debe llevar `Idempotency-Key`")
                    .to_str()
                    .expect("cabecera ASCII")
                    .to_string()
            })
            .collect();

        assert_eq!(keys.len(), 5, "cinco escrituras, cinco claves");
        for key in &keys {
            uuid::Uuid::parse_str(key)
                .unwrap_or_else(|error| panic!("`{key}` no es un UUID: {error}"));
        }
        let unique: std::collections::HashSet<&String> = keys.iter().collect();
        assert_eq!(unique.len(), 5, "una clave nueva por petición: {keys:?}");
    }

    /// H2 — el `PATCH` que reemplaza el array de etiquetas también va marcado:
    /// sin la cabecera, el `409` de `set_label` no podría significar «ya
    /// aplicado».
    #[tokio::test]
    async fn set_label_patch_carries_an_idempotency_key() {
        let server = MockServer::start().await;
        mount_set_label(&server, &[], &[("lbl-doing", "doing")]).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("t1", "Tarea", 4)))
            .mount(&server)
            .await;

        client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let patched: Vec<&wiremock::Request> = requests
            .iter()
            .filter(|request| request.method.as_str() == "PATCH")
            .collect();
        assert_eq!(patched.len(), 1, "un único `PATCH`, sin reintentos");
        let key = patched[0]
            .headers
            .get("idempotency-key")
            .expect("el `PATCH` de `set_label` debe llevar `Idempotency-Key`")
            .to_str()
            .expect("cabecera ASCII");
        uuid::Uuid::parse_str(key).unwrap_or_else(|error| panic!("`{key}` no es un UUID: {error}"));
    }

    /// H2 — un `409` en el `PATCH` de `set_label` es «ya aplicado»: se devuelve
    /// el array tal y como se envió, con la clave viajando y sin reintentar.
    #[tokio::test]
    async fn set_label_conflict_marks_already_applied_without_retry() {
        let server = MockServer::start().await;
        mount_set_label(&server, &[], &[("lbl-doing", "doing")]).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(409).set_body_string("idempotency key reusada"))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .set_label("t1", LabelGroup::State, Some("doing"))
            .await
            .unwrap();

        assert!(outcome.already_applied, "un 409 no es un error");
        assert_eq!(outcome.labels, vec!["lbl-doing".to_string()]);
        assert_eq!(
            outcome.created_label, None,
            "la etiqueta ya estaba en el catálogo"
        );

        let requests = server.received_requests().await.unwrap();
        let patched: Vec<&wiremock::Request> = requests
            .iter()
            .filter(|request| request.method.as_str() == "PATCH")
            .collect();
        assert_eq!(patched.len(), 1, "un 409 no se reintenta");
        assert_eq!(requests.len(), 3, "dos lecturas y una escritura, sin más");
        let key = patched[0]
            .headers
            .get("idempotency-key")
            .expect("la petición que provoca el 409 debe llevar la clave")
            .to_str()
            .expect("cabecera ASCII");
        uuid::Uuid::parse_str(key).unwrap_or_else(|error| panic!("`{key}` no es un UUID: {error}"));
    }

    /// H2 — el `409` «ya aplicado» viaja con la clave y sigue sin reintentar.
    #[tokio::test]
    async fn idempotency_conflict_does_not_retry_and_the_key_travels() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/tasks/t1"))
            .respond_with(ResponseTemplate::new(409).set_body_string("idempotency key reusada"))
            .mount(&server)
            .await;

        let outcome = client_for(&server)
            .update_task(
                "t1",
                &TaskPatch {
                    title: Some("Nuevo".to_string()),
                    ..TaskPatch::default()
                },
            )
            .await
            .unwrap();

        assert!(outcome.already_applied, "un 409 no es un error");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "un 409 no se reintenta");
        let key = requests[0]
            .headers
            .get("idempotency-key")
            .expect("la petición que provoca el 409 debe llevar la clave")
            .to_str()
            .expect("cabecera ASCII");
        uuid::Uuid::parse_str(key).expect("la clave debe ser un UUID");
    }

    /// H3 — un error de deserialización cita el valor recibido: si ese valor es
    /// el token, no puede salir ni por `Debug` ni por `Display`.
    #[tokio::test]
    async fn deserialization_errors_do_not_leak_the_token() {
        const TOKEN: &str = "SECRET";

        // El valor mal tipado es el token en los tres listados.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{ "id": "t1", "title": "x", "priority": TOKEN }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/projects"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "projects": [TOKEN] })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "labels": [TOKEN] })),
            )
            .mount(&server)
            .await;

        // El token del cliente **es** el valor que aparece en el cuerpo: es el
        // único que la redacción puede conocer.
        let client = Opentodo::new(OpentodoConfig {
            base_url: server.uri(),
            token: TOKEN.to_string(),
        })
        .expect("cliente de test");
        let errors = [
            client.list_tasks().await.unwrap_err(),
            client.list_projects().await.unwrap_err(),
            client.list_labels().await.unwrap_err(),
        ];

        // Y en la etiqueta que se crea desde `set_label`.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/tasks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tasks": [{ "id": "t1", "title": "x", "priority": 4, "labels": [] }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "labels": [] })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!("\"{TOKEN}\"")))
            .mount(&server)
            .await;
        let created_label = Opentodo::new(OpentodoConfig {
            base_url: server.uri(),
            token: TOKEN.to_string(),
        })
        .expect("cliente de test")
        .set_label("t1", LabelGroup::State, Some("doing"))
        .await
        .unwrap_err();

        for error in errors.into_iter().chain([created_label]) {
            assert!(
                matches!(error, OpentodoError::Upstream(_)),
                "err: {error:?}"
            );
            let debug = format!("{error:?}");
            assert!(!debug.contains(TOKEN), "el Debug filtra el token: {debug}");
            let display = error.to_string();
            assert!(
                !display.contains(TOKEN),
                "el Display filtra el token: {display}"
            );
        }
    }
}
