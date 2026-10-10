//! Herramientas de consulta de Strava (skill `running`), de solo lectura.
//!
//! Leen la actividad del atleta **en vivo** desde la API de Strava y devuelven
//! un resumen útil (nunca el objeto crudo). Ver
//! `openspec/changes/strava-running/specs/tools/strava/spec.md` (R4, R5).

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::services::strava::{Strava, StravaError};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Deportes considerados «running» por defecto.
const DEFAULT_RUNNING_SPORTS: [&str; 3] = ["Run", "TrailRun", "VirtualRun"];

/// Series temporales pedidas por defecto a Strava.
const DEFAULT_STREAM_KEYS: [&str; 6] = [
    "time",
    "distance",
    "heartrate",
    "velocity_smooth",
    "cadence",
    "altitude",
];

/// Nº máximo de puntos por serie antes de aplicar submuestreo.
const STREAM_SAMPLE_LIMIT: usize = 500;

/// Nº de actividades por página por defecto cuando el modelo no lo indica.
///
/// Strava aplica 30 por defecto y su máximo es 200: pedir 100 evita que, tras
/// filtrar por deporte, la página quede casi vacía de salidas válidas.
const DEFAULT_PER_PAGE: i64 = 100;

/// Máximo de actividades por página que acepta Strava.
const MAX_PER_PAGE: i64 = 200;

/// Claves de totales que resume `strava_athlete_stats`.
const TOTAL_KEYS: [&str; 9] = [
    "recent_run_totals",
    "recent_ride_totals",
    "recent_swim_totals",
    "ytd_run_totals",
    "ytd_ride_totals",
    "ytd_swim_totals",
    "all_run_totals",
    "all_ride_totals",
    "all_swim_totals",
];

/// Traduce un [`StravaError`] a un [`ToolError`] con mensaje accionable (R5).
///
/// Nunca propaga el error crudo ni provoca un pánico: sin conexión o sin
/// credenciales el mensaje pide conectar la cuenta; el resto conserva el
/// mensaje de Strava (que ya menciona el límite de tasa cuando toca).
fn tool_error(error: StravaError) -> ToolError {
    match error {
        StravaError::NotConnected | StravaError::MissingCredentials => ToolError::ExecutionError(
            "No hay conexión con Strava: conecta tu cuenta de Strava.".to_string(),
        ),
        other => ToolError::ExecutionError(other.to_string()),
    }
}

/// Redondea a un decimal.
fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Redondea a dos decimales.
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Lee un campo numérico (acepta enteros y flotantes).
fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|v| v.as_f64())
}

/// Tipo de deporte de una actividad: `sport_type` con respaldo en `type`.
fn sport_type_of(activity: &Value) -> String {
    activity
        .get("sport_type")
        .or_else(|| activity.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Ritmo en minutos por km a partir de la distancia (m) y el tiempo (s).
///
/// Devuelve `None` cuando la distancia es cero (no hay ritmo definido).
fn pace_min_km(distance_m: f64, moving_time_sec: f64) -> Option<f64> {
    if distance_m <= 0.0 {
        None
    } else {
        Some(round2(moving_time_sec / 60.0 / (distance_m / 1000.0)))
    }
}

/// Resume una actividad de Strava en el subconjunto de campos útil para el modelo.
fn summarise_activity(activity: &Value) -> Value {
    let distance_m = number(activity, "distance").unwrap_or(0.0);
    let moving_time = number(activity, "moving_time").unwrap_or(0.0);
    json!({
        "id": activity.get("id").cloned().unwrap_or(Value::Null),
        "name": activity.get("name").cloned().unwrap_or(Value::Null),
        "sport_type": sport_type_of(activity),
        "start_date_local": activity.get("start_date_local").cloned().unwrap_or(Value::Null),
        "distance_km": round2(distance_m / 1000.0),
        "moving_time_min": round1(moving_time / 60.0),
        "pace_min_km": pace_min_km(distance_m, moving_time),
        "elevation_gain_m": round1(number(activity, "total_elevation_gain").unwrap_or(0.0)),
        "average_heartrate": activity.get("average_heartrate").cloned().unwrap_or(Value::Null),
        "max_heartrate": activity.get("max_heartrate").cloned().unwrap_or(Value::Null),
    })
}

/// Resume las vueltas (`laps`) de una actividad.
fn summarise_laps(activity: &Value) -> Vec<Value> {
    activity
        .get("laps")
        .and_then(|v| v.as_array())
        .map(|laps| {
            laps.iter()
                .enumerate()
                .map(|(position, lap)| {
                    let distance_m = number(lap, "distance").unwrap_or(0.0);
                    let moving_time = number(lap, "moving_time").unwrap_or(0.0);
                    json!({
                        "index": lap
                            .get("lap_index")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(position as i64 + 1),
                        "distance_km": round2(distance_m / 1000.0),
                        "moving_time_min": round1(moving_time / 60.0),
                        "pace_min_km": pace_min_km(distance_m, moving_time),
                        "average_heartrate": lap.get("average_heartrate").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resume los splits métricos (`splits_metric`) de una actividad.
fn summarise_splits(activity: &Value) -> Vec<Value> {
    activity
        .get("splits_metric")
        .and_then(|v| v.as_array())
        .map(|splits| {
            splits
                .iter()
                .enumerate()
                .map(|(position, split)| {
                    let distance_m = number(split, "distance").unwrap_or(0.0);
                    let moving_time = number(split, "moving_time").unwrap_or(0.0);
                    json!({
                        "split": split
                            .get("split")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(position as i64 + 1),
                        "distance_km": round2(distance_m / 1000.0),
                        "moving_time_min": round1(moving_time / 60.0),
                        "pace_min_km": pace_min_km(distance_m, moving_time),
                        "elevation_difference_m": round1(number(split, "elevation_difference").unwrap_or(0.0)),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resume el perfil del atleta.
fn summarise_athlete(profile: &Value) -> Value {
    let first = profile
        .get("firstname")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim();
    let last = profile
        .get("lastname")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim();
    json!({
        "id": profile.get("id").cloned().unwrap_or(Value::Null),
        "name": format!("{first} {last}").trim().to_string(),
        "city": profile.get("city").cloned().unwrap_or(Value::Null),
        "country": profile.get("country").cloned().unwrap_or(Value::Null),
        "followers": profile.get("follower_count").cloned().unwrap_or(Value::Null),
    })
}

/// Resume un bloque de totales de Strava.
fn summarise_total(total: &Value) -> Value {
    let distance_m = number(total, "distance").unwrap_or(0.0);
    let moving_time = number(total, "moving_time").unwrap_or(0.0);
    json!({
        "distance_km": round2(distance_m / 1000.0),
        "moving_time_min": round1(moving_time / 60.0),
        "elevation_gain_m": round1(number(total, "elevation_gain").unwrap_or(0.0)),
        "count": total.get("count").cloned().unwrap_or(Value::Null),
    })
}

/// Resume los totales de año y recientes presentes en la respuesta de stats.
fn summarise_totals(stats: &Value) -> Value {
    let mut map = serde_json::Map::new();
    for key in TOTAL_KEYS {
        if let Some(total) = stats.get(key) {
            map.insert(key.to_string(), summarise_total(total));
        }
    }
    Value::Object(map)
}

/// Submuestrea una serie si supera [`STREAM_SAMPLE_LIMIT`].
///
/// Devuelve los datos (originales o submuestreados) y el paso aplicado
/// (`1` = sin submuestreo).
fn subsample(data: &[Value]) -> (Vec<Value>, usize) {
    if data.len() <= STREAM_SAMPLE_LIMIT {
        return (data.to_vec(), 1);
    }
    let step = data.len().div_ceil(STREAM_SAMPLE_LIMIT);
    (data.iter().step_by(step).cloned().collect(), step)
}

/// Convierte un `before`/`after` a epoch en segundos.
///
/// Acepta un entero (epoch ya en segundos) o una cadena ISO 8601
/// (`2026-10-05` o `2026-10-05T00:00:00Z`). Devuelve `None` si no es válido.
fn parse_epoch_bound(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => parse_epoch_str(text),
        _ => None,
    }
}

/// Interpreta una cadena como epoch en segundos o fecha ISO 8601.
fn parse_epoch_str(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // Epoch ya en segundos, pasado como cadena numérica.
    if let Ok(epoch) = raw.parse::<i64>() {
        return Some(epoch);
    }

    // Fecha-hora RFC 3339 (`2026-10-05T00:00:00Z`, o con desfase horario).
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(datetime.timestamp());
    }

    // Fecha-hora sin zona (`2026-10-05T00:00:00`), tratada como UTC.
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S") {
        return Some(datetime.and_utc().timestamp());
    }

    // Solo fecha (`2026-10-05`), a medianoche UTC.
    chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
}

/// Extrae el `id` obligatorio de los argumentos.
fn required_id(args: &Value) -> Result<i64, ToolError> {
    args.get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ToolError::InvalidArguments("Falta el id de la actividad".to_string()))
}

// ---------------------------------------------------------------------------
// strava_recent_activities
// ---------------------------------------------------------------------------

pub struct StravaRecentActivitiesTool {
    db: SqlitePool,
    strava: Strava,
}

impl StravaRecentActivitiesTool {
    /// Constructor con las bases reales de Strava.
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            strava: Strava::new(),
        }
    }

    /// Constructor para tests, con un cliente de Strava inyectable.
    pub fn with_strava(db: SqlitePool, strava: Strava) -> Self {
        Self { db, strava }
    }
}

#[async_trait]
impl Tool for StravaRecentActivitiesTool {
    fn name(&self) -> &'static str {
        "strava_recent_activities"
    }

    fn description(&self) -> &'static str {
        "Lista las actividades recientes de Strava del atleta, con filtros de fecha, paginación y deporte. Por defecto devuelve solo running (Run, TrailRun, VirtualRun)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "before": {
                    "type": "string",
                    "description": "Fecha ISO 8601 (p. ej. `2026-10-05` o `2026-10-05T00:00:00Z`) o epoch en segundos; solo actividades anteriores a ella"
                },
                "after": {
                    "type": "string",
                    "description": "Fecha ISO 8601 (p. ej. `2026-10-05` o `2026-10-05T00:00:00Z`) o epoch en segundos; solo actividades posteriores a ella"
                },
                "page": {
                    "type": "integer",
                    "description": "Número de página (paginación)"
                },
                "per_page": {
                    "type": "integer",
                    "description": "Número de actividades por página (por defecto 100; máximo 200)"
                },
                "sport_type": {
                    "type": "string",
                    "description": "Tipo de deporte a filtrar (p. ej. Run, Ride, Swim). Por defecto, running"
                }
            },
            "required": []
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let mut query: Vec<(&str, String)> = Vec::new();
        // `before`/`after` aceptan ISO 8601 o epoch; siempre se envían a Strava
        // como epoch en segundos.
        for key in ["before", "after"] {
            if let Some(value) = args.get(key).and_then(parse_epoch_bound) {
                query.push((key, value.to_string()));
            }
        }
        if let Some(page) = args.get("page").and_then(|v| v.as_i64()) {
            query.push(("page", page.to_string()));
        }
        // Strava aplica 30 por defecto; se pide una página amplia para que el
        // filtro por deporte no devuelva una página vacía llena de rides.
        let per_page = args
            .get("per_page")
            .and_then(|v| v.as_i64())
            .map(|value| value.clamp(1, MAX_PER_PAGE))
            .unwrap_or(DEFAULT_PER_PAGE);
        query.push(("per_page", per_page.to_string()));

        let body = self
            .strava
            .api_get(&self.db, "/athlete/activities", &query)
            .await
            .map_err(tool_error)?;

        let activities: Vec<Value> = match &body {
            Value::Array(items) => items.clone(),
            Value::Object(_) => body
                .get("activities")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        let filter: Vec<String> = match args.get("sport_type").and_then(|v| v.as_str()) {
            Some(sport) if !sport.trim().is_empty() => vec![sport.trim().to_string()],
            _ => DEFAULT_RUNNING_SPORTS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };

        let summaries: Vec<Value> = activities
            .iter()
            .filter(|activity| {
                let sport = sport_type_of(activity);
                filter.iter().any(|wanted| wanted == &sport)
            })
            .map(summarise_activity)
            .collect();

        let count = summaries.len();
        Ok(ToolResult {
            success: true,
            data: json!({ "activities": summaries, "count": count }),
            message: Some(format!("{count} actividad(es) reciente(s)")),
        })
    }
}

// ---------------------------------------------------------------------------
// strava_activity_detail
// ---------------------------------------------------------------------------

pub struct StravaActivityDetailTool {
    db: SqlitePool,
    strava: Strava,
}

impl StravaActivityDetailTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            strava: Strava::new(),
        }
    }

    pub fn with_strava(db: SqlitePool, strava: Strava) -> Self {
        Self { db, strava }
    }
}

#[async_trait]
impl Tool for StravaActivityDetailTool {
    fn name(&self) -> &'static str {
        "strava_activity_detail"
    }

    fn description(&self) -> &'static str {
        "Devuelve el detalle de una actividad de Strava: su resumen, sus vueltas (laps) y sus splits métricos."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "Identificador de la actividad de Strava"
                }
            },
            "required": ["id"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = required_id(&args)?;
        let path = format!("/activities/{id}");
        let body = self
            .strava
            .api_get(&self.db, &path, &[])
            .await
            .map_err(tool_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({
                "activity": summarise_activity(&body),
                "laps": summarise_laps(&body),
                "splits_metric": summarise_splits(&body),
            }),
            message: None,
        })
    }
}

// ---------------------------------------------------------------------------
// strava_activity_streams
// ---------------------------------------------------------------------------

pub struct StravaActivityStreamsTool {
    db: SqlitePool,
    strava: Strava,
}

impl StravaActivityStreamsTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            strava: Strava::new(),
        }
    }

    pub fn with_strava(db: SqlitePool, strava: Strava) -> Self {
        Self { db, strava }
    }
}

#[async_trait]
impl Tool for StravaActivityStreamsTool {
    fn name(&self) -> &'static str {
        "strava_activity_streams"
    }

    fn description(&self) -> &'static str {
        "Devuelve las series temporales de una actividad de Strava (tiempo, distancia, frecuencia cardiaca, cadencia, altitud). Si una serie es muy larga, se submuestrea y se indica en el mensaje."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "Identificador de la actividad de Strava"
                },
                "keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Series a devolver (p. ej. time, distance, heartrate, velocity_smooth, cadence, altitude). Por defecto, todas las habituales"
                }
            },
            "required": ["id"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = required_id(&args)?;

        let keys: Vec<String> = match args.get("keys").and_then(|v| v.as_array()) {
            Some(items) => {
                let parsed: Vec<String> = items
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if parsed.is_empty() {
                    DEFAULT_STREAM_KEYS.iter().map(|s| s.to_string()).collect()
                } else {
                    parsed
                }
            }
            None => DEFAULT_STREAM_KEYS.iter().map(|s| s.to_string()).collect(),
        };

        let query = vec![
            ("keys", keys.join(",")),
            ("key_by_type", "true".to_string()),
        ];
        let path = format!("/activities/{id}/streams");
        let body = self
            .strava
            .api_get(&self.db, &path, &query)
            .await
            .map_err(tool_error)?;

        let mut streams = serde_json::Map::new();
        let mut sampled_step = 1usize;
        let mut points = 0usize;

        if let Some(object) = body.as_object() {
            for (key, stream) in object {
                let data = stream
                    .get("data")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                points = points.max(data.len());
                let (data, step) = subsample(&data);
                sampled_step = sampled_step.max(step);
                streams.insert(key.clone(), Value::Array(data));
            }
        }

        let message = if sampled_step > 1 {
            Some(format!(
                "Series submuestreadas: se devuelve 1 de cada {sampled_step} puntos."
            ))
        } else {
            None
        };

        Ok(ToolResult {
            success: true,
            data: json!({ "streams": Value::Object(streams), "points": points }),
            message,
        })
    }
}

// ---------------------------------------------------------------------------
// strava_athlete_stats
// ---------------------------------------------------------------------------

pub struct StravaAthleteStatsTool {
    db: SqlitePool,
    strava: Strava,
}

impl StravaAthleteStatsTool {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            strava: Strava::new(),
        }
    }

    pub fn with_strava(db: SqlitePool, strava: Strava) -> Self {
        Self { db, strava }
    }
}

#[async_trait]
impl Tool for StravaAthleteStatsTool {
    fn name(&self) -> &'static str {
        "strava_athlete_stats"
    }

    fn description(&self) -> &'static str {
        "Devuelve el perfil del atleta de Strava (nombre, ciudad, seguidores) y sus totales de año y recientes (distancia, tiempo, desnivel)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let profile = self
            .strava
            .api_get(&self.db, "/athlete", &[])
            .await
            .map_err(tool_error)?;

        let athlete_id = match SettingsRepo::get(&self.db, "strava_athlete_id")
            .await
            .map_err(ToolError::from)?
        {
            Some(value) if !value.trim().is_empty() => value.trim().to_string(),
            _ => profile
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string())
                .ok_or_else(|| {
                    ToolError::ExecutionError(
                        "No se pudo determinar el atleta: conecta tu cuenta de Strava.".to_string(),
                    )
                })?,
        };

        let stats = self
            .strava
            .api_get(&self.db, &format!("/athletes/{athlete_id}/stats"), &[])
            .await
            .map_err(tool_error)?;

        Ok(ToolResult {
            success: true,
            data: json!({
                "athlete": summarise_athlete(&profile),
                "totals": summarise_totals(&stats),
            }),
            message: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::settings::SettingsRepo;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Epoch en segundos de una fecha a medianoche UTC, calculado con `chrono`
    /// para no fijar un literal que dependa de la zona horaria.
    fn midnight_utc(year: i32, month: u32, day: u32) -> i64 {
        chrono::NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(0, 0, 0)
            .expect("valid time")
            .and_utc()
            .timestamp()
    }

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    async fn seed_connected(pool: &SqlitePool) {
        SettingsRepo::set(pool, "strava_client_id", "cid")
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_client_secret", "csecret")
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_refresh_token", "REF")
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_access_token", "VALID")
            .await
            .unwrap();
        let expires = chrono::Utc::now().timestamp() + 6 * 3600;
        SettingsRepo::set(pool, "strava_expires_at", &expires.to_string())
            .await
            .unwrap();
        SettingsRepo::set(pool, "strava_athlete_id", "12345")
            .await
            .unwrap();
    }

    fn schema_required(params: &Value) -> Vec<String> {
        params["required"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn recent_activities_parameters_schema() {
        let pool = setup_pool().await;
        let tool = StravaRecentActivitiesTool::new(pool);
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        let props = params["properties"].as_object().unwrap();
        for key in ["before", "after", "page", "per_page", "sport_type"] {
            assert!(props.contains_key(key), "falta la propiedad {key}");
        }
        assert!(schema_required(&params).is_empty());
    }

    #[tokio::test]
    async fn activity_detail_parameters_schema() {
        let pool = setup_pool().await;
        let tool = StravaActivityDetailTool::new(pool);
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params["properties"].as_object().unwrap().contains_key("id"));
        assert_eq!(schema_required(&params), vec!["id".to_string()]);
    }

    #[tokio::test]
    async fn activity_streams_parameters_schema() {
        let pool = setup_pool().await;
        let tool = StravaActivityStreamsTool::new(pool);
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("id"));
        assert!(props.contains_key("keys"));
        assert_eq!(schema_required(&params), vec!["id".to_string()]);
    }

    #[tokio::test]
    async fn athlete_stats_parameters_schema() {
        let pool = setup_pool().await;
        let tool = StravaAthleteStatsTool::new(pool);
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(schema_required(&params).is_empty());
    }

    #[test]
    fn parse_epoch_bound_accepts_epoch_and_iso() {
        let expected = midnight_utc(2026, 10, 5);

        assert_eq!(
            parse_epoch_bound(&json!(1_700_000_000)),
            Some(1_700_000_000)
        );
        assert_eq!(parse_epoch_bound(&json!("1700000000")), Some(1_700_000_000));
        assert_eq!(parse_epoch_bound(&json!("2026-10-05")), Some(expected));
        assert_eq!(
            parse_epoch_bound(&json!("2026-10-05T00:00:00Z")),
            Some(expected)
        );
        assert_eq!(parse_epoch_bound(&json!("not-a-date")), None);
        assert_eq!(parse_epoch_bound(&json!(null)), None);
    }

    #[tokio::test]
    async fn recent_activities_accepts_iso_dates_as_epoch() {
        let pool = setup_pool().await;
        seed_connected(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(query_param("after", midnight_utc(2026, 10, 5).to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let tool = StravaRecentActivitiesTool::with_strava(pool, strava);

        let result = tool
            .execute(json!({ "after": "2026-10-05" }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["count"], 0);
    }

    #[tokio::test]
    async fn recent_activities_defaults_per_page_to_100() {
        let pool = setup_pool().await;
        seed_connected(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .and(query_param("per_page", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let tool = StravaRecentActivitiesTool::with_strava(pool, strava);

        let result = tool.execute(json!({})).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn recent_activities_summarises_and_filters_by_sport() {
        let pool = setup_pool().await;
        seed_connected(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete/activities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": 1,
                    "name": "Morning Run",
                    "sport_type": "Run",
                    "start_date_local": "2026-10-01T07:00:00Z",
                    "distance": 10000.0,
                    "moving_time": 3000,
                    "total_elevation_gain": 50.0,
                    "average_heartrate": 150.0,
                    "max_heartrate": 170.0
                },
                {
                    "id": 2,
                    "name": "Ride",
                    "sport_type": "Ride",
                    "start_date_local": "2026-10-01T09:00:00Z",
                    "distance": 20000.0,
                    "moving_time": 2400
                }
            ])))
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let tool = StravaRecentActivitiesTool::with_strava(pool, strava);

        let result = tool.execute(json!({ "per_page": 10 })).await.unwrap();
        assert!(result.success);

        let activities = result.data["activities"].as_array().unwrap();
        assert_eq!(activities.len(), 1, "solo debe quedar la Run");
        assert_eq!(activities[0]["id"], 1);
        assert_eq!(activities[0]["distance_km"], 10.0);
        assert_eq!(activities[0]["pace_min_km"], 5.0);
    }

    #[tokio::test]
    async fn activity_detail_includes_laps() {
        let pool = setup_pool().await;
        seed_connected(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/activities/99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 99,
                "name": "Long Run",
                "sport_type": "Run",
                "start_date_local": "2026-10-01T07:00:00Z",
                "distance": 21097.5,
                "moving_time": 7200,
                "laps": [
                    { "lap_index": 1, "distance": 10000.0, "moving_time": 3000, "average_heartrate": 145.0 },
                    { "lap_index": 2, "distance": 11097.5, "moving_time": 4200, "average_heartrate": 150.0 }
                ],
                "splits_metric": [
                    { "split": 1, "distance": 1000.0, "moving_time": 300, "elevation_difference": 5.0 }
                ]
            })))
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let tool = StravaActivityDetailTool::with_strava(pool, strava);

        let result = tool.execute(json!({ "id": 99 })).await.unwrap();
        let laps = result.data["laps"].as_array().unwrap();
        assert_eq!(laps.len(), 2);
        assert_eq!(laps[0]["index"], 1);
        let splits = result.data["splits_metric"].as_array().unwrap();
        assert_eq!(splits.len(), 1);
    }

    #[tokio::test]
    async fn athlete_stats_summarises_totals() {
        let pool = setup_pool().await;
        seed_connected(&pool).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athlete"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 12345,
                "firstname": "Jane",
                "lastname": "Doe",
                "city": "Madrid",
                "country": "Spain",
                "follower_count": 10
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v3/athletes/12345/stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "recent_run_totals": {
                    "distance": 50000.0,
                    "moving_time": 15000,
                    "elevation_gain": 500.0,
                    "count": 5
                },
                "ytd_run_totals": {
                    "distance": 500000.0,
                    "moving_time": 150000,
                    "elevation_gain": 5000.0,
                    "count": 50
                }
            })))
            .mount(&server)
            .await;

        let strava = Strava::with_bases(server.uri(), format!("{}/api/v3", server.uri()));
        let tool = StravaAthleteStatsTool::with_strava(pool, strava);

        let result = tool.execute(json!({})).await.unwrap();
        assert_eq!(result.data["athlete"]["name"], "Jane Doe");
        assert_eq!(result.data["athlete"]["city"], "Madrid");
        assert_eq!(
            result.data["totals"]["ytd_run_totals"]["distance_km"],
            500.0
        );
        assert_eq!(
            result.data["totals"]["recent_run_totals"]["distance_km"],
            50.0
        );
    }

    #[tokio::test]
    async fn tool_reports_not_connected_without_tokens() {
        let pool = setup_pool().await;
        // Sin tokens ni credenciales.
        let tool = StravaRecentActivitiesTool::new(pool);

        let err = tool.execute(json!({})).await.unwrap_err();
        match err {
            ToolError::ExecutionError(message) => {
                assert!(
                    message.to_lowercase().contains("conecta"),
                    "el mensaje debe pedir conectar la cuenta, got: {message}"
                );
            }
            other => panic!("esperaba ExecutionError, got: {other:?}"),
        }
    }
}
