//! Selección de skills por turno.
//!
//! El router llama **una sola vez** al clasificador, decide qué skills activa
//! el turno (con fallo abierto) y compone dos cosas: el conjunto de
//! herramientas expuesto al modelo y los fragmentos de prompt de las skills
//! activas.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::db::repos::settings::SettingsRepo;
use crate::llm::decisions::{DecisionsProvider, DecisionsQuestion, DecisionsRequest};
use crate::orchestrator::skills::{catalog, Skill, SkillSpec, CORE_TOOLS};

/// De dónde salió la selección.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionSource {
    /// El clasificador decidió (incluye la selección vacía bajo umbral).
    Router,
    /// El enrutador está apagado o no hay clasificador construido.
    Disabled,
    /// No había ninguna skill enrutable habilitada.
    NoRoutableSkills,
    /// El clasificador falló (error o timeout): fallo abierto.
    Error,
}

/// Telemetría de la llamada al clasificador, expuesta por el router para que el
/// orquestador la persista (D4).
///
/// Solo se puebla cuando hubo llamada real al clasificador (fuentes `Router` y
/// `Error`). El router **no** escribe en la tabla de estadísticas: solo la
/// devuelve aquí; el orquestador decide qué hacer con ella.
#[derive(Debug, Clone, PartialEq)]
pub struct RouterUsage {
    /// Modelo del clasificador que atendió la llamada.
    pub model: String,
    /// Tokens de entrada consumidos.
    pub input_tokens: i64,
    /// Tokens de salida generados.
    pub output_tokens: i64,
    /// Coste devuelto por el servicio.
    pub cost: f64,
    /// Latencia real de la llamada, en milisegundos.
    pub duration_ms: i64,
    /// Estado de la llamada: `"success"` o `"error"`.
    pub status: String,
}

/// Resultado de enrutar un turno.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Skills activadas, en orden del catálogo.
    pub skills: Vec<Skill>,
    /// Probabilidad de «sí» por skill preguntada.
    pub probabilities: Vec<(Skill, f32)>,
    /// De dónde salió la selección.
    pub source: SelectionSource,
    /// Telemetría de la llamada al clasificador, cuando la hubo.
    pub usage: Option<RouterUsage>,
}

/// Configuración del enrutador (se lee de `settings` en cada turno).
#[derive(Debug, Clone)]
pub struct SkillRouterConfig {
    pub enabled: bool,
    pub model: String,
    pub threshold: f32,
    /// Umbrales efectivos por skill que sobreescriben al global, indexados por
    /// id de skill (clave `ROUTER_THRESHOLD_<ID>`). Por defecto vacío.
    pub threshold_overrides: HashMap<String, f32>,
    pub timeout_ms: u64,
    pub history_turns: usize,
}

impl Default for SkillRouterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: "typesafe/jev-1.13".to_string(),
            threshold: 0.10,
            threshold_overrides: HashMap::new(),
            timeout_ms: 800,
            history_turns: 6,
        }
    }
}

/// Los tres textos con los que se pregunta al clasificador por una skill.
///
/// Es la forma **efectiva** de la pregunta: [`read_skill_criteria`] y
/// [`SkillRouter::with_criteria`] la construyen de modo que ningún campo quede
/// vacío (un campo ausente o en blanco cae al valor declarado en el catálogo).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillCriteria {
    pub instructions: String,
    pub criteria_true: String,
    pub criteria_false: String,
}

/// Enrutador de skills por turno.
pub struct SkillRouter {
    provider: Option<Arc<dyn DecisionsProvider>>,
    config: SkillRouterConfig,
    /// Criterios efectivos vigentes en `settings`, indexados por id de skill.
    /// Cada entrada sobreescribe los textos del catálogo para esa skill.
    criteria: HashMap<String, SkillCriteria>,
}

impl SkillRouter {
    /// Construye el router con un clasificador inyectable (`None` = sin
    /// clasificador, que en la práctica equivale a enrutador apagado).
    pub fn new(provider: Option<Arc<dyn DecisionsProvider>>, config: SkillRouterConfig) -> Self {
        Self {
            provider,
            config,
            criteria: HashMap::new(),
        }
    }

    /// Builder: adjunta los criterios efectivos vigentes.
    ///
    /// Las entradas se aplican al construir cada pregunta en [`select`]: un
    /// campo vacío o en blanco de una entrada cae al valor del catálogo, de
    /// modo que ninguna pregunta puede viajar sin criterios.
    pub fn with_criteria(mut self, criteria: HashMap<String, SkillCriteria>) -> Self {
        self.criteria = criteria;
        self
    }

    /// Umbral efectivo de una skill. Delega en [`effective_threshold`], que es
    /// la única implementación de la precedencia (la comparte la API del
    /// catálogo y el arnés; no se duplica aquí).
    pub fn effective_threshold(&self, spec: &SkillSpec) -> f32 {
        effective_threshold(&self.config, spec)
    }

    /// Decide las skills del turno a partir del mensaje actual, los últimos
    /// turnos y las herramientas habilitadas.
    ///
    /// Hace **una sola** llamada al clasificador por turno. Los fallos
    /// (enrutador apagado, sin clasificador, sin skills enrutables, error o
    /// timeout) no devuelven preguntas: dejan constancia en
    /// [`SelectionSource`] para que el llamante mande **todas** las
    /// herramientas habilitadas (fallo abierto).
    pub async fn select(
        &self,
        message: &str,
        history: &[String],
        enabled_tools: &[String],
    ) -> Selection {
        let fallback = |source: SelectionSource, usage: Option<RouterUsage>| {
            tracing::warn!(
                source = ?source,
                "skill routing fell open; exposing every enabled tool"
            );
            Selection {
                skills: Vec::new(),
                probabilities: Vec::new(),
                source,
                usage,
            }
        };

        if !self.config.enabled {
            return fallback(SelectionSource::Disabled, None);
        }

        let Some(provider) = self.provider.as_ref() else {
            return fallback(SelectionSource::Disabled, None);
        };

        // Solo se pregunta por lo que se puede ofrecer: una skill es enrutable
        // si al menos una de sus herramientas está habilitada.
        let routable: Vec<_> = catalog()
            .iter()
            .filter(|spec| {
                spec.tools.iter().any(|tool| {
                    enabled_tools
                        .iter()
                        .any(|enabled| enabled.as_str() == *tool)
                })
            })
            .collect();

        if routable.is_empty() {
            return fallback(SelectionSource::NoRoutableSkills, None);
        }

        // Estado: mensaje actual + los últimos `history_turns` turnos.
        let history_slice = if history.len() > self.config.history_turns {
            &history[history.len() - self.config.history_turns..]
        } else {
            history
        };
        let state = serde_json::json!({
            "message": message,
            "history": history_slice,
        });

        // Criterios efectivos: la entrada de `settings` si existe, con cada
        // campo cayendo al del catálogo cuando está vacío o en blanco. Así
        // ninguna pregunta puede viajar sin instrucciones ni criterios.
        let questions: Vec<DecisionsQuestion> = routable
            .iter()
            .map(|spec| {
                let entry = self.criteria.get(spec.id);
                DecisionsQuestion {
                    id: spec.id.to_string(),
                    instructions: effective_field(
                        entry.map(|c| c.instructions.as_str()),
                        spec.instructions,
                    ),
                    criteria_true: effective_field(
                        entry.map(|c| c.criteria_true.as_str()),
                        spec.criteria_true,
                    ),
                    criteria_false: effective_field(
                        entry.map(|c| c.criteria_false.as_str()),
                        spec.criteria_false,
                    ),
                }
            })
            .collect();

        let request = DecisionsRequest {
            model: self.config.model.clone(),
            state,
            questions,
        };

        let started = Instant::now();
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let response = match tokio::time::timeout(timeout, provider.decide(request)).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) | Err(_) => {
                return fallback(
                    SelectionSource::Error,
                    Some(RouterUsage {
                        model: self.config.model.clone(),
                        input_tokens: 0,
                        output_tokens: 0,
                        cost: 0.0,
                        duration_ms: started.elapsed().as_millis() as i64,
                        status: "error".to_string(),
                    }),
                );
            }
        };

        // Probabilidades de todas las skills preguntadas (orden del catálogo);
        // los ids de la respuesta que no correspondan se ignoran.
        let mut probabilities = Vec::new();
        let mut selected = Vec::new();
        let mut recognized = 0usize;
        for spec in &routable {
            let Some(&probability) = response.answers.get(spec.id) else {
                continue;
            };
            recognized += 1;
            probabilities.push((spec.skill, probability));
            if probability >= self.effective_threshold(spec) {
                selected.push(spec.skill);
            }
        }

        // R4/D6: si se preguntaron skills y **ninguna** respuesta se reconoció,
        // la respuesta es inútil (vacía o con ids desconocidos). Aplicarla
        // dejaría `skills` vacío con `source = Router` y el turno expondría
        // solo el core: el enrutador no puede quitar capacidades por un fallo
        // suyo. Se falla abierto. Una respuesta parcial (algunas reconocidas y
        // otras no) sí se aplica con lo reconocido.
        if recognized == 0 {
            return fallback(
                SelectionSource::Error,
                Some(RouterUsage {
                    model: self.config.model.clone(),
                    input_tokens: 0,
                    output_tokens: 0,
                    cost: 0.0,
                    duration_ms: started.elapsed().as_millis() as i64,
                    status: "error".to_string(),
                }),
            );
        }

        // R8/D10: la decisión se registra por log, incluyendo su coste y su
        // latencia. Nunca se escribe en la tabla de stats ni se toca
        // `last_api_call`.
        tracing::info!(
            skills = ?selected,
            probabilities = ?probabilities,
            source = "router",
            latency_ms = started.elapsed().as_millis() as u64,
            cost = response.cost,
            "skill routing decision"
        );

        Selection {
            skills: selected,
            probabilities,
            source: SelectionSource::Router,
            usage: Some(RouterUsage {
                model: self.config.model.clone(),
                input_tokens: response.input_tokens as i64,
                output_tokens: response.output_tokens as i64,
                cost: response.cost,
                duration_ms: started.elapsed().as_millis() as i64,
                status: "success".to_string(),
            }),
        }
    }
}

/// Umbral efectivo de una skill, con esta precedencia:
///
/// 1. El override por skill `ROUTER_THRESHOLD_<ID>` si está presente y es
///    válido (lo trae [`read_router_config`] en `threshold_overrides`).
/// 2. El umbral global `ROUTER_THRESHOLD`.
/// 3. El `spec.threshold` compilado en el catálogo.
///
/// El umbral global siempre está poblado ([`read_router_config`] cae a
/// [`SkillRouterConfig::default`]), así que el paso 3 solo se alcanzaría con
/// una configuración construida a mano sin global; se mantiene por
/// completitud de la precedencia. Sin overrides y con el global por defecto
/// el resultado es `0.10`; con el override sembrado de `widgets`, `0.20`.
///
/// Es una función libre para que la API del catálogo y el arnés reutilicen la
/// misma precedencia que el router sin construir uno.
pub fn effective_threshold(config: &SkillRouterConfig, spec: &SkillSpec) -> f32 {
    if let Some(&override_threshold) = config.threshold_overrides.get(spec.id) {
        return override_threshold;
    }
    if config.threshold.is_finite() && (0.0..=1.0).contains(&config.threshold) {
        return config.threshold;
    }
    spec.threshold
}

/// El valor efectivo de un campo editable de skill: el de `settings` si tiene
/// contenido, y si no el declarado en el catálogo. Un valor ausente o en blanco
/// nunca produce un campo vacío.
///
/// El valor sobrescrito se **recorta**, que es exactamente lo que viaja al
/// clasificador (`read_skill_criteria` lo obtiene de `trimmed()`). Así la API y
/// el informe declaran lo mismo que se envía.
///
/// Es una función libre (no un `closure`) para que el router, la lectura de
/// criterios y la API del catálogo compartan exactamente la misma regla.
pub fn effective_field(overridden: Option<&str>, catalog: &str) -> String {
    match overridden {
        Some(text) if !text.trim().is_empty() => text.trim().to_string(),
        _ => catalog.to_string(),
    }
}

/// `core ∪ skills seleccionadas ∩ habilitadas`. Nunca una deshabilitada.
///
/// En una decisión del [`SelectionSource::Router`] con selección vacía
/// devuelve solo el core (el turno no necesita herramientas). Con
/// [`SelectionSource::Disabled`], [`SelectionSource::NoRoutableSkills`] o
/// [`SelectionSource::Error`] —los casos de fallo abierto— devuelve **todas**
/// las habilitadas, que es el comportamiento sin enrutador.
pub fn exposed_tools(selection: &Selection, enabled_tools: &[String]) -> Vec<String> {
    match selection.source {
        SelectionSource::Disabled | SelectionSource::NoRoutableSkills | SelectionSource::Error => {
            let mut out = enabled_tools.to_vec();
            out.sort_unstable();
            out.dedup();
            out
        }
        SelectionSource::Router => {
            let mut out: Vec<String> = Vec::new();
            for core in CORE_TOOLS {
                if enabled_tools
                    .iter()
                    .any(|enabled| enabled.as_str() == *core)
                {
                    out.push((*core).to_string());
                }
            }
            for spec in catalog() {
                if !selection.skills.contains(&spec.skill) {
                    continue;
                }
                for tool in spec.tools {
                    if enabled_tools
                        .iter()
                        .any(|enabled| enabled.as_str() == *tool)
                        && !out.iter().any(|t| t.as_str() == *tool)
                    {
                        out.push((*tool).to_string());
                    }
                }
            }
            out
        }
    }
}

/// Fragmentos de las skills activas, en orden del catálogo; omite los vacíos y
/// los que ya estén presentes en `base_prompt`.
pub fn compose_skill_fragments(
    selection: &Selection,
    base_prompt: &str,
    fragments: &HashMap<String, String>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for spec in catalog() {
        if !selection.skills.contains(&spec.skill) {
            continue;
        }
        let Some(fragment) = fragments.get(spec.prompt_key) else {
            continue;
        };
        if fragment.trim().is_empty() {
            continue;
        }
        if base_prompt.contains(spec.prompt_heading) {
            continue;
        }
        out.push(fragment.clone());
    }
    out
}

// ─── Lectura en caliente de la configuración y los fragmentos (R7 / D9) ─────

/// Lee un ajuste en crudo (recortado); `None` si falta o si la lectura falla,
/// en cuyo caso registra un warning (trata el fallo como ausencia).
async fn read_raw(pool: &SqlitePool, key: &str) -> Option<String> {
    match SettingsRepo::get(pool, key).await {
        Ok(Some(value)) => Some(value.trim().to_string()),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, key = %key, "failed to read router setting; using default");
            None
        }
    }
}

/// Lee y parsea un ajuste numérico; un valor ausente o ilegible cae al default
/// (con `warn!` en el segundo caso). Nunca falla.
async fn read_number<T>(pool: &SqlitePool, key: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    match read_raw(pool, key).await {
        None => default,
        Some(raw) => raw.parse::<T>().unwrap_or_else(|_| {
            tracing::warn!(
                key = %key,
                value = %raw,
                "router setting is not a valid number; using the default"
            );
            default
        }),
    }
}

/// Tope de `ROUTER_HISTORY_TURNS`: es entrada arbitraria por la API de ajustes
/// y un historial más largo solo engorda el estado del clasificador.
const ROUTER_HISTORY_TURNS_MAX: usize = 10;

/// Lee el umbral `ROUTER_THRESHOLD`. Solo es válido si es **finito** y está en
/// `[0, 1]`: `"NaN"` e `"inf"` parsean con éxito pero desactivarían el enrutado
/// en silencio (`prob >= NaN` es siempre falso). Si no es válido, cae al
/// default con `warn!`.
async fn read_threshold(pool: &SqlitePool, default: f32) -> f32 {
    match read_raw(pool, "ROUTER_THRESHOLD").await {
        None => default,
        Some(raw) => match raw.parse::<f32>() {
            Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => value,
            _ => {
                tracing::warn!(
                    key = "ROUTER_THRESHOLD",
                    value = %raw,
                    "router threshold is not a finite value in [0, 1]; using the default"
                );
                default
            }
        },
    }
}

/// Lee el timeout `ROUTER_TIMEOUT_MS`. Un `0` (o un valor ilegible) haría que
/// toda llamada al clasificador expirase; cae al default con `warn!`.
async fn read_timeout_ms(pool: &SqlitePool, default: u64) -> u64 {
    match read_raw(pool, "ROUTER_TIMEOUT_MS").await {
        None => default,
        Some(raw) => match raw.parse::<u64>() {
            Ok(value) if value > 0 => value,
            _ => {
                tracing::warn!(
                    key = "ROUTER_TIMEOUT_MS",
                    value = %raw,
                    "router timeout is zero or not a number; using the default"
                );
                default
            }
        },
    }
}

/// Lee `ROUTER_HISTORY_TURNS` y recorta por encima de
/// [`ROUTER_HISTORY_TURNS_MAX`] con `warn!`.
async fn read_history_turns(pool: &SqlitePool, default: usize) -> usize {
    let value = read_number(pool, "ROUTER_HISTORY_TURNS", default).await;
    if value > ROUTER_HISTORY_TURNS_MAX {
        tracing::warn!(
            key = "ROUTER_HISTORY_TURNS",
            value = value,
            max = ROUTER_HISTORY_TURNS_MAX,
            "router history is above the maximum; capping it"
        );
        ROUTER_HISTORY_TURNS_MAX
    } else {
        value
    }
}

/// Lee los overrides por skill `ROUTER_THRESHOLD_<ID>` en **una sola** lectura
/// de la tabla, indexados por id de skill.
///
/// Un valor ausente **no** entra en el mapa: la skill usa el global. Un valor en
/// blanco, no numérico, `NaN`/`inf` o fuera de `[0, 1]` tampoco entra y se
/// registra un `warn!` (la skill cae al global y, si el global fuese ilegible,
/// al `spec.threshold` del catálogo). Si la lectura falla, devuelve un mapa
/// vacío tras registrarlo con `warn!`.
async fn read_threshold_overrides(pool: &SqlitePool) -> HashMap<String, f32> {
    let mut overrides = HashMap::new();

    let all = match SettingsRepo::get_all(pool).await {
        Ok(all) => all,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to read per-skill router thresholds; using the global"
            );
            return overrides;
        }
    };

    for spec in catalog() {
        let key = format!("ROUTER_THRESHOLD_{}", spec.id.to_ascii_uppercase());
        let Some(raw) = all.get(&key) else {
            continue;
        };
        let raw = raw.trim();
        match raw.parse::<f32>() {
            Ok(value) if value.is_finite() && (0.0..=1.0).contains(&value) => {
                overrides.insert(spec.id.to_string(), value);
            }
            _ => {
                tracing::warn!(
                    key = %key,
                    value = %raw,
                    "router per-skill threshold is not a finite value in [0, 1]; using the global"
                );
            }
        }
    }

    overrides
}

/// Lee la configuración del enrutador desde `settings`.
///
/// Lee en **cada** llamada (no cachea) para que editar los ajustes surta efecto
/// sin reiniciar. Un valor ausente o ilegible cae al default de
/// [`SkillRouterConfig::default()`] registrando un `tracing::warn!` (el ausente
/// no avisa: es el estado normal hasta que se edita). **Nunca** falla.
pub async fn read_router_config(pool: &SqlitePool) -> SkillRouterConfig {
    let defaults = SkillRouterConfig::default();

    let enabled = match read_raw(pool, "ROUTER_ENABLED").await {
        None => defaults.enabled,
        Some(raw) => match raw.to_ascii_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            other => {
                tracing::warn!(
                    key = "ROUTER_ENABLED",
                    value = %other,
                    "router setting is not a boolean; using the default"
                );
                defaults.enabled
            }
        },
    };

    let model = match read_raw(pool, "ROUTER_MODEL").await {
        None => defaults.model.clone(),
        Some(raw) if raw.is_empty() => {
            tracing::warn!(
                key = "ROUTER_MODEL",
                "router model is empty; using the default"
            );
            defaults.model
        }
        Some(raw) => raw,
    };

    let threshold = read_threshold(pool, defaults.threshold).await;
    let threshold_overrides = read_threshold_overrides(pool).await;
    let timeout_ms = read_timeout_ms(pool, defaults.timeout_ms).await;
    let history_turns = read_history_turns(pool, defaults.history_turns).await;

    SkillRouterConfig {
        enabled,
        model,
        threshold,
        threshold_overrides,
        timeout_ms,
        history_turns,
    }
}

/// Lee los fragmentos de las skills indicadas desde `settings` (una sola
/// consulta a la tabla).
///
/// El mapa va indexado por `prompt_key` (p. ej. `"SKILL_AGENDA_PROMPT"`), que es
/// como lo consume [`compose_skill_fragments`]. Una clave ausente **no** se
/// incluye: no se inventa una cadena vacía. Si la lectura falla, devuelve un
/// mapa vacío tras registrarlo con `warn!`.
pub async fn read_skill_fragments(pool: &SqlitePool, skills: &[Skill]) -> HashMap<String, String> {
    let mut fragments = HashMap::new();

    let all = match SettingsRepo::get_all(pool).await {
        Ok(all) => all,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read skill fragments; injecting none");
            return fragments;
        }
    };

    for spec in catalog() {
        if !skills.contains(&spec.skill) {
            continue;
        }
        if let Some(fragment) = all.get(spec.prompt_key) {
            fragments.insert(spec.prompt_key.to_string(), fragment.clone());
        }
    }

    fragments
}

/// Criterios **efectivos** de cada skill a partir de `settings`, indexados por
/// id de skill.
///
/// Hace **una sola** lectura de la tabla ([`SettingsRepo::get_all`]). Para cada
/// skill del catálogo recoge `SKILL_<ID>_QUESTION`, `SKILL_<ID>_CRITERIA_TRUE`
/// y `SKILL_<ID>_CRITERIA_FALSE`.
///
/// Cada campo se recorta y se toma del override si tiene contenido; si falta o
/// está en blanco, cae al valor declarado en el catálogo. Una skill solo entra
/// en el mapa si **al menos uno** de sus tres campos está sobrescrito (si no,
/// no aporta nada: el catálogo es equivalente). El resultado es que ninguna
/// entrada puede llevar un criterio vacío.
///
/// Si la lectura falla, devuelve un mapa vacío tras registrarlo con `warn!`.
pub async fn read_skill_criteria(pool: &SqlitePool) -> HashMap<String, SkillCriteria> {
    let mut criteria = HashMap::new();

    let all = match SettingsRepo::get_all(pool).await {
        Ok(all) => all,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read skill criteria; using the catalog");
            return criteria;
        }
    };

    for spec in catalog() {
        let id = spec.id.to_ascii_uppercase();
        let question = trimmed(all.get(&format!("SKILL_{id}_QUESTION")));
        let criteria_true = trimmed(all.get(&format!("SKILL_{id}_CRITERIA_TRUE")));
        let criteria_false = trimmed(all.get(&format!("SKILL_{id}_CRITERIA_FALSE")));

        if question.is_none() && criteria_true.is_none() && criteria_false.is_none() {
            continue;
        }

        criteria.insert(
            spec.id.to_string(),
            SkillCriteria {
                instructions: question.unwrap_or(spec.instructions).to_string(),
                criteria_true: criteria_true.unwrap_or(spec.criteria_true).to_string(),
                criteria_false: criteria_false.unwrap_or(spec.criteria_false).to_string(),
            },
        );
    }

    criteria
}

/// Recorta un valor de `settings` y lo devuelve solo si queda contenido: un
/// campo ausente o en blanco significa «usa el valor del catálogo».
fn trimmed(value: Option<&String>) -> Option<&str> {
    value.map(|v| v.trim()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::repos::settings::SettingsRepo;
    use crate::llm::decisions::{DecisionsRequest, DecisionsResponse};
    use crate::llm::provider::LLMError;
    use crate::orchestrator::skills::catalog;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Las trece herramientas del registry de producción.
    const ALL_TOOLS: &[&str] = &[
        "calendar",
        "geocode",
        "get_current_location",
        "get_current_time",
        "notes",
        "reminders",
        "render_widget",
        "reverse_geocode",
        "search_places",
        "tasks",
        "unified_search",
        "weather",
        "web_search",
    ];
    const CORE: &[&str] = &["get_current_time", "get_current_location"];

    const SKILL_AGENDA_KEY: &str = "SKILL_AGENDA_PROMPT";
    const SKILL_PENDIENTES_KEY: &str = "SKILL_PENDIENTES_PROMPT";
    const SKILL_ENTORNO_KEY: &str = "SKILL_ENTORNO_PROMPT";
    const SKILL_WIDGETS_KEY: &str = "SKILL_WIDGETS_PROMPT";

    fn all_enabled() -> Vec<String> {
        ALL_TOOLS.iter().map(|s| s.to_string()).collect()
    }

    fn core_enabled() -> Vec<String> {
        CORE.iter().map(|s| s.to_string()).collect()
    }

    /// Ordena y deduplica para comparar conjuntos sin depender del orden.
    fn sorted(mut names: Vec<String>) -> Vec<String> {
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Construye una selección ya decidida (para tests de ensamblado).
    fn selection(skills: Vec<Skill>) -> Selection {
        Selection {
            skills,
            probabilities: Vec::new(),
            source: SelectionSource::Router,
            usage: None,
        }
    }

    // ─── Dobles del clasificador ────────────────────────────────────────────

    struct FakeDecisions {
        answers: HashMap<String, f32>,
    }

    impl FakeDecisions {
        // Doble de test: devuelve el `Arc<dyn ...>` listo para inyectar, no `Self`.
        #[allow(clippy::new_ret_no_self)]
        fn new(answers: &[(&str, f32)]) -> Arc<dyn DecisionsProvider> {
            Arc::new(Self {
                answers: answers.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            })
        }
    }

    #[async_trait]
    impl DecisionsProvider for FakeDecisions {
        async fn decide(&self, _request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
            Ok(DecisionsResponse {
                answers: self.answers.clone(),
                input_tokens: 10,
                output_tokens: 5,
                cost: 0.00002,
            })
        }
    }

    struct FailingDecisions;

    #[async_trait]
    impl DecisionsProvider for FailingDecisions {
        async fn decide(&self, _request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
            Err(LLMError::HttpError("boom".to_string()))
        }
    }

    struct HangingDecisions;

    #[async_trait]
    impl DecisionsProvider for HangingDecisions {
        async fn decide(&self, _request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Err(LLMError::Timeout("never returned".to_string()))
        }
    }

    // ─── Umbral y selección ─────────────────────────────────────────────────

    #[tokio::test]
    async fn threshold_selects_only_skills_above_it() {
        let provider =
            FakeDecisions::new(&[("agenda", 0.81), ("pendientes", 0.08), ("entorno", 0.12)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.skills,
            vec![Skill::Agenda],
            "only the skill above the threshold is selected"
        );
        assert_eq!(
            sel.source,
            SelectionSource::Router,
            "a successful classifier run reports the Router source"
        );
    }

    #[tokio::test]
    async fn all_below_threshold_is_empty_and_exposes_core_only() {
        let provider =
            FakeDecisions::new(&[("agenda", 0.10), ("pendientes", 0.05), ("entorno", 0.20)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router.select("hola", &[], &all_enabled()).await;

        assert!(sel.skills.is_empty(), "no skill clears the threshold");
        assert_eq!(
            sel.source,
            SelectionSource::Router,
            "an empty selection is still a legitimate router decision"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(core_enabled()),
            "a conversational turn exposes only the core"
        );
    }

    #[tokio::test]
    async fn multiskill_selects_every_skill_above_threshold() {
        let provider =
            FakeDecisions::new(&[("entorno", 0.94), ("agenda", 0.81), ("pendientes", 0.02)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router
            .select(
                "busca una cafetería cerca y créame un evento mañana",
                &[],
                &all_enabled(),
            )
            .await;

        assert_eq!(
            sel.skills.len(),
            2,
            "both skills are active: {:?}",
            sel.skills
        );
        assert!(sel.skills.contains(&Skill::Entorno));
        assert!(sel.skills.contains(&Skill::Agenda));
    }

    // ─── Matriz de fallo abierto: todas las habilitadas ─────────────────────

    #[tokio::test]
    async fn router_disabled_fails_open_with_all_enabled() {
        let provider = FakeDecisions::new(&[("agenda", 1.0)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: false,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Disabled,
            "a disabled router must report Disabled"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn missing_provider_fails_open_with_all_enabled() {
        let router = SkillRouter::new(
            None,
            SkillRouterConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Disabled,
            "without a classifier the router is effectively disabled"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn no_routable_skills_fails_open_with_all_enabled() {
        let provider = FakeDecisions::new(&[("agenda", 1.0)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let enabled = core_enabled(); // only the core is enabled → nothing routable
        let sel = router.select("hola", &[], &enabled).await;

        assert_eq!(
            sel.source,
            SelectionSource::NoRoutableSkills,
            "with no routable tool enabled the classifier must not run"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &enabled)),
            sorted(enabled.clone()),
            "fail-open exposes every enabled tool (only the core here)"
        );
    }

    #[tokio::test]
    async fn classifier_error_fails_open_with_all_enabled() {
        let router = SkillRouter::new(
            Some(Arc::new(FailingDecisions)),
            SkillRouterConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Error,
            "a classifier error is reported as Error"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn classifier_timeout_fails_open_with_all_enabled() {
        let router = SkillRouter::new(
            Some(Arc::new(HangingDecisions)),
            SkillRouterConfig {
                enabled: true,
                timeout_ms: 10,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Error,
            "a timeout is reported as Error"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn empty_answers_fail_open_with_all_enabled() {
        // R4/D6: a response with no recognised answer at all is a classifier
        // failure, not a legitimate empty selection. It must never shrink the
        // exposed set.
        let provider = FakeDecisions::new(&[]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Error,
            "an empty answers map must fail open, not be read as an empty selection"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn only_unknown_ids_fail_open_with_all_enabled() {
        // R4/D6: answers that match none of the questions asked are useless —
        // applying them would silently drop every routable skill.
        let provider =
            FakeDecisions::new(&[("skill_inexistente", 0.99), ("otra_inexistente", 0.51)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(
            sel.source,
            SelectionSource::Error,
            "a response with only unknown ids must fail open"
        );
        assert_eq!(
            sorted(exposed_tools(&sel, &all_enabled())),
            sorted(all_enabled()),
            "fail-open exposes every enabled tool"
        );
    }

    #[tokio::test]
    async fn unknown_ids_are_ignored_and_the_rest_applies() {
        let provider = FakeDecisions::new(&[("skill_inexistente", 0.99), ("agenda", 0.81)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(sel.source, SelectionSource::Router);
        assert_eq!(
            sel.skills,
            vec![Skill::Agenda],
            "the unknown id is ignored and the valid one still applies"
        );
    }

    // ─── Telemetría del clasificador expuesta en la selección (D4) ──────────

    /// Un router apagado no llama al clasificador: no hay telemetría que exponer.
    #[tokio::test]
    async fn disabled_router_exposes_no_usage() {
        let provider = FakeDecisions::new(&[("agenda", 1.0)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: false,
                ..Default::default()
            },
        );

        let sel = router.select("hola", &[], &all_enabled()).await;

        assert_eq!(sel.source, SelectionSource::Disabled);
        assert!(
            sel.usage.is_none(),
            "a disabled router never calls the classifier, so it exposes no usage"
        );
    }

    /// Sin skills enrutables el clasificador no se invoca: tampoco hay telemetría.
    #[tokio::test]
    async fn no_routable_skills_expose_no_usage() {
        let provider = FakeDecisions::new(&[("agenda", 1.0)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let sel = router.select("hola", &[], &core_enabled()).await;

        assert_eq!(sel.source, SelectionSource::NoRoutableSkills);
        assert!(
            sel.usage.is_none(),
            "without a classifier call there is no usage to expose"
        );
    }

    /// Una decisión correcta expone modelo, tokens, coste, latencia y estado.
    #[tokio::test]
    async fn successful_router_exposes_usage() {
        let provider = FakeDecisions::new(&[("agenda", 0.9)]);
        let router = SkillRouter::new(
            Some(provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.3,
                model: "typesafe/jev-1.13".to_string(),
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(sel.source, SelectionSource::Router);
        let usage = sel
            .usage
            .expect("a classifier call must expose its telemetry");
        assert_eq!(usage.status, "success");
        assert_eq!(usage.model, "typesafe/jev-1.13");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
        assert!((usage.cost - 0.00002).abs() < 1e-12);
        assert!(
            usage.duration_ms >= 0,
            "the latency is a non-negative real time"
        );
    }

    /// Un fallo del clasificador también expone telemetría, marcada como error y
    /// sin tokens ni coste (D5).
    #[tokio::test]
    async fn failed_router_exposes_error_usage() {
        let router = SkillRouter::new(
            Some(Arc::new(FailingDecisions)),
            SkillRouterConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let sel = router
            .select("convoca una reunión", &[], &all_enabled())
            .await;

        assert_eq!(sel.source, SelectionSource::Error);
        let usage = sel
            .usage
            .expect("a failed classifier call must still expose telemetry");
        assert_eq!(usage.status, "error");
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cost, 0.0);
    }

    // ─── Nunca una deshabilitada ────────────────────────────────────────────

    #[test]
    fn a_disabled_tool_is_never_exposed_even_if_its_skill_is_selected() {
        let enabled = core_enabled(); // `calendar` is absent
        let sel = selection(vec![Skill::Agenda]);

        let exposed = sorted(exposed_tools(&sel, &enabled));

        assert!(
            !exposed.contains(&"calendar".to_string()),
            "a disabled tool must never be exposed: {exposed:?}"
        );
        assert_eq!(
            exposed,
            sorted(enabled.clone()),
            "the rest of the selection is exposed normally (core only here)"
        );
    }

    // ─── Activar una skill añade sus herramientas (R5) ──────────────────────

    #[test]
    fn activating_a_skill_adds_its_tools_to_the_exposed_set() {
        let enabled = all_enabled();

        let agenda = sorted(exposed_tools(&selection(vec![Skill::Agenda]), &enabled));
        assert!(
            agenda.contains(&"calendar".to_string()),
            "agenda adds calendar: {agenda:?}"
        );
        assert!(
            !agenda.contains(&"weather".to_string()),
            "non-selected skills do not appear: {agenda:?}"
        );

        let agenda_entorno = sorted(exposed_tools(
            &selection(vec![Skill::Agenda, Skill::Entorno]),
            &enabled,
        ));
        assert!(
            agenda_entorno.contains(&"weather".to_string()),
            "entorno adds weather: {agenda_entorno:?}"
        );
        assert!(
            agenda_entorno.contains(&"geocode".to_string()),
            "entorno adds its prerequisite geocode: {agenda_entorno:?}"
        );
        assert!(
            !agenda_entorno.contains(&"tasks".to_string()),
            "non-selected skills do not appear: {agenda_entorno:?}"
        );
    }

    // ─── Fragmentos de prompt ───────────────────────────────────────────────

    fn fragments_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Encabezado real de la skill si el catálogo ya está poblado; si no, un
    /// respaldo para que el test falle por aserción y no por pánico.
    fn heading_of(skill: Skill, fallback: &'static str) -> &'static str {
        catalog()
            .iter()
            .find(|spec| spec.skill == skill)
            .map(|spec| spec.prompt_heading)
            .unwrap_or(fallback)
    }

    #[test]
    fn composes_only_active_fragments_in_catalog_order() {
        let fragments = fragments_map(&[
            (SKILL_AGENDA_KEY, "FRAGMENTO_AGENDA"),
            (SKILL_PENDIENTES_KEY, "FRAGMENTO_TAREAS"),
            (SKILL_ENTORNO_KEY, "FRAGMENTO_CLIMA"),
        ]);

        let out = compose_skill_fragments(
            &selection(vec![Skill::Agenda, Skill::Pendientes]),
            "prompt base",
            &fragments,
        );

        assert_eq!(out.len(), 2, "only active skills contribute: {out:?}");
        assert!(
            out[0].contains("FRAGMENTO_AGENDA") && out[1].contains("FRAGMENTO_TAREAS"),
            "fragments follow the catalog order: {out:?}"
        );
        assert!(
            !out.iter().any(|f| f.contains("FRAGMENTO_CLIMA")),
            "inactive skills do not contribute: {out:?}"
        );
    }

    #[test]
    fn blank_fragments_leave_no_trace() {
        let fragments = fragments_map(&[
            (SKILL_AGENDA_KEY, "   \n  "),
            (SKILL_PENDIENTES_KEY, "FRAGMENTO_TAREAS"),
        ]);

        let out = compose_skill_fragments(
            &selection(vec![Skill::Agenda, Skill::Pendientes]),
            "prompt base",
            &fragments,
        );

        assert_eq!(out.len(), 1, "a blank fragment must be omitted: {out:?}");
        assert!(out[0].contains("FRAGMENTO_TAREAS"));
    }

    #[test]
    fn fragment_already_in_base_prompt_is_not_duplicated() {
        let heading = heading_of(Skill::Agenda, "## Skill: Agenda");
        let base = format!("prompt base\n{heading}\ncontenido del usuario");
        let agenda_fragment = format!("{heading} duplicado");

        let fragments = fragments_map(&[
            (SKILL_AGENDA_KEY, agenda_fragment.as_str()),
            (SKILL_PENDIENTES_KEY, "FRAGMENTO_TAREAS"),
        ]);

        let out = compose_skill_fragments(
            &selection(vec![Skill::Agenda, Skill::Pendientes]),
            &base,
            &fragments,
        );

        assert!(
            !out.iter().any(|f| f.contains("duplicado")),
            "a fragment already present in the base prompt must not be injected: {out:?}"
        );
        assert!(
            out.iter().any(|f| f.contains("FRAGMENTO_TAREAS")),
            "the rest of the active fragments are injected: {out:?}"
        );
    }

    #[test]
    fn widget_guide_is_not_duplicated_when_the_base_prompt_already_carries_it() {
        // The «edited» case: the base prompt carries the widget guide by its
        // real content heading —the migration relocates the block verbatim and
        // only removes it when it is byte-for-byte ours—, but the catalog's
        // duplicate marker used to be the *old* `# SKILL ACTIVA: WIDGETS`, so
        // the fragment was injected a second time. The test seeds the fragment
        // exactly as the migration does: starting with the guide heading.
        let guide_heading = "# Instrucciones de Interfaz y Widgets Interactivos";
        let fragment =
            format!("{guide_heading}\n\nDispones de la herramienta ejecutable `render_widget`.");
        let fragments = fragments_map(&[(SKILL_WIDGETS_KEY, fragment.as_str())]);

        let base_with_guide =
            format!("prompt base\n{guide_heading}\ncontenido editado por el usuario");
        let out = compose_skill_fragments(
            &selection(vec![Skill::Widgets]),
            &base_with_guide,
            &fragments,
        );
        assert!(
            out.is_empty(),
            "la guía de widgets ya está en el prompt base y no puede viajar dos veces: {out:?}"
        );

        let out =
            compose_skill_fragments(&selection(vec![Skill::Widgets]), "prompt base", &fragments);
        assert_eq!(
            out.len(),
            1,
            "sin el duplicado, la guía de widgets sí se inyecta: {out:?}"
        );
    }

    #[test]
    fn effective_field_trims_the_override() {
        // The router sends the trimmed value (it comes from `trimmed()`); the
        // API and the report must declare exactly what travels.
        assert_eq!(effective_field(Some("  hola  "), "catalogo"), "hola");
        assert_eq!(effective_field(Some("\t\n"), "catalogo"), "catalogo");
        assert_eq!(effective_field(None, "catalogo"), "catalogo");
    }

    // ─── Lectura en caliente de la configuración (R7 / D9) ──────────────────

    /// Pool en memoria con las migraciones aplicadas (los ajustes quedan
    /// sembrados con sus defaults).
    async fn db() -> SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        crate::db::schema::run_migrations(&pool)
            .await
            .expect("migrations");
        pool
    }

    /// Las cinco claves `ROUTER_*` que siembra la migración.
    const ROUTER_KEYS: &[&str] = &[
        "ROUTER_ENABLED",
        "ROUTER_MODEL",
        "ROUTER_THRESHOLD",
        "ROUTER_TIMEOUT_MS",
        "ROUTER_HISTORY_TURNS",
    ];

    #[tokio::test]
    async fn read_router_config_reads_the_current_values() {
        let pool = db().await;
        SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_MODEL", "typesafe/jev-2.0")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_THRESHOLD", "0.42")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_TIMEOUT_MS", "1500")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_HISTORY_TURNS", "5")
            .await
            .unwrap();

        let config = read_router_config(&pool).await;

        assert!(config.enabled, "ROUTER_ENABLED=true enables the router");
        assert_eq!(config.model, "typesafe/jev-2.0");
        assert!((config.threshold - 0.42).abs() < 1e-6);
        assert_eq!(config.timeout_ms, 1500);
        assert_eq!(config.history_turns, 5);
    }

    #[tokio::test]
    async fn read_router_config_rejects_non_finite_or_out_of_range_threshold() {
        // R7/D6: `"NaN".parse::<f32>()` and `"inf".parse::<f32>()` succeed, and
        // `prob >= NaN` is always false — a silent routing shut-off. The
        // threshold is only valid when finite and within `[0, 1]`.
        let pool = db().await;
        let defaults = SkillRouterConfig::default();

        for value in ["NaN", "inf", "-inf", "2.5", "-1"] {
            SettingsRepo::set(&pool, "ROUTER_THRESHOLD", value)
                .await
                .unwrap();
            let config = read_router_config(&pool).await;
            assert!(
                (config.threshold - defaults.threshold).abs() < f32::EPSILON,
                "threshold {value:?} must fall back to the default, got {}",
                config.threshold
            );
        }
    }

    #[tokio::test]
    async fn read_router_config_rejects_zero_timeout_and_caps_history() {
        // Both are arbitrary input from the settings API: a zero timeout would
        // make every classifier call time out, and an unbounded history bloats
        // the classifier state.
        let pool = db().await;
        let defaults = SkillRouterConfig::default();
        SettingsRepo::set(&pool, "ROUTER_TIMEOUT_MS", "0")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_HISTORY_TURNS", "999")
            .await
            .unwrap();

        let config = read_router_config(&pool).await;

        assert_eq!(
            config.timeout_ms, defaults.timeout_ms,
            "a zero timeout must fall back to the default"
        );
        assert_eq!(
            config.history_turns, 10,
            "an excessive history is capped at the maximum"
        );
    }

    #[tokio::test]
    async fn read_router_config_falls_back_to_defaults_when_absent() {
        let pool = db().await;
        for key in ROUTER_KEYS {
            SettingsRepo::delete(&pool, key).await.unwrap();
        }

        let config = read_router_config(&pool).await;
        let defaults = SkillRouterConfig::default();

        assert!(!config.enabled, "absent ROUTER_ENABLED defaults to false");
        assert_eq!(config.model, defaults.model);
        assert!((config.threshold - defaults.threshold).abs() < f32::EPSILON);
        assert_eq!(config.timeout_ms, defaults.timeout_ms);
        assert_eq!(config.history_turns, defaults.history_turns);
    }

    #[tokio::test]
    async fn read_router_config_falls_back_on_unreadable_values() {
        let pool = db().await;
        SettingsRepo::set(&pool, "ROUTER_ENABLED", "quizá")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_MODEL", "").await.unwrap();
        SettingsRepo::set(&pool, "ROUTER_THRESHOLD", "alta")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_TIMEOUT_MS", "poco")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_HISTORY_TURNS", "muchos")
            .await
            .unwrap();

        // Unreadable values must not fail the call: they fall back to defaults.
        let config = read_router_config(&pool).await;
        let defaults = SkillRouterConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.model, defaults.model);
        assert!((config.threshold - defaults.threshold).abs() < f32::EPSILON);
        assert_eq!(config.timeout_ms, defaults.timeout_ms);
        assert_eq!(config.history_turns, defaults.history_turns);
    }

    // ─── Lectura de fragmentos de las skills (R7 / D9) ──────────────────────

    #[tokio::test]
    async fn read_skill_fragments_returns_only_present_keys_of_requested_skills() {
        let pool = db().await;
        // Customise one key and drop another.
        SettingsRepo::delete(&pool, SKILL_PENDIENTES_KEY)
            .await
            .unwrap();
        SettingsRepo::set(&pool, SKILL_AGENDA_KEY, "FRAGMENTO_AGENDA")
            .await
            .unwrap();

        let fragments = read_skill_fragments(&pool, &[Skill::Agenda, Skill::Pendientes]).await;

        assert_eq!(
            fragments.get(SKILL_AGENDA_KEY).map(String::as_str),
            Some("FRAGMENTO_AGENDA"),
            "a present key is returned verbatim"
        );
        assert_eq!(
            fragments.get(SKILL_PENDIENTES_KEY),
            None,
            "a missing key must not be invented"
        );
        assert_eq!(
            fragments.len(),
            1,
            "only the requested, present keys appear"
        );
    }

    #[tokio::test]
    async fn read_skill_fragments_ignores_skills_not_requested() {
        let pool = db().await;
        SettingsRepo::set(&pool, SKILL_ENTORNO_KEY, "FRAGMENTO_ENTORNO")
            .await
            .unwrap();

        let fragments = read_skill_fragments(&pool, &[Skill::Entorno]).await;

        assert_eq!(fragments.len(), 1, "only the one requested skill is read");
        assert!(fragments.contains_key(SKILL_ENTORNO_KEY));
    }

    // ─── RED: umbrales por defecto y override (skill-router-tuning 1.2) ──────

    #[test]
    fn router_config_defaults_match_the_measurement() {
        let defaults = SkillRouterConfig::default();
        assert!(
            (defaults.threshold - 0.10).abs() < 1e-6,
            "el umbral global por defecto debe ser 0.10, got {}",
            defaults.threshold
        );
        assert_eq!(
            defaults.history_turns, 6,
            "el historial por defecto debe ser 6, got {}",
            defaults.history_turns
        );
    }

    /// La spec de una skill por su id, o `None` si aún no existe en el catálogo.
    fn spec_by_id(id: &str) -> Option<&'static SkillSpec> {
        catalog().iter().find(|spec| spec.id == id)
    }

    /// Umbral efectivo de una skill por su id, o `None` si no está el catálogo.
    fn effective_for(router: &SkillRouter, id: &str) -> Option<f32> {
        spec_by_id(id).map(|spec| router.effective_threshold(spec))
    }

    /// Porcentaje entero del umbral, para comparar sin ruido de coma flotante.
    fn pct(t: f32) -> i32 {
        (t * 100.0).round() as i32
    }

    #[tokio::test]
    async fn effective_threshold_uses_the_widget_override_when_present() {
        let pool = db().await;
        // Pin the global so the domain expectation does not depend on the
        // seeded value: the precedence is override → global → catalog default.
        SettingsRepo::set(&pool, "ROUTER_THRESHOLD", "0.10")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "ROUTER_THRESHOLD_WIDGETS", "0.55")
            .await
            .unwrap();
        let router = SkillRouter::new(None, read_router_config(&pool).await);

        let widgets = effective_for(&router, "widgets");
        assert_eq!(
            widgets.map(pct),
            Some(55),
            "el override de widgets debe dar 0.55; got {widgets:?}"
        );

        for id in ["agenda", "pendientes", "recuerdos", "entorno", "web"] {
            let t = effective_for(&router, id);
            assert_eq!(
                t.map(pct),
                Some(10),
                "el dominio `{id}` usa el global 0.10; got {t:?}"
            );
        }
    }

    #[tokio::test]
    async fn effective_threshold_falls_back_to_the_global_when_override_absent() {
        let pool = db().await;
        SettingsRepo::delete(&pool, "ROUTER_THRESHOLD_WIDGETS")
            .await
            .unwrap();
        SettingsRepo::delete(&pool, "ROUTER_THRESHOLD")
            .await
            .unwrap();
        let router = SkillRouter::new(None, read_router_config(&pool).await);

        let widgets = effective_for(&router, "widgets");
        assert_eq!(
            widgets.map(pct),
            Some(10),
            "sin override, widgets cae al global por defecto 0.10, no a su spec 0.20; got {widgets:?}"
        );

        let agenda = effective_for(&router, "agenda");
        assert_eq!(
            agenda.map(pct),
            Some(10),
            "sin override, agenda cae al global por defecto 0.10; got {agenda:?}"
        );
    }

    #[tokio::test]
    async fn effective_threshold_ignores_unreadable_widget_overrides() {
        let pool = db().await;
        SettingsRepo::set(&pool, "ROUTER_THRESHOLD", "0.10")
            .await
            .unwrap();
        for bad in ["", "NaN", "inf", "-inf", "2.5", "-1", "alta"] {
            SettingsRepo::set(&pool, "ROUTER_THRESHOLD_WIDGETS", bad)
                .await
                .unwrap();
            let router = SkillRouter::new(None, read_router_config(&pool).await);
            let widgets = effective_for(&router, "widgets");
            assert_eq!(
                widgets.map(pct),
                Some(10),
                "un override ilegible {bad:?} cae al global 0.10; got {widgets:?}"
            );
        }
    }

    // ─── RED: criterios efectivos (skill-router-tuning 1.3) ─────────────────

    #[tokio::test]
    async fn read_skill_criteria_is_empty_without_config_and_reads_question_overrides() {
        let pool = db().await;
        assert!(
            read_skill_criteria(&pool).await.is_empty(),
            "sin claves de criterios, el mapa debe estar vacío"
        );

        SettingsRepo::set(&pool, "SKILL_PENDIENTES_QUESTION", "¿pendientes?")
            .await
            .unwrap();
        let map = read_skill_criteria(&pool).await;
        assert_eq!(
            map.get("pendientes").map(|c| c.instructions.as_str()),
            Some("¿pendientes?"),
            "debe recogerse el override de la pregunta; got {map:?}"
        );
    }

    #[tokio::test]
    async fn read_skill_criteria_drops_blank_values() {
        let pool = db().await;
        SettingsRepo::set(&pool, "SKILL_AGENDA_QUESTION", "¿agenda?")
            .await
            .unwrap();
        SettingsRepo::set(&pool, "SKILL_PENDIENTES_QUESTION", "   ")
            .await
            .unwrap();

        let map = read_skill_criteria(&pool).await;
        assert_eq!(
            map.get("agenda").map(|c| c.instructions.as_str()),
            Some("¿agenda?"),
            "una clave con contenido debe aparecer; got {map:?}"
        );
        assert!(
            !map.contains_key("pendientes"),
            "una clave solo con espacios no debe aportar un override; got {map:?}"
        );
    }

    /// Doble que además **captura** la `DecisionsRequest` enviada.
    struct CapturingDecisions {
        answers: HashMap<String, f32>,
        captured: Mutex<Option<DecisionsRequest>>,
    }

    impl CapturingDecisions {
        #[allow(clippy::new_ret_no_self)]
        fn new(answers: &[(&str, f32)]) -> Arc<Self> {
            Arc::new(Self {
                answers: answers.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
                captured: Mutex::new(None),
            })
        }

        fn captured_request(&self) -> Option<DecisionsRequest> {
            self.captured.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl DecisionsProvider for CapturingDecisions {
        async fn decide(&self, request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
            *self.captured.lock().unwrap() = Some(request);
            Ok(DecisionsResponse {
                answers: self.answers.clone(),
                input_tokens: 10,
                output_tokens: 5,
                cost: 0.00002,
            })
        }
    }

    #[tokio::test]
    async fn select_sends_overridden_criteria_and_falls_back_to_the_catalog() {
        let provider = CapturingDecisions::new(&[("agenda", 0.9), ("entorno", 0.9)]);
        let dyn_provider: Arc<dyn DecisionsProvider> = provider.clone();

        let mut criteria = HashMap::new();
        criteria.insert(
            "agenda".to_string(),
            SkillCriteria {
                instructions: "INSTRUCCIONES_SOBRESCRITAS".to_string(),
                criteria_true: "TRUE_SOBRESCRITO".to_string(),
                criteria_false: "FALSE_SOBRESCRITO".to_string(),
            },
        );
        // Override parcial: solo la pregunta; las criteria caen al catálogo.
        criteria.insert(
            "entorno".to_string(),
            SkillCriteria {
                instructions: "PREGUNTA_ENTORNO".to_string(),
                criteria_true: String::new(),
                criteria_false: String::new(),
            },
        );

        let router = SkillRouter::new(
            Some(dyn_provider),
            SkillRouterConfig {
                enabled: true,
                threshold: 0.0,
                ..Default::default()
            },
        )
        .with_criteria(criteria);

        let _ = router
            .select("convoca una reunión y dime el tiempo", &[], &all_enabled())
            .await;

        let request = provider
            .captured_request()
            .expect("el doble debe capturar la petición enviada");

        let agenda = request
            .questions
            .iter()
            .find(|q| q.id == "agenda")
            .expect("agenda debe preguntarse");
        assert_eq!(
            agenda.instructions, "INSTRUCCIONES_SOBRESCRITAS",
            "la pregunta sobrescrita debe viajar al clasificador"
        );
        assert_eq!(agenda.criteria_true, "TRUE_SOBRESCRITO");
        assert_eq!(agenda.criteria_false, "FALSE_SOBRESCRITO");

        let entorno = request
            .questions
            .iter()
            .find(|q| q.id == "entorno")
            .expect("entorno debe preguntarse");
        assert_eq!(
            entorno.instructions, "PREGUNTA_ENTORNO",
            "la pregunta sobrescrita de entorno debe viajar"
        );
        let entorno_spec = spec_by_id("entorno").expect("entorno debe estar en el catálogo");
        assert_eq!(
            entorno.criteria_true, entorno_spec.criteria_true,
            "un override parcial no puede dejar una criteria vacía: cae al catálogo"
        );
        assert_eq!(
            entorno.criteria_false, entorno_spec.criteria_false,
            "un override parcial no puede dejar una criteria vacía: cae al catálogo"
        );

        for q in &request.questions {
            assert!(
                !q.instructions.trim().is_empty(),
                "ninguna pregunta puede viajar sin instrucciones: {q:?}"
            );
            assert!(
                !q.criteria_true.trim().is_empty(),
                "ninguna pregunta puede viajar con criteria_true vacía: {q:?}"
            );
            assert!(
                !q.criteria_false.trim().is_empty(),
                "ninguna pregunta puede viajar con criteria_false vacía: {q:?}"
            );
        }
    }
}
