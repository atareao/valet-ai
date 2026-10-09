use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::models::stats::{CallKind, LastApiCall};
use tokio::sync::mpsc;

use crate::db::repos::stats::StatsRepo;
use crate::llm::decisions::DecisionsProvider;
use crate::llm::provider::{ChatMessage, ChatRequest, LLMProvider, StreamEvent, ToolCall};
use crate::orchestrator::context_builder::ContextBuilder;
use crate::orchestrator::context_classifier::ContextClassifier;
use crate::orchestrator::guardrails::{ApprovalOutcome, GuardrailResult, Guardrails};
use crate::orchestrator::skill_router::{
    compose_skill_fragments, exposed_tools, read_router_config, read_skill_fragments, SkillRouter,
};
use crate::tools::geo_utils::reverse_geocode;
use crate::tools::r#trait::ToolResult;
use crate::tools::registry::ToolRegistry;
use crate::tools::time_format::format_browser_timestamp;
use crate::tools::widget::RENDER_WIDGET_TOOL_NAME;
use futures::StreamExt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_iterations: usize,
    /// Legacy ceiling for tokens per turn. The chat is now governed by the
    /// `GENERATION_CHAT_MAX_TOKENS` setting (read on every turn via
    /// [`crate::generation::read_generation_params`]); this field is kept for
    /// API compatibility and is no longer used to build the chat request.
    pub max_tokens_per_turn: u32,
    pub model: String,
    pub enable_reflection: bool,
    /// Token threshold above which a message is sent to the collapse worker.
    pub collapse_threshold_tokens: usize,
}

/// Minimal generic system prompt used only when `settings.system_prompt` is
/// missing or empty. The real personality prompt lives in the database
/// (seeded by migration `20260929000001_prompts.sql`).
const DEFAULT_SYSTEM_PROMPT_FALLBACK: &str = "You are Valet, a helpful AI assistant.";

/// Maximum characters kept per turn when building the router's conversational
/// state. Cost and context hygiene: the state must not grow without bound.
const ROUTER_HISTORY_TURN_MAX_CHARS: usize = 400;

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_tokens_per_turn: 4096,
            model: "default".into(),
            enable_reflection: true,
            collapse_threshold_tokens: 2000,
        }
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub message: String,
    pub tool_calls: Vec<ToolCallInfo>,
    pub reflection: Option<Reflection>,
    pub iterations: usize,
}

#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: Value,
    pub result: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct Reflection {
    pub is_coherent: bool,
    pub is_complete: bool,
    pub needs_clarification: Option<String>,
    pub suggested_followup: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    LLMError(String),
    #[error("Tool error: {0}")]
    ToolError(String),
    #[error("Guardrail error: {0}")]
    GuardrailError(String),
    #[error("Context error: {0}")]
    ContextError(String),
    #[error("Max iterations exceeded")]
    MaxIterationsExceeded,
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<crate::orchestrator::context_builder::ContextError> for AgentError {
    fn from(err: crate::orchestrator::context_builder::ContextError) -> Self {
        AgentError::ContextError(err.to_string())
    }
}

impl From<sqlx::Error> for AgentError {
    fn from(err: sqlx::Error) -> Self {
        AgentError::Internal(err.to_string())
    }
}

impl From<crate::llm::provider::LLMError> for AgentError {
    fn from(err: crate::llm::provider::LLMError) -> Self {
        AgentError::LLMError(err.to_string())
    }
}

impl From<crate::orchestrator::guardrails::GuardrailError> for AgentError {
    fn from(err: crate::orchestrator::guardrails::GuardrailError) -> Self {
        AgentError::GuardrailError(err.to_string())
    }
}

impl From<crate::tools::r#trait::ToolError> for AgentError {
    fn from(err: crate::tools::r#trait::ToolError) -> Self {
        AgentError::ToolError(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// SSE events (used by streaming endpoint)
// ---------------------------------------------------------------------------

/// Events sent to the frontend via SSE during orchestrator streaming.
///
/// Each variant is serialized with a `"type"` tag that the frontend
/// uses to distinguish event kinds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SSEEvent {
    /// A text chunk of the assistant's response.
    #[serde(rename = "chunk")]
    Chunk { content: String },

    /// A tool was invoked by the LLM.
    #[serde(rename = "tool_call")]
    ToolCall { name: String, args: Value },

    /// The result of a tool execution.
    #[serde(rename = "tool_result")]
    ToolResult { name: String, success: bool },

    /// Streaming is complete; the assistant message has been persisted.
    #[serde(rename = "done")]
    Done {
        message_id: String,
        user_message_id: String,
        location: Option<String>,
        tools_used: Option<String>,
        user_location: Option<String>,
        user_created_at: String,
    },

    /// An error occurred during processing.
    #[serde(rename = "error")]
    Error { message: String },

    /// Human-in-the-loop approval is required before a tool can proceed.
    #[serde(rename = "approval_required")]
    ApprovalRequired {
        request_id: String,
        tool_name: String,
        reason: String,
    },

    /// The result of an approval resolution (approved or denied).
    #[serde(rename = "approval_result")]
    ApprovalResult { request_id: String, approved: bool },

    /// The orchestrator asks the client to render an interactive widget.
    #[serde(rename = "widget")]
    Widget {
        id: String,
        name: String,
        data: Value,
    },
}

impl SSEEvent {
    /// Serialise this event to a JSON string suitable for an SSE `data:` field.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"type":"error","message":"Failed to serialize SSE event"}"#.to_string()
        })
    }
}

/// Contextual information from the user's browser (date, time, location).
///
/// Injected as a system message so the LLM can personalise responses
/// (e.g. adjust greetings, interpret relative dates, offer local info).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserContext {
    pub timestamp: String,
    pub timezone: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_name: Option<String>,
}

/// Maximum number of times the same tool+operation can be called in one ReAct loop.
const MAX_TOOL_RETRIES: usize = 5;

/// Maximum time a turn waits for a human approval decision before treating it
/// as a denial (`ApprovalOutcome::TimedOut`).
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);

/// Title of the code-composed episodic-memory section injected as a `system`
/// message (block 8.3). The block is composed here, never from a placeholder in
/// the editable `settings.system_prompt` (design D2).
const EPISODIC_MEMORY_SECTION_TITLE: &str = "# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)";

/// Title of the code-composed persistent-memory section (Layer C). It is
/// injected between the prompt and the episodic section when the state is
/// non-empty, and omitted entirely otherwise.
const PERSISTENT_MEMORY_SECTION_TITLE: &str = "# MEMORIA PERSISTENTE (CAPA C)";

/// Title of the code-composed user-name section. It is injected right after the
/// prompt and before the persistent-memory section, and omitted entirely when
/// the profile has no real name.
const USER_NAME_SECTION_TITLE: &str = "# USUARIO";

/// Explicit instruction that the persistent state is stable, permanent context
/// and not the user's current turn.
const PERSISTENT_MEMORY_INSTRUCTION: &str = "Estado estable del usuario (perfil y reglas fijadas). Es contexto permanente; NO es el turno actual del usuario.";

/// Explicit instruction that the recovered cards are background, not the turn
/// currently being answered. Without it the model can reply to a memory as if
/// it were the user's current message.
const EPISODIC_MEMORY_INSTRUCTION: &str = "Las fichas siguientes son antecedentes recuperados de conversaciones anteriores. NO forman parte del turno actual del usuario; úsalas solo como contexto para entender sus preferencias e historia.";

/// Compose the episodic-memory block from the already-formatted cards, or
/// `None` when there is nothing to inject.
///
/// Used by `process_message_stream` so the streaming path emits the
/// **identical** block (same format, same position). Returns `None` for an
/// empty slice, so no filler text (e.g. "no hay antecedentes") is ever
/// injected. The block is composed in code (design D2).
fn compose_episodic_memory_block(memories: &[String]) -> Option<String> {
    if memories.is_empty() {
        return None;
    }
    let mut block = String::with_capacity(256);
    block.push_str(EPISODIC_MEMORY_SECTION_TITLE);
    block.push('\n');
    block.push_str(EPISODIC_MEMORY_INSTRUCTION);
    block.push_str("\n\n<episodic_memory>\n");
    for memory in memories {
        block.push_str("- ");
        block.push_str(memory);
        block.push('\n');
    }
    block.push_str("</episodic_memory>");
    Some(block)
}

/// Compose the persistent-memory section (Layer C) from the raw stored JSON
/// payload, or `None` when there is nothing to inject.
///
/// The payload is re-serialized in its **minified** form, preceded by a short
/// header and a one-line purpose. An empty state — or an unparseable payload —
/// yields `None`, so no header, marker or blank line is ever left behind. The
/// state is read in every request construction by the caller.
fn compose_persistent_memory_block(payload_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    if crate::persistent_memory::is_empty_state(&value) {
        return None;
    }
    Some(format!(
        "{PERSISTENT_MEMORY_SECTION_TITLE}\n{PERSISTENT_MEMORY_INSTRUCTION}\n{}",
        crate::persistent_memory::minified_json(&value)
    ))
}

/// Compose the user-name section from the profile name, or `None` when it must
/// not be injected.
///
/// The section is composed in code — never from a `settings.system_prompt`
/// placeholder. A name that is blank after trimming, or that still equals the
/// default profile name, yields `None`, so no title, guidance or blank line is
/// ever left behind. The name is trimmed before use.
fn compose_user_name_section(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name == crate::db::repos::profiles::DEFAULT_PROFILE_NAME {
        return None;
    }
    Some(format!(
        "{USER_NAME_SECTION_TITLE}\nEl nombre del usuario es {name}. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta."
    ))
}

/// Compose the single `role:"system"` message that opens every request.
///
/// Sections are appended in order and the ones that are present are separated
/// by a blank line (`\n\n`):
///
///   1. the prompt (`settings.system_prompt`),
///   2. the user-name section, when the profile has a real name,
///   3. the persistent-memory section (Capa C), when the state is non-empty,
///   4. the episodic-memory section, when there are cards, and
///   5. the date/time/location section, when the browser sent context.
///
/// An absent section leaves no trace: no title, marker, separator or stray
/// blank line. With only the prompt the result is *exactly* the prompt.
fn compose_system_message(
    prompt: &str,
    user_name: Option<String>,
    persistent_memory: Option<String>,
    episodic_memory: Option<String>,
    browser_section: Option<String>,
) -> String {
    let mut sections: Vec<String> = Vec::with_capacity(5);
    sections.push(prompt.to_string());
    if let Some(section) = user_name {
        sections.push(section);
    }
    if let Some(section) = persistent_memory {
        sections.push(section);
    }
    if let Some(section) = episodic_memory {
        sections.push(section);
    }
    if let Some(section) = browser_section {
        sections.push(section);
    }
    sections.join("\n\n")
}

// ---------------------------------------------------------------------------
// Orchestrator — ReAct loop
// ---------------------------------------------------------------------------

pub struct Orchestrator {
    pub llm: Arc<dyn LLMProvider>,
    pub registry: Arc<ToolRegistry>,
    pub guardrails: Arc<Guardrails>,
    pub context_builder: Arc<ContextBuilder>,
    pub classifier: ContextClassifier,
    pub config: OrchestratorConfig,
    pub db: SqlitePool,
    pub collapse_tx: Option<mpsc::Sender<String>>,
    pub memory_tx: Option<mpsc::Sender<()>>,
    pub last_api_call: Arc<RwLock<Option<LastApiCall>>>,
    /// Cliente del modelo de decisiones. `None` = sin credencial (el enrutado
    /// cae a fallo abierto). Nace a `None` en [`Orchestrator::new`], de modo que
    /// el enrutado queda apagado salvo que se adjunte con
    /// [`Orchestrator::with_decisions`].
    pub decisions: Option<Arc<dyn DecisionsProvider>>,
}

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        llm: Arc<dyn LLMProvider>,
        registry: Arc<ToolRegistry>,
        guardrails: Arc<Guardrails>,
        context_builder: Arc<ContextBuilder>,
        config: OrchestratorConfig,
        db: SqlitePool,
        collapse_tx: Option<mpsc::Sender<String>>,
        memory_tx: Option<mpsc::Sender<()>>,
        last_api_call: Arc<RwLock<Option<LastApiCall>>>,
    ) -> Self {
        Self {
            llm,
            registry,
            guardrails,
            context_builder,
            classifier: ContextClassifier::new(),
            config,
            db,
            collapse_tx,
            memory_tx,
            last_api_call,
            decisions: None,
        }
    }

    /// Builder: adjunta el cliente de decisiones (producción). `None` deja el
    /// enrutado apagado y cae a fallo abierto.
    pub fn with_decisions(mut self, decisions: Option<Arc<dyn DecisionsProvider>>) -> Self {
        self.decisions = decisions;
        self
    }

    /// Save the last API call data in memory so it can be served by the stats endpoint.
    #[allow(clippy::too_many_arguments)]
    fn save_last_call(
        &self,
        model: &str,
        request_body: Option<&str>,
        response_body: Option<&str>,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
        cached_tokens: u32,
        reasoning_tokens: u32,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        error_message: Option<&str>,
        tool_calls: Option<&str>,
    ) {
        let last = LastApiCall {
            model: model.to_string(),
            request_body: request_body.map(|s| s.to_string()),
            response_body: response_body.map(|s| s.to_string()),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_tokens,
            reasoning_tokens,
            cost,
            duration_ms,
            status: status.to_string(),
            error_message: error_message.map(|s| s.to_string()),
            tool_calls: tool_calls.map(|s| s.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        *self.last_api_call.write().unwrap() = Some(last);
    }

    /// Record a failed streaming LLM call for observability: one `llm_requests`
    /// row and the in-memory last-call snapshot, both with status `"error"`.
    ///
    /// This only records the failure; it does not swallow it. The caller is
    /// still responsible for propagating the original error with `return Err`.
    async fn record_stream_failure(&self, profile_id: &str, duration_ms: i64, error_message: &str) {
        let _ = StatsRepo::record_request(
            &self.db,
            CallKind::Chat,
            &Uuid::new_v4().to_string(),
            &self.config.model,
            Some(profile_id),
            0,
            0,
            0,
            0,
            0,
            0.0,
            Some(duration_ms),
            "error",
            Some(error_message),
            None,
            None,
        )
        .await;
        self.save_last_call(
            &self.config.model,
            None,
            None,
            0,
            0,
            0,
            0,
            0,
            0.0,
            Some(duration_ms),
            "error",
            Some(error_message),
            None,
        );
    }

    /// Resolve the user's current location name from settings or reverse geocoding.
    /// Caches the result back to settings so subsequent calls are instant.
    async fn resolve_location(&self) -> Option<String> {
        // First try stored location_name
        if let Some(name) = crate::db::repos::settings::SettingsRepo::get(&self.db, "location_name")
            .await
            .ok()
            .flatten()
        {
            return Some(name);
        }

        // Fall back to reverse geocoding from stored lat/lon
        let lat = crate::db::repos::settings::SettingsRepo::get(&self.db, "latitude")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok());
        let lon = crate::db::repos::settings::SettingsRepo::get(&self.db, "longitude")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok());

        if let (Some(lat), Some(lon)) = (lat, lon) {
            let address = crate::tools::geo_utils::reverse_geocode(lat, lon).await?;
            // Cache back to settings so next call is instant
            let _ =
                crate::db::repos::settings::SettingsRepo::set(&self.db, "location_name", &address)
                    .await;
            Some(address)
        } else {
            None
        }
    }

    /// Streaming entry point: runs the ReAct loop and emits [`SSEEvent`] values
    /// over the provided channel so the frontend can receive them incrementally.
    pub async fn process_message_stream(
        &self,
        profile_id: &str,
        user_message: &str,
        browser_context: Option<BrowserContext>,
        tx: mpsc::Sender<SSEEvent>,
    ) -> Result<(), AgentError> {
        tracing::info!(
            user_message_len = %user_message.len(),
            "🚀 Orchestrator processing message stream"
        );

        let mut iterations = 0usize;
        let mut messages: Vec<ChatMessage> = Vec::new();
        let mut used_tools: Vec<String> = Vec::new();
        let mut rendered_widgets: Vec<Value> = Vec::new();
        let mut tool_call_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // 1. Classify intent
        let classification = self.classifier.classify(user_message);

        // 2. Build context
        let ctx = self
            .context_builder
            .build(classification.strategy.clone(), profile_id, user_message)
            .await?;

        // Read settings from DB (max_window_tokens, system_prompt)
        let max_window_tokens =
            crate::db::repos::settings::SettingsRepo::get(&self.db, "max_window_tokens")
                .await?
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(10000);

        // The system prompt is stored in the `settings` table (seeded by
        // migration). If it is missing or empty, fall back to a minimal prompt.
        let system_prompt =
            match crate::db::repos::settings::SettingsRepo::get(&self.db, "system_prompt").await? {
                Some(p) if !p.trim().is_empty() => p,
                _ => {
                    tracing::warn!(
                        "settings.system_prompt is missing or empty; using minimal fallback"
                    );
                    DEFAULT_SYSTEM_PROMPT_FALLBACK.to_string()
                }
            };

        // Load conversation history from the DB on the token budget. Done here,
        // once per turn and before composing the system message, so a single
        // read feeds both the router and the request. The final order of
        // `messages` stays `[system, ...history..., user]`.
        let history = crate::db::repos::messages::MessagesRepo::list_by_token_budget(
            &self.db,
            max_window_tokens,
        )
        .await?;

        // Skill routing: one decision per turn, taken before the ReAct loop
        // (D1). The router reads its settings on every turn, so toggling it
        // takes effect without a restart. On any failure it falls open by
        // exposing every enabled tool — the behaviour without a router.
        let router = SkillRouter::new(self.decisions.clone(), read_router_config(&self.db).await);

        let enabled_tools: Vec<String> = self
            .registry
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();

        // Conversational state for the classifier: each turn is truncated so
        // the state cannot grow without bound; `select` keeps only the last
        // `ROUTER_HISTORY_TURNS`.
        let history_for_router: Vec<String> = history
            .iter()
            .map(|msg| {
                let turn = format!("{}: {}", msg.role, msg.content);
                turn.chars().take(ROUTER_HISTORY_TURN_MAX_CHARS).collect()
            })
            .collect();

        let selection = router
            .select(user_message, &history_for_router, &enabled_tools)
            .await;

        // `core ∪ skills_seleccionadas ∩ habilitadas`. Computed once, outside
        // the loop: every iteration advertises the same set and never decides
        // again nor re-reads settings.
        let exposed_names = exposed_tools(&selection, &enabled_tools);
        let exposed_refs: Vec<&str> = exposed_names.iter().map(String::as_str).collect();

        tracing::debug!(
            exposed_tools = ?exposed_names,
            source = ?selection.source,
            "Skill routing decision for this turn"
        );

        // Prompt fragments of the active skills (R6/D8), inserted after the base
        // prompt and before the code-composed sections. The base prompt is never
        // modified: the duplicate check uses the original.
        let fragments = read_skill_fragments(&self.db, &selection.skills).await;
        let skill_sections = compose_skill_fragments(&selection, &system_prompt, &fragments);
        let prompt_with_skills = if skill_sections.is_empty() {
            system_prompt.clone()
        } else {
            format!("{}\n\n{}", system_prompt, skill_sections.join("\n\n"))
        };

        // 3. ReAct loop
        tracing::debug!(
            system_prompt_len = %ctx.system_prompt.len(),
            rag_count = %ctx.rag_memories.len(),
            "Context built for ReAct loop"
        );

        // Browser context (date/time/location from the user's browser). It is
        // composed here because resolving the location name may hit the
        // network (`reverse_geocode` is async), then handed to the composer as
        // the closing section. No browser context means no section at all.
        let browser_section = if let Some(ref ctx) = browser_context {
            let fecha = format_browser_timestamp(&ctx.timestamp, &ctx.timezone)
                .unwrap_or_else(|| ctx.timestamp.clone());

            let mut parts = vec![format!("{:}.", fecha)];

            // Reverse‑geocode coordinates if we have them but no location name yet
            let location_name = if let (Some(lat), Some(lon)) = (ctx.latitude, ctx.longitude) {
                match &ctx.location_name {
                    Some(name) => Some(name.clone()),
                    None => reverse_geocode(lat, lon).await,
                }
            } else {
                None
            };

            if let (Some(lat), Some(lon)) = (ctx.latitude, ctx.longitude) {
                match &location_name {
                    Some(name) => {
                        parts.push(format!("Ubicación: {} ({:.4}, {:.4}).", name, lat, lon))
                    }
                    None => parts.push(format!("Coordenadas: ({:.4}, {:.4}).", lat, lon)),
                }
            }

            tracing::info!(
                timestamp = %ctx.timestamp,
                timezone = %ctx.timezone,
                latitude = ?ctx.latitude,
                longitude = ?ctx.longitude,
                location_name = ?location_name,
                "🌍 Browser context injected"
            );

            Some(parts.join(" "))
        } else {
            None
        };

        // Layer C: read the persistent state on EVERY request construction and
        // inject its minified section between the prompt and the episodic one.
        // A missing row or an empty state leaves no trace.
        let persistent_section =
            match crate::db::repos::persistent_memory::PersistentMemoryRepo::get(&self.db).await {
                Ok(Some(state)) => compose_persistent_memory_block(&state.payload),
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to read persistent memory; omitting the section"
                    );
                    None
                }
            };

        // The user's display name is read from the profile on EVERY request and
        // composed in code (never from a `system_prompt` placeholder). A blank
        // name, the default name, a missing profile or a read error all omit
        // the section without aborting the request.
        let user_name =
            match crate::db::repos::profiles::ProfilesRepo::get_by_id(&self.db, profile_id).await {
                Ok(Some(profile)) => compose_user_name_section(&profile.name),
                Ok(None) => None,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to read profile; omitting the user-name section"
                    );
                    None
                }
            };

        // Single system message: prompt → user-name section (when the profile
        // has a real name) → persistent-memory section (Capa C, when non-empty)
        // → episodic section → browser context. Absent sections leave no trace,
        // so with only the prompt the message is exactly the prompt, and the
        // browser section always closes it.
        let system_content = compose_system_message(
            &prompt_with_skills,
            user_name,
            persistent_section,
            compose_episodic_memory_block(&ctx.rag_memories),
            browser_section,
        );
        messages.push(ChatMessage {
            role: "system".into(),
            content: system_content,
            tool_calls: None,
            tool_result: None,
            tool_call_id: None,
        });

        // Append the conversation history pre-loaded above (same content and
        // same order as before: `[system, ...history..., user]`).
        for msg in &history {
            let tool_calls: Option<Vec<ToolCall>> = msg
                .tool_calls
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            messages.push(ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                tool_calls,
                tool_result: msg.tool_results.clone(),
                tool_call_id: None,
            });
        }

        messages.push(ChatMessage {
            role: "user".into(),
            content: user_message.to_string(),
            tool_calls: None,
            tool_result: None,
            tool_call_id: None,
        });

        // Persist user message to DB
        let location = self.resolve_location().await;
        let user_message_id = {
            let collapse_callback = self
                .collapse_tx
                .clone()
                .map(crate::workers::collapse::collapse_forwarder);
            let msg = crate::db::repos::messages::MessagesRepo::create(
                &self.db,
                "user",
                user_message,
                None,
                None,
                location.as_deref(),
                None, // tools_used (user messages don't have this)
                self.config.collapse_threshold_tokens,
                collapse_callback,
            )
            .await?;
            msg.id
        };

        // Notify the episodic memory worker about the new user message
        if let Some(ref tx) = self.memory_tx {
            let _ = tx.send(()).await;
        }

        'react_loop: loop {
            if iterations >= self.config.max_iterations {
                tracing::error!(error = %AgentError::MaxIterationsExceeded, "❌ Orchestrator error");
                let _ = tx
                    .send(SSEEvent::Error {
                        message: "Max iterations exceeded".into(),
                    })
                    .await
                    .ok();
                return Err(AgentError::MaxIterationsExceeded);
            }

            tracing::debug!(iteration = %iterations, "ReAct loop iteration");

            let generation = crate::generation::read_generation_params(
                &self.db,
                crate::generation::GenerationRole::Chat,
            )
            .await;

            let request = ChatRequest {
                model: self.config.model.clone(),
                messages: messages.clone(),
                tools: Some(self.registry.definitions_for(&exposed_refs)),
                temperature: Some(generation.temperature),
                max_tokens: Some(generation.max_tokens),
                stream: true,
                reasoning: generation.reasoning,
                response_format: None,
            };

            let request_body_str = serde_json::to_string(&request).unwrap_or_default();
            let call_start = std::time::Instant::now();
            let mut stream = match self.llm.chat_stream(request).await {
                Ok(s) => s,
                Err(e) => {
                    let duration_ms = call_start.elapsed().as_millis() as i64;
                    self.record_stream_failure(profile_id, duration_ms, &e.to_string())
                        .await;
                    return Err(e.into());
                }
            };
            let stream_start = std::time::Instant::now();

            iterations += 1;

            let mut content_buffer = String::new();
            let mut tool_calls_from_stream: Option<Vec<ToolCall>> = None;

            tracing::debug!(
                iteration = %iterations,
                "LLM stream started"
            );

            while let Some(event) = stream.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        let duration_ms = stream_start.elapsed().as_millis() as i64;
                        let error_message = e.to_string();
                        self.record_stream_failure(profile_id, duration_ms, &error_message)
                            .await;
                        return Err(AgentError::LLMError(error_message));
                    }
                };
                match event {
                    StreamEvent::Chunk(text) => {
                        content_buffer.push_str(&text);
                        if tx.send(SSEEvent::Chunk { content: text }).await.is_err() {
                            return Ok(()); // client disconnected
                        }
                    }
                    StreamEvent::ToolCall(tc) => {
                        tool_calls_from_stream.get_or_insert_with(Vec::new).push(tc);
                    }
                    StreamEvent::Done(response) => {
                        // Prefer tool_calls from Done (full arguments from finalize())
                        let tool_calls = response
                            .message
                            .tool_calls
                            .clone()
                            .or_else(|| tool_calls_from_stream.take());

                        // Record stats for this LLM call
                        let prompt_tokens = response
                            .usage
                            .as_ref()
                            .map(|u| u.prompt_tokens as i64)
                            .unwrap_or(0);
                        let completion_tokens = response
                            .usage
                            .as_ref()
                            .map(|u| u.completion_tokens as i64)
                            .unwrap_or(0);
                        let total_tokens = prompt_tokens + completion_tokens;

                        let _ = StatsRepo::record_request(
                            &self.db,
                            CallKind::Chat,
                            &Uuid::new_v4().to_string(),
                            &self.config.model,
                            Some(profile_id),
                            prompt_tokens,
                            completion_tokens,
                            total_tokens,
                            response
                                .usage
                                .as_ref()
                                .map(|u| u.cached_tokens as i64)
                                .unwrap_or(0),
                            response
                                .usage
                                .as_ref()
                                .map(|u| u.reasoning_tokens as i64)
                                .unwrap_or(0),
                            response.usage.as_ref().map(|u| u.cost).unwrap_or(0.0),
                            Some(stream_start.elapsed().as_millis() as i64),
                            "success",
                            None,
                            None,
                            None,
                        )
                        .await;

                        self.save_last_call(
                            &self.config.model,
                            Some(&request_body_str),
                            Some(&serde_json::to_string(&response).unwrap_or_default()),
                            prompt_tokens as u32,
                            completion_tokens as u32,
                            total_tokens as u32,
                            response
                                .usage
                                .as_ref()
                                .map(|u| u.cached_tokens)
                                .unwrap_or(0),
                            response
                                .usage
                                .as_ref()
                                .map(|u| u.reasoning_tokens)
                                .unwrap_or(0),
                            response.usage.as_ref().map(|u| u.cost).unwrap_or(0.0),
                            Some(stream_start.elapsed().as_millis() as i64),
                            "success",
                            None,
                            None,
                        );

                        if let Some(tcs) = tool_calls {
                            let content = content_buffer.clone();

                            tracing::debug!(tool_calls_from_stream = ?tool_calls_from_stream.as_ref().map(|t| t.len()), response_tool_calls = ?response.message.tool_calls.as_ref().map(|t| t.len()), "📦 Orchestrator tool_calls check");
                            tracing::debug!(
                                iteration = %iterations,
                                tool_call_count = %tcs.len(),
                                "Tool calls received from stream"
                            );

                            messages.push(ChatMessage {
                                role: "assistant".into(),
                                content,
                                tool_calls: Some(tcs.clone()),
                                tool_result: None,
                                tool_call_id: None,
                            });

                            for tc in &tcs {
                                tracing::info!(tool_name = %tc.name, "🔧 Executing tool call");

                                // Emit tool_call event
                                if tx
                                    .send(SSEEvent::ToolCall {
                                        name: tc.name.clone(),
                                        args: tc.arguments.clone(),
                                    })
                                    .await
                                    .is_err()
                                {
                                    return Ok(()); // client disconnected
                                }

                                // Guardrails check
                                let guardrail = self
                                    .guardrails
                                    .check(&tc.name, &tc.arguments)
                                    .map_err(|e| {
                                        tracing::error!(error = %e, "❌ Orchestrator error");
                                        AgentError::GuardrailError(e.to_string())
                                    })?;

                                let allowed = match guardrail {
                                    GuardrailResult::Allowed { .. } => true,
                                    GuardrailResult::RequiresApproval { request_id } => {
                                        let _ = tx
                                            .send(SSEEvent::ApprovalRequired {
                                                request_id: request_id.clone(),
                                                tool_name: tc.name.clone(),
                                                reason: format!(
                                                    "Tool '{}' requires explicit approval",
                                                    tc.name
                                                ),
                                            })
                                            .await
                                            .ok();

                                        let approved = matches!(
                                            self.guardrails
                                                .await_approval(&request_id, APPROVAL_TIMEOUT)
                                                .await,
                                            ApprovalOutcome::Approved
                                        );

                                        let _ = tx
                                            .send(SSEEvent::ApprovalResult {
                                                request_id: request_id.clone(),
                                                approved,
                                            })
                                            .await
                                            .ok();

                                        approved
                                    }
                                };

                                if allowed {
                                    // Check per-tool retry limit (max 3 calls per tool per ReAct loop)
                                    let op = tc.arguments.get("operation").and_then(|v| v.as_str());
                                    let op_key = match op {
                                        Some(op_val) => format!("{}::{}", tc.name, op_val),
                                        None => tc.name.clone(),
                                    };
                                    let tool_count =
                                        tool_call_counts.entry(op_key.clone()).or_insert(0);
                                    *tool_count += 1;
                                    if *tool_count > MAX_TOOL_RETRIES {
                                        let display_name = match op {
                                            Some(op_val) => format!("{}::{}", tc.name, op_val),
                                            None => tc.name.clone(),
                                        };
                                        tracing::warn!(
                                            tool_name = %display_name,
                                            call_count = %tool_count,
                                            "⚠️ Tool retry limit reached"
                                        );
                                        let msg = format!(
                                                "Tool '{}' has been called 3 times. No more retries allowed. Inform the user and suggest alternatives.",
                                                display_name
                                            );
                                        messages.push(ChatMessage {
                                            role: "tool".into(),
                                            content: msg.clone(),
                                            tool_calls: None,
                                            tool_result: None,
                                            tool_call_id: Some(tc.id.clone()),
                                        });
                                        // Still emit tool_result event so frontend knows tool was "called"
                                        let _ = tx
                                            .send(SSEEvent::ToolResult {
                                                name: tc.name.clone(),
                                                success: false,
                                            })
                                            .await
                                            .ok();
                                        continue;
                                    }

                                    tracing::debug!(
                                        tool_name = %tc.name,
                                        tool_args = %tc.arguments,
                                        "🔧 Executing tool"
                                    );

                                    // Inject profile_id from authenticated session
                                    let mut args = tc.arguments.clone();
                                    if let Some(obj) = args.as_object_mut() {
                                        obj.insert(
                                            "profile_id".into(),
                                            serde_json::json!(profile_id),
                                        );
                                    }

                                    let tool_result =
                                        match self.registry.execute(&tc.name, args).await {
                                            Ok(result) => result,
                                            Err(e) => {
                                                tracing::error!(
                                                    tool_name = %tc.name,
                                                    tool_args = %tc.arguments,
                                                    error = %e,
                                                    error_debug = ?e,
                                                    "❌ Tool execution error"
                                                );
                                                ToolResult {
                                                    success: false,
                                                    data: serde_json::json!({}),
                                                    message: Some(e.to_string()),
                                                }
                                            }
                                        };

                                    match tool_result.success {
                                        true => {
                                            // Track the tool name for the footer
                                            used_tools.push(op_key.clone());

                                            // Emit success event
                                            let _ = tx
                                                .send(SSEEvent::ToolResult {
                                                    name: tc.name.clone(),
                                                    success: true,
                                                })
                                                .await
                                                .ok();

                                            // A successful `render_widget` call is surfaced
                                            // as a dedicated event so the client can render
                                            // the widget live, before the turn finishes.
                                            if tc.name == RENDER_WIDGET_TOOL_NAME {
                                                // The tool reports the validated widget
                                                // name and the normalised data; without a
                                                // name there is nothing to render.
                                                if let Some(widget_name) = tool_result
                                                    .data
                                                    .get("widget_name")
                                                    .and_then(|v| v.as_str())
                                                {
                                                    let widget_data = tool_result
                                                        .data
                                                        .get("data")
                                                        .filter(|v| v.is_object())
                                                        .cloned()
                                                        .unwrap_or_else(|| serde_json::json!({}));
                                                    // Generate the widget id once and reuse it
                                                    // both for the SSE event and the persisted
                                                    // record, so the reloaded widget keeps the
                                                    // same `[widget:Name#id]` identity.
                                                    let widget_id = Uuid::new_v4().to_string();
                                                    let _ = tx
                                                        .send(SSEEvent::Widget {
                                                            id: widget_id.clone(),
                                                            name: widget_name.to_string(),
                                                            data: widget_data.clone(),
                                                        })
                                                        .await
                                                        .ok();
                                                    rendered_widgets.push(serde_json::json!({
                                                        "id": widget_id,
                                                        "name": widget_name,
                                                        "data": widget_data,
                                                    }));
                                                }
                                            }

                                            messages.push(ChatMessage {
                                                role: "tool".into(),
                                                content: serde_json::to_string(&tool_result.data)
                                                    .unwrap_or_default(),
                                                tool_calls: None,
                                                tool_result: Some(tool_result.data),
                                                tool_call_id: Some(tc.id.clone()),
                                            });
                                        }
                                        false => {
                                            let err_msg = tool_result.message.unwrap_or_default();
                                            tracing::error!(
                                                tool_name = %tc.name,
                                                tool_args = %tc.arguments,
                                                error_msg = %err_msg,
                                                "❌ Tool returned failure"
                                            );
                                            let _ = tx
                                                .send(SSEEvent::ToolResult {
                                                    name: tc.name.clone(),
                                                    success: false,
                                                })
                                                .await
                                                .ok();

                                            messages.push(ChatMessage {
                                                role: "tool".into(),
                                                content: format!("Error: {}", err_msg),
                                                tool_calls: None,
                                                tool_result: None,
                                                tool_call_id: Some(tc.id.clone()),
                                            });
                                        }
                                    }
                                } else {
                                    let msg = format!(
                                        "Tool '{}' was not approved by the user. Do not retry it; inform the user.",
                                        tc.name
                                    );
                                    let _ = tx
                                        .send(SSEEvent::ToolResult {
                                            name: tc.name.clone(),
                                            success: false,
                                        })
                                        .await
                                        .ok();
                                    messages.push(ChatMessage {
                                        role: "tool".into(),
                                        content: msg,
                                        tool_calls: None,
                                        tool_result: None,
                                        tool_call_id: Some(tc.id.clone()),
                                    });
                                }
                            }

                            content_buffer.clear();
                            continue 'react_loop;
                        } else {
                            // No tool calls — final answer

                            // Build tools_used string with counts
                            let tools_used: Option<String> = if !used_tools.is_empty() {
                                let mut counts: std::collections::HashMap<String, usize> =
                                    std::collections::HashMap::new();
                                for t in &used_tools {
                                    *counts.entry(t.clone()).or_insert(0) += 1;
                                }
                                let mut parts: Vec<String> = Vec::new();
                                // Sort for deterministic output
                                let mut keys: Vec<&String> = counts.keys().collect();
                                keys.sort();
                                for key in keys {
                                    let count = counts[key];
                                    if count > 1 {
                                        parts.push(format!("({}) {}", count, key));
                                    } else {
                                        parts.push(key.clone());
                                    }
                                }
                                Some(parts.join(", "))
                            } else {
                                None
                            };

                            // Persist assistant message to DB (capture the real UUID)
                            let location = self.resolve_location().await;
                            let assistant_message_id = {
                                let collapse_callback = self
                                    .collapse_tx
                                    .clone()
                                    .map(crate::workers::collapse::collapse_forwarder);
                                let msg = crate::db::repos::messages::MessagesRepo::create(
                                    &self.db,
                                    "assistant",
                                    &content_buffer,
                                    None,
                                    None,
                                    location.as_deref(),
                                    tools_used.as_deref(),
                                    self.config.collapse_threshold_tokens,
                                    collapse_callback,
                                )
                                .await?;
                                msg.id
                            };

                            // Persist the widgets emitted during this turn, if any,
                            // so the frontend can rebuild them on reload.
                            if !rendered_widgets.is_empty() {
                                crate::db::repos::messages::MessagesRepo::set_widgets(
                                    &self.db,
                                    &assistant_message_id,
                                    &serde_json::json!(rendered_widgets.clone()),
                                )
                                .await?;
                            }

                            // Notify the episodic memory worker about the new assistant message
                            if let Some(ref tx) = self.memory_tx {
                                let _ = tx.send(()).await;
                            }

                            // Fetch user message metadata for the frontend
                            let (user_location, user_created_at) = {
                                crate::db::repos::messages::MessagesRepo::find_by_id(
                                    &self.db,
                                    &user_message_id,
                                )
                                .await
                                .ok()
                                .flatten()
                                .map(|msg| (msg.location, msg.created_at))
                                .unwrap_or_else(|| (None, String::new()))
                            };

                            let _ = tx
                                .send(SSEEvent::Done {
                                    message_id: assistant_message_id,
                                    user_message_id: user_message_id.clone(),
                                    location: location.clone(),
                                    tools_used: tools_used.clone(),
                                    user_location,
                                    user_created_at,
                                })
                                .await
                                .ok();

                            tracing::info!(
                                iterations,
                                "✅ Orchestrator finished processing message"
                            );

                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reflection analyzer
// ---------------------------------------------------------------------------

pub struct ReflectionAnalyzer {
    llm: Arc<dyn LLMProvider>,
    last_api_call: Arc<RwLock<Option<LastApiCall>>>,
}

impl ReflectionAnalyzer {
    pub fn new(llm: Arc<dyn LLMProvider>, last_api_call: Arc<RwLock<Option<LastApiCall>>>) -> Self {
        Self { llm, last_api_call }
    }

    #[allow(clippy::too_many_arguments)]
    fn save_last_call(
        &self,
        model: &str,
        request_body: Option<&str>,
        response_body: Option<&str>,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
        cached_tokens: u32,
        reasoning_tokens: u32,
        cost: f64,
        duration_ms: Option<i64>,
        status: &str,
        error_message: Option<&str>,
        tool_calls: Option<&str>,
    ) {
        let last = LastApiCall {
            model: model.to_string(),
            request_body: request_body.map(|s| s.to_string()),
            response_body: response_body.map(|s| s.to_string()),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cached_tokens,
            reasoning_tokens,
            cost,
            duration_ms,
            status: status.to_string(),
            error_message: error_message.map(|s| s.to_string()),
            tool_calls: tool_calls.map(|s| s.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        *self.last_api_call.write().unwrap() = Some(last);
    }

    /// Ask the LLM to reflect on its own response — checking coherence,
    /// completeness, and whether clarification is needed.
    pub async fn analyze(
        &self,
        conversation: &[ChatMessage],
        response: &str,
        db: &SqlitePool,
        profile_id: &str,
    ) -> Result<Reflection, AgentError> {
        let conv_json = serde_json::to_string(conversation).unwrap_or_default();

        let prompt = format!(
            r#"Analyze this assistant response:

Conversation:
{}

Response:
{}

Answer the following yes/no questions (reply with one JSON object):
- Is the response coherent?
- Is the response complete?
- Does it need clarification from the user?
- What suggested follow-up would you propose?

Respond in JSON format:
{{"is_coherent": bool, "is_complete": bool, "needs_clarification": bool, "followup": "..."}}"#,
            conv_json, response
        );

        let request = ChatRequest {
            model: "default".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: prompt,
                tool_calls: None,
                tool_result: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(0.3),
            max_tokens: Some(256),
            stream: false,
            reasoning: None,
            response_format: None,
        };

        let request_body_str = serde_json::to_string(&request).unwrap_or_default();
        let start = Instant::now();
        match self.llm.chat(request).await {
            Ok(resp) => {
                // Record stats for this LLM call
                let duration_ms = start.elapsed().as_millis() as i64;
                let prompt_tokens = resp
                    .usage
                    .as_ref()
                    .map(|u| u.prompt_tokens as i64)
                    .unwrap_or(0);
                let completion_tokens = resp
                    .usage
                    .as_ref()
                    .map(|u| u.completion_tokens as i64)
                    .unwrap_or(0);
                let total_tokens = prompt_tokens + completion_tokens;
                let _ = StatsRepo::record_request(
                    db,
                    CallKind::Chat,
                    &Uuid::new_v4().to_string(),
                    "default",
                    Some(profile_id),
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    resp.usage
                        .as_ref()
                        .map(|u| u.cached_tokens as i64)
                        .unwrap_or(0),
                    resp.usage
                        .as_ref()
                        .map(|u| u.reasoning_tokens as i64)
                        .unwrap_or(0),
                    resp.usage.as_ref().map(|u| u.cost).unwrap_or(0.0),
                    Some(duration_ms),
                    "success",
                    None,
                    None,
                    None,
                )
                .await;

                self.save_last_call(
                    "default",
                    Some(&request_body_str),
                    Some(&serde_json::to_string(&resp).unwrap_or_default()),
                    prompt_tokens as u32,
                    completion_tokens as u32,
                    total_tokens as u32,
                    resp.usage.as_ref().map(|u| u.cached_tokens).unwrap_or(0),
                    resp.usage.as_ref().map(|u| u.reasoning_tokens).unwrap_or(0),
                    resp.usage.as_ref().map(|u| u.cost).unwrap_or(0.0),
                    Some(duration_ms),
                    "success",
                    None,
                    None,
                );

                // Try to parse JSON from the response
                let content = resp.message.content.trim().to_lowercase();

                // Fallback heuristic if JSON parsing fails
                let is_coherent =
                    !content.contains("not coherent") && !content.contains("incoherent");
                let is_complete =
                    !content.contains("not complete") && !content.contains("incomplete");

                // Try structured JSON parsing
                let parsed = serde_json::from_str::<serde_json::Value>(resp.message.content.trim());

                if let Ok(json) = parsed {
                    let coherent = json
                        .get("is_coherent")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(is_coherent);
                    let complete = json
                        .get("is_complete")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(is_complete);
                    let needs_clar = json
                        .get("needs_clarification")
                        .and_then(|v| v.as_bool())
                        .unwrap_or_else(|| {
                            content.contains("clarification") || content.contains("unclear")
                        });
                    let followup = json
                        .get("followup")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    return Ok(Reflection {
                        is_coherent: coherent,
                        is_complete: complete,
                        needs_clarification: if needs_clar {
                            Some("I may need more details. Could you clarify?".into())
                        } else {
                            None
                        },
                        suggested_followup: followup,
                    });
                }

                // Heuristic fallback
                Ok(Reflection {
                    is_coherent,
                    is_complete,
                    needs_clarification: if content.contains("clarification")
                        || content.contains("unclear")
                    {
                        Some("I may need more details. Could you clarify?".into())
                    } else {
                        None
                    },
                    suggested_followup: if !is_complete {
                        Some("Is there anything else you'd like to know?".into())
                    } else {
                        None
                    },
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                let _ = StatsRepo::record_request(
                    db,
                    CallKind::Chat,
                    &Uuid::new_v4().to_string(),
                    "default",
                    Some(profile_id),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0.0,
                    Some(duration_ms),
                    "error",
                    Some(&e.to_string()),
                    None,
                    None,
                )
                .await;
                self.save_last_call(
                    "default",
                    None,
                    None,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0.0,
                    Some(duration_ms),
                    "error",
                    Some(&e.to_string()),
                    None,
                );
                Err(AgentError::LLMError(e.to_string()))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::decisions::{DecisionsProvider, DecisionsRequest, DecisionsResponse};
    use crate::llm::provider::{
        ChatResponse, LLMError, ReasoningEffort, ReasoningSpec, StreamEvent, TokenUsage,
    };
    use crate::tools::permission::Permission;
    use crate::tools::r#trait::{Tool, ToolError, ToolResult};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tokio_stream::Stream;

    // --- SSE event serialization tests (legacy, must keep passing) -----------

    #[test]
    fn test_chunk_serialization() {
        let event = SSEEvent::Chunk {
            content: "Hello".to_string(),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"chunk""#));
        assert!(json.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_done_serialization() {
        let event = SSEEvent::Done {
            message_id: "msg-123".to_string(),
            user_message_id: "msg-user-123".to_string(),
            location: None,
            tools_used: None,
            user_location: None,
            user_created_at: String::new(),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"done""#));
        assert!(json.contains(r#""message_id":"msg-123""#));
        assert!(json.contains(r#""user_message_id":"msg-user-123""#));
    }

    #[test]
    fn test_done_serialization_with_user_message_id() {
        let event = SSEEvent::Done {
            message_id: "msg-1".to_string(),
            user_message_id: "user-1".to_string(),
            location: None,
            tools_used: None,
            user_location: None,
            user_created_at: String::new(),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"done""#));
        assert!(json.contains(r#""message_id":"msg-1""#));
        assert!(json.contains(r#""user_message_id":"user-1""#));
    }

    #[test]
    fn test_approval_required_serialization() {
        let event = SSEEvent::ApprovalRequired {
            request_id: "req-1".to_string(),
            tool_name: "delete_file".to_string(),
            reason: "Requires explicit approval".to_string(),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"approval_required""#));
        assert!(json.contains(r#""tool_name":"delete_file""#));
    }

    #[test]
    fn test_error_serialization() {
        let event = SSEEvent::Error {
            message: "Something went wrong".to_string(),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains(r#""message":"Something went wrong""#));
    }

    #[test]
    fn test_tool_call_serialization() {
        let event = SSEEvent::ToolCall {
            name: "get_weather".to_string(),
            args: serde_json::json!({"city": "Madrid"}),
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"tool_call""#));
        assert!(json.contains(r#""name":"get_weather""#));
        assert!(json.contains(r#""city":"Madrid""#));
    }

    #[test]
    fn test_tool_result_serialization() {
        let event = SSEEvent::ToolResult {
            name: "get_weather".to_string(),
            success: true,
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"tool_result""#));
        assert!(json.contains(r#""name":"get_weather""#));
        assert!(json.contains(r#""success":true"#));
    }

    #[test]
    fn test_approval_result_serialization() {
        let event = SSEEvent::ApprovalResult {
            request_id: "req-1".to_string(),
            approved: true,
        };
        let json = event.to_json_string();
        assert!(json.contains(r#""type":"approval_result""#));
        assert!(json.contains(r#""request_id":"req-1""#));
        assert!(json.contains(r#""approved":true"#));
    }

    // --- New Orchestrator / AgentError / config tests ------------------------

    #[test]
    fn test_orchestrator_config_defaults() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_tokens_per_turn, 4096);
        assert!(config.enable_reflection);
    }

    #[test]
    fn test_default_system_prompt_fallback_is_minimal_and_non_empty() {
        // The hardcoded personality template was removed: the fallback must be
        // a minimal generic prompt.
        assert!(!DEFAULT_SYSTEM_PROMPT_FALLBACK.is_empty());
        assert_eq!(
            DEFAULT_SYSTEM_PROMPT_FALLBACK,
            "You are Valet, a helpful AI assistant."
        );
    }

    /// Build an orchestrator backed by a DB and a mock LLM that captures the
    /// first system message of every request.
    async fn build_orchestrator_with_capture(
        pool: SqlitePool,
        captured: Arc<Mutex<Option<String>>>,
        enable_reflection: bool,
    ) -> Orchestrator {
        let llm = Arc::new(SystemPromptCaptureLLM { captured });
        let registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig {
            enable_reflection,
            ..Default::default()
        };
        Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        )
    }

    #[tokio::test]
    async fn test_system_prompt_fallback_when_missing_in_stream(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        // Remove the value seeded by the migration to force the fallback.
        crate::db::repos::settings::SettingsRepo::delete(&pool, "system_prompt").await?;
        omit_user_name_section(&pool).await;

        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let orchestrator =
            build_orchestrator_with_capture(pool.clone(), captured.clone(), false).await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        assert_eq!(
            captured.lock().unwrap().as_deref(),
            Some(DEFAULT_SYSTEM_PROMPT_FALLBACK),
            "When settings.system_prompt is missing the minimal fallback must be used"
        );

        Ok(())
    }

    // ─── unified-system-message block 1: characterisation of the OLD contract ─
    //
    // The request used to open with THREE separate `role:"system"` messages,
    // in this exact order, before the conversation history:
    //   1. the prompt (`settings.system_prompt`),
    //   2. the episodic-memory section (`<episodic_memory>`), and
    //   3. the browser context (date, time, location).
    //
    // Two explicit exceptions are declared on purpose:
    //   * the reverse-geocoding branch (browser context WITHOUT `location_name`)
    //     is not reproduced here because it would hit the network; a known
    //     `location_name` is supplied instead, and
    //   * the reserved persistent-memory slot did not exist yet.
    //
    // The `unified-system-message` change broke that contract: blocks 2 and 3
    // merged all three sections into ONE single system message, in the order
    // prompt → episodic → browser. This characterisation documents the final
    // contract; the intermediate two-message state was captured in block 2.
    #[tokio::test]
    async fn characterization_single_system_message() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let browser = BrowserContext {
            timestamp: "2026-09-26T06:23:55.149Z".into(),
            timezone: "Europe/Madrid".into(),
            latitude: Some(40.4168),
            longitude: Some(-3.7038),
            location_name: Some("Madrid".into()),
        };

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", Some(browser), tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured
            .lock()
            .unwrap()
            .clone()
            .expect("LLM must have been called once");
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();

        assert_eq!(
            system_messages.len(),
            1,
            "the request must open with exactly one system message"
        );
        let content = &system_messages[0].content;
        let episodic_pos = content.find(EPISODIC_MEMORY_SECTION_TITLE).unwrap();
        let location_pos = content.find("Ubicación: Madrid").unwrap();
        assert!(
            episodic_pos < location_pos,
            "episodic precedes the browser section"
        );
        assert!(content.ends_with("Ubicación: Madrid (40.4168, -3.7038)."));

        Ok(())
    }

    // ─── unified-system-message block 2: one single system message ───────────

    /// 2.1 — the request must open with exactly ONE `role:"system"` message
    /// before the history, starting with the prompt and continuing with the
    /// episodic section when there are cards.
    #[tokio::test]
    async fn single_system_message_merges_prompt_and_episodic(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        let prompt = "PROMPT_DE_PRUEBA_UNICO";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;
        // A previous turn, so the conversation history is present too.
        crate::db::repos::messages::MessagesRepo::create(
            &pool,
            "user",
            "previous turn",
            None,
            None,
            None,
            None,
            100_000,
            None,
        )
        .await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured
            .lock()
            .unwrap()
            .clone()
            .expect("LLM must have been called once");
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();

        assert_eq!(
            system_messages.len(),
            1,
            "there must be exactly one system message before the history"
        );
        let content = &system_messages[0].content;
        let prompt_pos = content.find(prompt).expect("prompt must be present");
        let episodic_pos = content
            .find(EPISODIC_MEMORY_SECTION_TITLE)
            .expect("episodic section must be present");
        assert!(
            prompt_pos < episodic_pos,
            "the prompt must precede the episodic section"
        );
        assert!(content.contains("<episodic_memory>"));

        let system_idx = messages
            .iter()
            .position(|m| m.role == "system")
            .expect("system message present");
        let history_idx = messages
            .iter()
            .position(|m| m.content == "previous turn")
            .expect("history present");
        assert!(
            system_idx < history_idx,
            "the system message must precede the history"
        );

        Ok(())
    }

    /// 2.3 — without cards the episodic section is omitted entirely: the single
    /// system message is *exactly* the prompt, with no tags and no filler.
    #[tokio::test]
    async fn single_system_message_omits_episodic_without_cards(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        omit_user_name_section(&pool).await;
        // No memory seeded on purpose.
        let prompt = "PROMPT_SIN_FICHAS";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(system_messages.len(), 1);
        assert_eq!(
            system_messages[0].content, prompt,
            "without cards the single system message must be exactly the prompt"
        );
        assert!(!system_messages[0].content.contains("<episodic_memory>"));

        Ok(())
    }

    // ─── unified-system-message block 3: date/time/location closes the message ─

    /// 3.1 — with a `BrowserContext`, the date/time/location section is the
    /// last one of the single system message and keeps the current format.
    #[tokio::test]
    async fn single_system_message_browser_section_is_last(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        let prompt = "PROMPT_CON_CONTEXTO";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let browser = BrowserContext {
            timestamp: "2026-09-26T06:23:55.149Z".into(),
            timezone: "Europe/Madrid".into(),
            latitude: Some(40.4168),
            longitude: Some(-3.7038),
            location_name: Some("Madrid".into()),
        };

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", Some(browser), tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(
            system_messages.len(),
            1,
            "browser context must not add a second system message"
        );

        let content = &system_messages[0].content;
        let episodic_pos = content
            .find(EPISODIC_MEMORY_SECTION_TITLE)
            .expect("episodic section present");
        let location_pos = content
            .find("Ubicación: Madrid (40.4168, -3.7038).")
            .expect("location present");
        assert!(
            episodic_pos < location_pos,
            "the browser section must come after the episodic one"
        );
        assert!(
            content.ends_with("Ubicación: Madrid (40.4168, -3.7038)."),
            "the browser section must be the last one, got: {content:?}"
        );
        assert!(
            content.contains("8:23"),
            "the formatted local time must be present, got: {content:?}"
        );

        Ok(())
    }

    /// 3.3 — without a `BrowserContext` there is no date/time/location section:
    /// the message is exactly the prompt and carries no location or coordinates.
    #[tokio::test]
    async fn single_system_message_omits_browser_section_without_context(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        omit_user_name_section(&pool).await;
        let prompt = "PROMPT_SIN_CONTEXTO";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(system_messages.len(), 1);
        let content = &system_messages[0].content;
        assert_eq!(
            content, prompt,
            "without context the message is exactly the prompt"
        );
        assert!(!content.contains("Ubicación:"));
        assert!(!content.contains("Coordenadas:"));

        Ok(())
    }

    // ─── unified-system-message: persistent-memory section (Capa C) ──────────

    /// The composer accepts an OPTIONAL persistent-memory section placed
    /// between the prompt and the episodic one. When it is absent (`None`) it
    /// emits nothing: no title, marker, comment, separator or stray blank line.
    #[test]
    fn compose_system_message_places_persistent_section_between_prompt_and_episodic() {
        // Absent section: prompt + episodic only, no trace of a placeholder.
        let empty =
            compose_system_message("PROMPT", None, None, Some("EPISODIC".to_string()), None);
        assert_eq!(empty, "PROMPT\n\nEPISODIC");

        // A present section lands exactly between prompt and episodic.
        let filled = compose_system_message(
            "PROMPT",
            None,
            Some("PERSISTENT".to_string()),
            Some("EPISODIC".to_string()),
            Some("BROWSER".to_string()),
        );
        assert_eq!(filled, "PROMPT\n\nPERSISTENT\n\nEPISODIC\n\nBROWSER");
    }

    /// Request level — with no persistent state, the assembled request carries
    /// no persistent-memory section at all: the prompt and the episodic section
    /// are adjacent, with no marker between them.
    #[tokio::test]
    async fn request_without_persistent_state_has_no_section(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        omit_user_name_section(&pool).await;
        let prompt = "PROMPT_SIN_ESTADO";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(system_messages.len(), 1);
        let content = &system_messages[0].content;
        assert!(
            content.starts_with(&format!("{prompt}\n\n{EPISODIC_MEMORY_SECTION_TITLE}")),
            "no persistent-memory text may sit between the prompt and the episodic \
             section, got: {content:?}"
        );

        Ok(())
    }

    /// Full order with prompt, cards and browser context (no persistent state):
    /// prompt → episodic → date/time/location.
    #[tokio::test]
    async fn single_system_message_full_section_order() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        omit_user_name_section(&pool).await;
        let prompt = "PROMPT_ORDEN";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let browser = BrowserContext {
            timestamp: "2026-09-26T06:23:55.149Z".into(),
            timezone: "Europe/Madrid".into(),
            latitude: Some(40.4168),
            longitude: Some(-3.7038),
            location_name: Some("Madrid".into()),
        };

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", Some(browser), tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(system_messages.len(), 1);
        let content = &system_messages[0].content;

        let prompt_pos = content.find(prompt).expect("prompt present");
        let episodic_pos = content
            .find(EPISODIC_MEMORY_SECTION_TITLE)
            .expect("episodic present");
        let browser_pos = content
            .find("Ubicación: Madrid")
            .expect("browser section present");
        assert!(prompt_pos < episodic_pos, "prompt before episodic");
        assert!(episodic_pos < browser_pos, "episodic before browser");
        assert!(content.ends_with("Ubicación: Madrid (40.4168, -3.7038)."));
        assert!(
            content.starts_with(&format!("{prompt}\n\n{EPISODIC_MEMORY_SECTION_TITLE}")),
            "the absent persistent section leaves no text between prompt and episodic"
        );

        Ok(())
    }

    // ─── Capa C: sección de memoria persistente (Bloque 6) ─────────────────

    /// The composer minifies the stored payload and omits empty/unparseable
    /// states entirely.
    #[test]
    fn compose_persistent_memory_block_minifies_and_omits_empty() {
        let non_empty = compose_persistent_memory_block(
            r#"{ "schema_version": 1, "user_profile": {"city": "Madrid"} }"#,
        )
        .expect("a non-empty state must produce a section");
        assert!(
            non_empty.starts_with(PERSISTENT_MEMORY_SECTION_TITLE),
            "the section starts with its title, got {non_empty:?}"
        );
        assert!(
            non_empty.contains(PERSISTENT_MEMORY_INSTRUCTION),
            "the section states its purpose, got {non_empty:?}"
        );
        assert!(
            non_empty.contains(r#""user_profile":{"city":"Madrid"}"#),
            "the payload must be minified, got {non_empty:?}"
        );

        // Empty states leave no trace.
        assert!(compose_persistent_memory_block(r#"{"schema_version":1}"#).is_none());
        assert!(compose_persistent_memory_block(
            r#"{"schema_version":1,"user_profile":{},"system_rules":[]}"#
        )
        .is_none());

        // Unparseable payloads leave no trace.
        assert!(compose_persistent_memory_block("not json").is_none());
    }

    /// A non-empty state is injected between the prompt and the episodic
    /// section, as minified JSON under a short header.
    #[tokio::test]
    async fn persistent_memory_section_is_injected_between_prompt_and_episodic(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        seed_persistent_state(
            &pool,
            r#"{"schema_version":1,"user_profile":{"city":"Madrid"}}"#,
        )
        .await;

        let prompt = "PROMPT_PERSIST";
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        assert_eq!(system_messages.len(), 1);
        let content = &system_messages[0].content;

        let prompt_pos = content.find(prompt).expect("prompt present");
        let persistent_pos = content
            .find(PERSISTENT_MEMORY_SECTION_TITLE)
            .expect("persistent section present");
        let episodic_pos = content
            .find(EPISODIC_MEMORY_SECTION_TITLE)
            .expect("episodic section present");
        assert!(prompt_pos < persistent_pos, "prompt before persistent");
        assert!(
            persistent_pos < episodic_pos,
            "persistent section between the prompt and the episodic one"
        );
        assert!(
            content.contains(r#""user_profile":{"city":"Madrid"}"#),
            "the minified payload must be present, got {content:?}"
        );

        Ok(())
    }

    /// An empty stored state leaves no trace: no header, no marker, no blank
    /// line between the prompt and the episodic section.
    #[tokio::test]
    async fn empty_persistent_state_leaves_no_trace() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        seed_persistent_state(
            &pool,
            r#"{"schema_version":1,"user_profile":{},"system_rules":[]}"#,
        )
        .await;

        let prompt = "PROMPT_EMPTY_PERSIST";
        omit_user_name_section(&pool).await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", prompt).await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let system_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "system").collect();
        let content = &system_messages[0].content;

        assert!(
            !content.contains(PERSISTENT_MEMORY_SECTION_TITLE),
            "an empty state must not inject the persistent header"
        );
        assert!(
            content.starts_with(&format!("{prompt}\n\n{EPISODIC_MEMORY_SECTION_TITLE}")),
            "an empty state leaves no trace between prompt and episodic, got {content:?}"
        );

        Ok(())
    }

    // ─── Block 8: automatic memory injection into the LLM request ───────────
    //
    // The block-1 characterization tests pinned the OLD behaviour: memory was
    // injected as a `system` message prefixed with the literal
    // `"[Memory context] "` and only on the `RAG` strategy. Block 8
    // deliberately breaks both halves of that contract:
    //   * memory is now retrieved for every strategy (block 8.6), so the
    //     `SlidingWindow` path (`Hola`, no override) receives memory too, and
    //   * the literal is replaced by a code-composed `<episodic_memory>` block
    //     inside `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)` (blocks 8.3–8.5).
    //
    // The four obsolete tests (`characterization_rag_injects_memory_context_in_*`
    // and `characterization_sliding_window_does_not_inject_memory_in_*`) were
    // replaced by the tests below: their assertions (`[Memory context]` prefix
    // present on RAG / memory absent on SlidingWindow) are exactly what this
    // block changes, so they are obsolete by construction.

    /// Extract the episodic-memory block (`system` message carrying the
    /// `<episodic_memory>` tags) from a captured request, if any.
    fn extract_episodic_block(messages: &[ChatMessage]) -> Option<String> {
        messages
            .iter()
            .find(|m| m.role == "system" && m.content.contains("<episodic_memory>"))
            .map(|m| m.content.clone())
    }

    /// 8.4 — the same holds for the streaming path, and it must be the SAME
    /// block (identical format) that `compose_episodic_memory_block` produces.
    #[tokio::test]
    async fn injects_episodic_memory_block_in_stream() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "what does the user like", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured
            .lock()
            .unwrap()
            .clone()
            .expect("LLM must have been called once");
        let block = extract_episodic_block(&messages).expect("episodic block must be injected");
        assert!(block.contains("# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)"));
        assert!(block.contains("<episodic_memory>"));
        assert!(block.contains("</episodic_memory>"));
        assert!(block.contains("User likes Rust"));
        assert!(
            !messages
                .iter()
                .any(|m| m.content.contains("[Memory context]")),
            "the old `[Memory context]` literal must be gone (8.4)"
        );
        Ok(())
    }

    /// 8.5 — the `<episodic_memory>` block appears BEFORE the first message of
    /// the conversation history, so the reminder is never read as the current
    /// turn. Plain `Hola` (`SlidingWindow`, no override) also proves that the
    /// common path receives memory.
    #[tokio::test]
    async fn episodic_memory_block_precedes_history() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        seed_one_memory(&pool).await;
        // A previous turn, loaded into the request by `list_by_token_budget`.
        crate::db::repos::messages::MessagesRepo::create(
            &pool,
            "user",
            "previous turn",
            None,
            None,
            None,
            None,
            100_000,
            None,
        )
        .await?;

        let captured: Arc<Mutex<Option<Vec<ChatMessage>>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_with_full_capture(
            pool.clone(),
            captured.clone(),
            memory_context_builder(pool.clone()),
        )
        .await;
        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await?;
        while rx.recv().await.is_some() {}

        let messages = captured.lock().unwrap().clone().unwrap();
        let block_idx = messages
            .iter()
            .position(|m| m.content.contains("<episodic_memory>"))
            .expect("episodic block must be present");
        let history_idx = messages
            .iter()
            .position(|m| m.content == "previous turn")
            .expect("conversation history must be present");
        assert!(
            block_idx < history_idx,
            "the episodic block (idx {block_idx}) must precede the history (idx {history_idx})"
        );
        Ok(())
    }

    /// Mock LLM that captures the full message list of the *first* request.
    struct FullRequestCaptureLLM {
        captured: Arc<Mutex<Option<Vec<ChatMessage>>>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for FullRequestCaptureLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            {
                let mut slot = self.captured.lock().unwrap();
                if slot.is_none() {
                    *slot = Some(request.messages.clone());
                }
            }
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "OK.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            {
                let mut slot = self.captured.lock().unwrap();
                if slot.is_none() {
                    *slot = Some(request.messages.clone());
                }
            }
            let events: Vec<Result<StreamEvent, LLMError>> =
                vec![Ok(StreamEvent::Done(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "OK.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                }))];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// Fixed embedding provider (dimension 1024, as `vec0` declares),
    /// matching the seeded vector.
    struct FixedEmbedProvider;

    /// Pad a leading slice to the 1024 dimensions the `vec0` table declares.
    fn v1024(leading: &[f32]) -> Vec<f32> {
        let mut v = leading.to_vec();
        v.resize(1024, 0.0);
        v
    }

    #[async_trait::async_trait]
    impl crate::embeddings::EmbeddingProvider for FixedEmbedProvider {
        async fn embed(
            &self,
            _input: &str,
        ) -> Result<Vec<f32>, crate::embeddings::provider::EmbeddingError> {
            Ok(v1024(&[0.1, 0.2, 0.3]))
        }
    }

    /// Seed one `memory` card plus its `vec_memory` row.
    ///
    /// CHANGED ON PURPOSE (invariant exception 2): `vec_memory` now stores
    /// binary `vec0` vectors of the declared 1024 dimensions, so the row is
    /// written through `vec_f32(?)` instead of as JSON text. The
    /// characterization assertions are untouched.
    async fn seed_one_memory(pool: &SqlitePool) {
        let mem = crate::db::repos::memory::MemoryRepo::create(
            pool,
            "User likes Rust",
            10,
            &serde_json::json!({"tags": ["rust", "backend"]}),
        )
        .await
        .expect("create memory should succeed");
        let json = serde_json::to_string(&v1024(&[0.1, 0.2, 0.3])).expect("serialize embedding");
        sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES (?1, vec_f32(?2))")
            .bind(&mem.id)
            .bind(&json)
            .execute(pool)
            .await
            .expect("insert vec_memory row");
    }

    /// Seed the Layer C `persistent_memory` global state with a raw payload.
    async fn seed_persistent_state(pool: &SqlitePool, payload: &str) {
        crate::db::repos::persistent_memory::PersistentMemoryRepo::upsert(
            pool,
            payload,
            "2026-10-01T10:00:00Z",
        )
        .await
        .expect("upsert persistent memory");
    }

    /// Builder wired with pool + provider and one seeded memory, so the RAG
    /// path can actually retrieve something.
    fn memory_context_builder(pool: SqlitePool) -> ContextBuilder {
        ContextBuilder {
            pool: Some(pool),
            provider: Some(Arc::new(FixedEmbedProvider)),
            rag_budget_tokens: 2000,
        }
    }

    /// Orchestrator with a full-request capturer and no reflection, so the
    /// first (only) main LLM call is the one observed.
    async fn build_orchestrator_with_full_capture(
        pool: SqlitePool,
        captured: Arc<Mutex<Option<Vec<ChatMessage>>>>,
        context_builder: ContextBuilder,
    ) -> Orchestrator {
        let llm = Arc::new(FullRequestCaptureLLM { captured });
        let registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let config = OrchestratorConfig {
            enable_reflection: false,
            ..Default::default()
        };
        Orchestrator::new(
            llm,
            registry,
            guardrails,
            Arc::new(context_builder),
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        )
    }

    /// Mock LLM that captures the full `ChatRequest` of the first chat call.
    struct FullChatRequestCaptureLLM {
        captured: Arc<Mutex<Option<ChatRequest>>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for FullChatRequestCaptureLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            {
                let mut slot = self.captured.lock().unwrap();
                if slot.is_none() {
                    *slot = Some(request.clone());
                }
            }
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "OK.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            {
                let mut slot = self.captured.lock().unwrap();
                if slot.is_none() {
                    *slot = Some(request);
                }
            }
            let events: Vec<Result<StreamEvent, LLMError>> =
                vec![Ok(StreamEvent::Done(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "OK.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                }))];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// Orchestrator whose mock LLM captures the full chat request.
    async fn build_orchestrator_capturing_chat_request(
        pool: SqlitePool,
        captured: Arc<Mutex<Option<ChatRequest>>>,
    ) -> Orchestrator {
        let llm = Arc::new(FullChatRequestCaptureLLM { captured });
        let registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig {
            enable_reflection: false,
            ..Default::default()
        };
        Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        )
    }

    /// Run `process_message_stream` once and return the captured chat request.
    async fn captured_chat_request(pool: SqlitePool) -> ChatRequest {
        let captured: Arc<Mutex<Option<ChatRequest>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_orchestrator_capturing_chat_request(pool, captured.clone()).await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await
            .expect("process_message_stream must succeed");
        while rx.recv().await.is_some() {}

        let request = captured
            .lock()
            .unwrap()
            .clone()
            .expect("the chat request must have been captured");
        request
    }

    #[test]
    fn test_agent_error_display_llm() {
        let err = AgentError::LLMError("rate limited".into());
        assert!(err.to_string().contains("rate limited"));
    }

    #[test]
    fn test_agent_error_display_tool() {
        let err = AgentError::ToolError("not found".into());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_agent_error_display_guardrail() {
        let err = AgentError::GuardrailError("denied".into());
        assert!(err.to_string().contains("denied"));
    }

    #[test]
    fn test_agent_error_display_context() {
        let err = AgentError::ContextError("profile missing".into());
        assert!(err.to_string().contains("profile missing"));
    }

    #[test]
    fn test_agent_error_display_max_iterations() {
        let err = AgentError::MaxIterationsExceeded;
        assert_eq!(err.to_string(), "Max iterations exceeded");
    }

    #[test]
    fn test_agent_error_display_internal() {
        let err = AgentError::Internal("something broke".into());
        assert!(err.to_string().contains("something broke"));
    }

    #[test]
    fn test_agent_error_impl_debug_and_clone() {
        let err = AgentError::LLMError("oops".into());
        let cloned = err.clone();
        assert!(format!("{:?}", cloned).contains("oops"));
    }

    #[test]
    fn test_tool_call_info_construction() {
        let info = ToolCallInfo {
            name: "search".into(),
            arguments: serde_json::json!({"q": "test"}),
            result: Some(serde_json::json!({"results": []})),
        };
        assert_eq!(info.name, "search");
        assert!(info.result.is_some());
    }

    #[test]
    fn test_reflection_defaults() {
        let r = Reflection {
            is_coherent: true,
            is_complete: false,
            needs_clarification: Some("Please clarify".into()),
            suggested_followup: None,
        };
        assert!(r.is_coherent);
        assert!(!r.is_complete);
        assert!(r.needs_clarification.is_some());
        assert!(r.suggested_followup.is_none());
    }

    // -----------------------------------------------------------------------
    // Mock LLM that returns a tool call on first invocation, then plain text
    // -----------------------------------------------------------------------

    struct MockLLMWithToolThenAnswer {
        call_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMWithToolThenAnswer {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == 1 {
                // First call: return a tool call that the orchestrator will execute
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Let me check the weather.".into(),
                        tool_calls: Some(vec![ToolCall {
                            id: "call-1".into(),
                            name: "weather".into(),
                            arguments: serde_json::json!({"latitude": 40.4168, "longitude": -3.7038}),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                // Second call: return final answer without tool calls
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "The weather in Madrid is 22°C and sunny.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let content = result.message.content.clone();
            let tool_calls = result.message.tool_calls.clone();

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();

            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            } else {
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            }

            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    /// Dummy tool that always succeeds — used to exercise the tool tracking code.
    struct MockWeatherTool;

    #[async_trait::async_trait]
    impl Tool for MockWeatherTool {
        fn name(&self) -> &'static str {
            "weather"
        }

        fn description(&self) -> &'static str {
            "Get weather forecast"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({"temperature": 22.0, "description": "sunny"}),
                message: None,
            })
        }
    }

    // -----------------------------------------------------------------------
    // Streaming integration test — verifies tool footer is added
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_process_message_stream_adds_tool_footer() -> Result<(), Box<dyn std::error::Error>>
    {
        use crate::db::repos::messages::MessagesRepo;

        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create a registry with a mock weather tool
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockWeatherTool));
        let registry = Arc::new(registry);

        // 3. Create mock LLM that returns tool call then answer
        let llm = Arc::new(MockLLMWithToolThenAnswer {
            call_count: Arc::new(Mutex::new(0)),
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        // 4. Call process_message_stream (no conversation_id needed)
        let (tx, _rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-id", "What's the weather?", None, tx)
            .await?;

        // 5. Query the DB for the assistant message and verify footer
        let (messages, _) = MessagesRepo::list_all(&pool, 100, None).await?;
        let assistant_messages: Vec<_> =
            messages.iter().filter(|m| m.role == "assistant").collect();

        assert!(
            !assistant_messages.is_empty(),
            "Expected at least one assistant message in DB"
        );

        let last_msg = assistant_messages.last().unwrap();
        assert!(
            last_msg.tools_used.is_some(),
            "Assistant message should have tools_used set, got: {:?}",
            last_msg.tools_used
        );
        assert!(
            last_msg.tools_used.as_deref().unwrap().contains("weather"),
            "tools_used should contain 'weather', got: {:?}",
            last_msg.tools_used
        );
        assert!(
            last_msg
                .content
                .contains("The weather in Madrid is 22°C and sunny."),
            "Assistant message should contain the original content, got: {}",
            last_msg.content
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // A successful `render_widget` call must emit an SSE `widget` event
    // -----------------------------------------------------------------------

    /// Mock LLM that asks to render a widget on its first call, then answers.
    struct MockLLMWithWidgetThenAnswer {
        call_count: Arc<Mutex<usize>>,
        widget_name: String,
        widget_data: Value,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMWithWidgetThenAnswer {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == 1 {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Voy a pedir un widget.".into(),
                        tool_calls: Some(vec![ToolCall {
                            id: "call-widget-1".into(),
                            name: "render_widget".into(),
                            arguments: serde_json::json!({
                                "widget_name": self.widget_name.clone(),
                                "data": self.widget_data.clone(),
                            }),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Aquí tienes.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let content = result.message.content.clone();
            let tool_calls = result.message.tool_calls.clone();

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            } else {
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            }

            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// Configurable stand-in for `RenderWidgetTool` used to drive the orchestrator.
    struct MockRenderWidgetTool {
        allow: bool,
    }

    #[async_trait::async_trait]
    impl Tool for MockRenderWidgetTool {
        fn name(&self) -> &'static str {
            RENDER_WIDGET_TOOL_NAME
        }

        fn description(&self) -> &'static str {
            "mock render_widget"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
            if self.allow {
                let widget_name = args
                    .get("widget_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                // Mirror the production tool: normalise `data` and report it back.
                let normalized_data = args
                    .get("data")
                    .filter(|v| v.is_object())
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                Ok(ToolResult {
                    success: true,
                    data: serde_json::json!({
                        "rendered": true,
                        "widget_name": widget_name,
                        "data": normalized_data,
                    }),
                    message: Some("Widget requested".into()),
                })
            } else {
                Err(ToolError::InvalidArguments(
                    "widget_name not allowed".into(),
                ))
            }
        }
    }

    /// Run one streaming turn against the mock widget tool and collect every
    /// event together with the pool the turn was persisted into.
    async fn run_widget_turn(
        widget_name: &str,
        widget_data: Value,
        allow: bool,
    ) -> Result<(SqlitePool, Vec<SSEEvent>), Box<dyn std::error::Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockRenderWidgetTool { allow }));
        let registry = Arc::new(registry);

        let llm = Arc::new(MockLLMWithWidgetThenAnswer {
            call_count: Arc::new(Mutex::new(0)),
            widget_name: widget_name.to_string(),
            widget_data,
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            OrchestratorConfig::default(),
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-id", "Quiero un widget", None, tx)
            .await?;

        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        Ok((pool, events))
    }

    fn is_widget_event(event: &SSEEvent) -> bool {
        matches!(event, SSEEvent::Widget { .. })
    }

    fn is_done_event(event: &SSEEvent) -> bool {
        matches!(event, SSEEvent::Done { .. })
    }

    #[tokio::test]
    async fn test_render_widget_success_emits_widget_event(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::json!({"title": "Elige ciudad", "fields": []});
        let (_pool, events) = run_widget_turn("QuickForm", data.clone(), true).await?;

        let widget = events
            .iter()
            .find(|e| is_widget_event(e))
            .expect("a successful render_widget call must emit a `widget` event");

        match widget {
            SSEEvent::Widget {
                id,
                name,
                data: emitted,
            } => {
                assert!(!id.is_empty(), "the widget id must not be empty");
                assert_eq!(name, "QuickForm");
                assert_eq!(
                    emitted, &data,
                    "the widget data must be passed through intact"
                );
                assert!(
                    widget.to_json_string().contains(r#""type":"widget""#),
                    "the event must serialize with type=widget"
                );
            }
            other => panic!("expected a widget event, got {other:?}"),
        }

        assert!(
            events.iter().any(is_done_event),
            "the turn must still end with a `done` event"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_render_widget_invalid_name_does_not_emit_widget_event(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Positive control: the same harness with a permitted widget must emit a
        // `widget` event, proving the negative case below is meaningful.
        let (_pool, control) = run_widget_turn("QuickForm", serde_json::json!({}), true).await?;
        assert!(
            control.iter().any(is_widget_event),
            "control: a permitted widget must emit a `widget` event"
        );

        // Negative case: an invalid widget name must not emit a `widget` event.
        let (_pool, events) =
            run_widget_turn("SystemMonitor", serde_json::json!({}), false).await?;
        assert!(
            !events.iter().any(is_widget_event),
            "an invalid widget name must not emit a `widget` event"
        );
        assert!(
            events.iter().any(is_done_event),
            "the turn must continue and end with `done` even when the widget is invalid"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_render_widget_non_object_data_emits_empty_object(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Non-object `data` must be normalised to `{}` before it reaches the client.
        let (_pool, events) =
            run_widget_turn("QuickForm", serde_json::json!("not-an-object"), true).await?;

        let widget = events
            .iter()
            .find(|e| is_widget_event(e))
            .expect("a successful render_widget call must emit a `widget` event");

        match widget {
            SSEEvent::Widget { data, .. } => {
                assert_eq!(
                    data,
                    &serde_json::json!({}),
                    "non-object data must reach the client as an empty object"
                );
            }
            other => panic!("expected a widget event, got {other:?}"),
        }

        assert!(
            events.iter().any(is_done_event),
            "the turn must still end with a `done` event"
        );
        Ok(())
    }

    /// A turn that succeeds in `render_widget` persists the widget list on the
    /// assistant message, reusing the id, name and data of the emitted event.
    #[tokio::test]
    async fn test_render_widget_persists_widgets_on_assistant_message(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::db::repos::messages::MessagesRepo;

        let data = serde_json::json!({"title": "Elige ciudad", "fields": []});
        let (pool, events) = run_widget_turn("QuickForm", data.clone(), true).await?;

        let (event_id, event_name, event_data) = events
            .iter()
            .find_map(|e| match e {
                SSEEvent::Widget { id, name, data } => {
                    Some((id.clone(), name.clone(), data.clone()))
                }
                _ => None,
            })
            .expect("a successful render_widget call must emit a `widget` event");

        let (messages, _) = MessagesRepo::list_all(&pool, 100, None).await?;
        let assistant = messages
            .iter()
            .find(|m| m.role == "assistant")
            .expect("the turn must persist an assistant message");

        let widgets = assistant
            .widgets
            .as_ref()
            .expect("the assistant message must have widgets persisted");
        let widgets = widgets
            .as_array()
            .expect("the persisted widgets must be a JSON array");
        assert_eq!(widgets.len(), 1, "exactly one widget must be persisted");
        assert_eq!(widgets[0]["id"], event_id, "the widget id must be reused");
        assert_eq!(widgets[0]["name"], event_name);
        assert_eq!(widgets[0]["data"], event_data);

        Ok(())
    }

    /// A turn without a successful `render_widget` leaves `widgets` as `None`.
    #[tokio::test]
    async fn test_turn_without_widget_leaves_widgets_null() -> Result<(), Box<dyn std::error::Error>>
    {
        use crate::db::repos::messages::MessagesRepo;

        let (pool, events) = run_widget_turn("SystemMonitor", serde_json::json!({}), false).await?;
        assert!(
            !events.iter().any(is_widget_event),
            "an invalid widget must not emit a `widget` event"
        );

        let (messages, _) = MessagesRepo::list_all(&pool, 100, None).await?;
        let assistant = messages
            .iter()
            .find(|m| m.role == "assistant")
            .expect("the turn must persist an assistant message");
        assert!(
            assistant.widgets.is_none(),
            "a turn without widgets must leave `widgets` as None"
        );

        Ok(())
    }

    /// Given an Orchestrator processing a long user message (8000 chars),
    /// when the message is persisted via MessagesRepo::create(),
    /// then the collapse callback SHOULD fire and send the message_id through
    /// the collapse channel.
    ///
    /// RED: This test will fail because the orchestrator currently passes
    /// `None` as the `on_collapse_needed` callback to MessagesRepo::create(),
    /// so no message_id arrives on collapse_rx.
    #[tokio::test]
    async fn test_collapse_callback_fires_for_long_user_message(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create a simple mock LLM that returns plain text
        struct SimpleMockLLM;

        #[async_trait::async_trait]
        impl LLMProvider for SimpleMockLLM {
            async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "This is a simple response to a very long message.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }

            async fn chat_stream(
                &self,
                request: ChatRequest,
            ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
            {
                let result = self.chat(request).await?;
                let content = result.message.content.clone();
                let tool_calls = result.message.tool_calls.clone();

                let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();

                if let Some(tcs) = tool_calls {
                    for tc in tcs {
                        events.push(Ok(StreamEvent::ToolCall(tc)));
                    }
                    events.push(Ok(StreamEvent::Done(result)));
                } else {
                    for chunk in content
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(10)
                        .map(|c| c.iter().collect::<String>())
                    {
                        events.push(Ok(StreamEvent::Chunk(chunk)));
                    }
                    events.push(Ok(StreamEvent::Done(result)));
                }

                let stream = futures::stream::iter(events);
                Ok(Box::pin(stream))
            }
        }

        // 3. Create collapse channel that should receive the message_id
        let (collapse_tx, mut collapse_rx) = mpsc::channel::<String>(16);

        // 4. Create orchestrator
        let llm = Arc::new(SimpleMockLLM);
        let registry = Arc::new(ToolRegistry::new());
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            Some(collapse_tx),
            None,
            Arc::new(RwLock::new(None)),
        );

        // 5. Call process_message_stream with a very long message (2000+ words for ~2660 tokens)
        let long_msg = "x ".repeat(2000);
        let (tx, _rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-id", &long_msg, None, tx)
            .await?;

        // 6. Verify that the collapse channel received the message_id
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(500), collapse_rx.recv()).await;

        match received {
            Ok(Some(msg_id)) => {
                assert!(!msg_id.is_empty(), "message_id should not be empty");
            }
            _ => {
                panic!("Should have received message_id via collapse channel for long message");
            }
        }
        Ok(())
    }

    /// Given an Orchestrator with memory_tx wired,
    /// when process_message_stream persists a user message,
    /// then a signal (()) SHOULD be sent on memory_tx.
    ///
    /// RED: This test will fail because the orchestrator does not yet
    /// send signals through memory_tx after persisting messages.
    #[tokio::test]
    async fn test_orchestrator_sends_memory_signal_on_user_message(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create a local mock LLM that returns plain text immediately
        struct MemoryMockLLM;

        #[async_trait::async_trait]
        impl LLMProvider for MemoryMockLLM {
            async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Simple response for memory test.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }

            async fn chat_stream(
                &self,
                request: ChatRequest,
            ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
            {
                let result = self.chat(request).await?;
                let content = result.message.content.clone();

                let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));

                let stream = futures::stream::iter(events);
                Ok(Box::pin(stream))
            }
        }

        let llm = Arc::new(MemoryMockLLM);
        let registry = Arc::new(ToolRegistry::new());
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        // 3. Create memory_tx channel
        let (memory_tx, mut memory_rx) = mpsc::channel::<()>(16);

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            Some(memory_tx),
            Arc::new(RwLock::new(None)),
        );

        // 4. Call process_message_stream
        let (tx, _rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("test-profile", "Hello", None, tx)
            .await?;

        // 5. Verify that memory_rx receives at least one signal
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(500), memory_rx.recv()).await;

        match received {
            Ok(Some(())) => { /* expected: received signal */ }
            _ => {
                panic!("Should have received () via memory_tx after persisting user message");
            }
        }
        Ok(())
    }

    /// Given an Orchestrator with memory_tx wired,
    /// when process_message_stream persists both user and assistant messages,
    /// then TWO signals SHOULD be sent on memory_tx
    /// (one for the user message, one for the assistant response).
    ///
    /// RED: This test will fail because the orchestrator does not yet
    /// send signals through memory_tx after persisting messages.
    #[tokio::test]
    async fn test_orchestrator_sends_memory_signal_on_assistant_message(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create a local mock LLM that returns plain text immediately
        struct MemoryMockLLM;

        #[async_trait::async_trait]
        impl LLMProvider for MemoryMockLLM {
            async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Simple response for memory test.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }

            async fn chat_stream(
                &self,
                request: ChatRequest,
            ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
            {
                let result = self.chat(request).await?;
                let content = result.message.content.clone();

                let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));

                let stream = futures::stream::iter(events);
                Ok(Box::pin(stream))
            }
        }

        let llm = Arc::new(MemoryMockLLM);
        let registry = Arc::new(ToolRegistry::new());
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        // 3. Create memory_tx channel
        let (memory_tx, mut memory_rx) = mpsc::channel::<()>(16);

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            Some(memory_tx),
            Arc::new(RwLock::new(None)),
        );

        // 4. Call process_message_stream
        let (tx, _rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("test-profile", "Hello", None, tx)
            .await?;

        // 5. Verify that memory_rx receives at least TWO signals
        //    (user message + assistant response)
        let signal1 =
            tokio::time::timeout(std::time::Duration::from_millis(500), memory_rx.recv()).await;
        let signal2 =
            tokio::time::timeout(std::time::Duration::from_millis(500), memory_rx.recv()).await;

        match signal1 {
            Ok(Some(())) => { /* first signal received */ }
            _ => {
                panic!("Should have received first () via memory_tx (user message)");
            }
        }

        match signal2 {
            Ok(Some(())) => { /* second signal received */ }
            _ => {
                panic!("Should have received second () via memory_tx (assistant response)");
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Mock tool that always fails — used to test error recovery in ReAct loop
    // -----------------------------------------------------------------------

    struct MockFailingTool;

    #[async_trait::async_trait]
    impl Tool for MockFailingTool {
        fn name(&self) -> &'static str {
            "failing_tool"
        }

        fn description(&self) -> &'static str {
            "A tool that always fails"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            Err(ToolError::ExecutionError(
                "Overpass timeout simulated".into(),
            ))
        }
    }

    /// Mock LLM that returns a tool call for `failing_tool` on first
    /// invocation, then returns a plain-text answer on the second call.
    /// This exercises the scenario where a tool *errors* and the LLM
    /// should still get a chance to respond.
    struct MockLLMWithFailingToolThenAnswer {
        call_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMWithFailingToolThenAnswer {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == 1 {
                // First call: return a tool call for the failing tool
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Let me look that up.".into(),
                        tool_calls: Some(vec![ToolCall {
                            id: "call-fail-1".into(),
                            name: "failing_tool".into(),
                            arguments: serde_json::json!({"query": "test"}),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                // Second call: return final answer (LLM recovers from error)
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "The tool failed, but I can still help.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let content = result.message.content.clone();
            let tool_calls = result.message.tool_calls.clone();

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();

            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            } else {
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            }

            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    // -----------------------------------------------------------------------
    // Test: tool error recovery — currently RED because `?` breaks the loop
    // -----------------------------------------------------------------------
    //
    // Este test demuestra el comportamiento ACTUAL (roto): cuando un tool
    // falla con Err(ToolError), el `?` en la línea 670 propaga el error y
    // cortocircuita el ReAct loop, impidiendo que se emitan los eventos SSE
    // ToolResult { success: false }, Chunk y Done.
    //
    // El test captura los eventos SSE y verifica que se complete el flujo
    // completo (ToolCall -> ToolResult(success:false) -> Chunk -> Done).
    // Actualmente FALLA porque el error se propaga antes de emitir Done.
    //
    // RED: Este test falla → lo haremos pasar en GREEN.

    #[tokio::test]
    async fn test_tool_error_does_not_break_react_loop() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create a registry with a mock failing tool
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockFailingTool));
        let registry = Arc::new(registry);

        // 3. Create mock LLM that returns failing tool call then answer
        let llm = Arc::new(MockLLMWithFailingToolThenAnswer {
            call_count: Arc::new(Mutex::new(0)),
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        // 4. Call process_message_stream and capture SSE events
        let (tx, mut rx) = mpsc::channel(100);
        let result = orchestrator
            .process_message_stream("profile-id", "Look something up", None, tx)
            .await;

        // 5. Collect all SSE events with a timeout
        let mut events: Vec<SSEEvent> = Vec::new();
        while let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {
            events.push(event);
        }

        // 6. Verify the event sequence — this SHOULD work once the `?` is
        // replaced with a match that converts the error into a ToolResult.
        //
        // ACTUAL: This assertion fails because process_message_stream returns
        // Err(...) (the `?` propagates the ToolError) and no ToolResult event
        // is emitted for the failing tool.
        assert!(
            result.is_ok(),
            "El orquestador NO debe propagar errores de tool como errores del ReAct loop. \
             Error actual: {:?}",
            result.err()
        );

        // Verify the expected event sequence
        let tool_call_events: Vec<&SSEEvent> = events
            .iter()
            .filter(|e| matches!(e, SSEEvent::ToolCall { .. }))
            .collect();
        assert!(
            !tool_call_events.is_empty(),
            "Debe emitirse al menos un SSEEvent::ToolCall para failing_tool"
        );

        let tool_result_events: Vec<&SSEEvent> = events
            .iter()
            .filter(|e| matches!(e, SSEEvent::ToolResult { .. }))
            .collect();
        assert!(
            !tool_result_events.is_empty(),
            "Debe emitirse al menos un SSEEvent::ToolResult (incluyendo success: false)"
        );

        // Verify there is a ToolResult with success: false
        let has_failure = events.iter().any(|e| {
            matches!(
                e,
                SSEEvent::ToolResult {
                    name: _,
                    success: false
                }
            )
        });
        assert!(
            has_failure,
            "Debe haber un SSEEvent::ToolResult con success: false para el tool fallido"
        );

        // Verify Chunk events exist (LLM response after error)
        let chunk_events: Vec<&SSEEvent> = events
            .iter()
            .filter(|e| matches!(e, SSEEvent::Chunk { .. }))
            .collect();
        assert!(
            !chunk_events.is_empty(),
            "Debe emitirse SSEEvent::Chunk (el LLM responde incluso tras error del tool)"
        );

        // Verify Done event exists
        let done_events: Vec<&SSEEvent> = events
            .iter()
            .filter(|e| matches!(e, SSEEvent::Done { .. }))
            .collect();
        assert!(
            !done_events.is_empty(),
            "Debe emitirse SSEEvent::Done al completar el ReAct loop"
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Mock tool that counts calls — for the retry limit test
    // -----------------------------------------------------------------------

    struct MockLimitedTool {
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Tool for MockLimitedTool {
        fn name(&self) -> &'static str {
            "limited_tool"
        }

        fn description(&self) -> &'static str {
            "Tool that counts calls"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({"status": "ok"}),
                message: None,
            })
        }
    }

    // -----------------------------------------------------------------------
    // Mock LLM that calls limited_tool MAX_TOOL_RETRIES + 1 times,
    // then returns plain text on the next call.
    // -----------------------------------------------------------------------

    struct MockLLMExceedingRetries {
        call_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMExceedingRetries {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count <= MAX_TOOL_RETRIES + 1 {
                // Return a tool call for limited_tool
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Let me try...".into(),
                        tool_calls: Some(vec![ToolCall {
                            id: format!("call-{}", count),
                            name: "limited_tool".into(),
                            arguments: serde_json::json!({"input": count.to_string()}),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                // Final answer after exhausting retries
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Done after retries.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let content = result.message.content.clone();
            let tool_calls = result.message.tool_calls.clone();

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();

            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            } else {
                for chunk in content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(result)));
            }

            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    // -----------------------------------------------------------------------
    // Test: tool should not execute more than 3 times in the same ReAct loop
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_tool_max_retries_in_react_loop() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool and run migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Registry with MockLimitedTool
        let call_count = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockLimitedTool {
            call_count: call_count.clone(),
        }));
        let registry = Arc::new(registry);

        // 3. Mock LLM that calls limited_tool MAX_TOOL_RETRIES + 1 times
        let llm = Arc::new(MockLLMExceedingRetries {
            call_count: Arc::new(Mutex::new(0)),
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        let (tx, mut rx) = mpsc::channel(100);

        // 4. Execute
        let result = orchestrator
            .process_message_stream("profile-id", "test", None, tx)
            .await;

        // 5. Verify: orchestrator should complete successfully
        assert!(
            result.is_ok(),
            "Orchestrator should complete successfully, got: {:?}",
            result
        );

        // 6. Verify: tool should be called max MAX_TOOL_RETRIES times
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            MAX_TOOL_RETRIES,
            "Tool should be called max {} times, but was called {} times",
            MAX_TOOL_RETRIES,
            call_count.load(Ordering::SeqCst)
        );

        // 7. Verify SSE events: must have Done
        let mut got_done = false;
        while let Some(event) = rx.recv().await {
            if matches!(event, SSEEvent::Done { .. }) {
                got_done = true;
                break;
            }
        }
        assert!(got_done, "Should emit Done event");

        Ok(())
    }

    // -----------------------------------------------------------------------
    // T0.2: Orchestrator injects profile_id into tool calls
    // -----------------------------------------------------------------------

    /// Mock tool that records whether profile_id was injected.
    struct ProfileIdCaptureTool {
        profile_id_received: Arc<Mutex<bool>>,
    }

    #[async_trait::async_trait]
    impl Tool for ProfileIdCaptureTool {
        fn name(&self) -> &'static str {
            "capture_tool"
        }

        fn description(&self) -> &'static str {
            "Tool that captures profile_id"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, args: serde_json::Value) -> Result<ToolResult, ToolError> {
            let has_profile_id =
                args.get("profile_id").and_then(|v| v.as_str()) == Some("profile-id");
            *self.profile_id_received.lock().unwrap() = has_profile_id;
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({"ok": true}),
                message: None,
            })
        }
    }

    /// Mock LLM that returns a tool call on first invocation (without profile_id),
    /// then plain text on subsequent calls.
    struct MockLLMWithToolCallNoProfile {
        call_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMWithToolCallNoProfile {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == 1 {
                // First call: return a tool call WITHOUT profile_id
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Let me process that.".into(),
                        tool_calls: Some(vec![ToolCall {
                            id: "call-1".into(),
                            name: "capture_tool".into(),
                            arguments: serde_json::json!({"some_arg": "value"}),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                // Subsequent calls: return plain text answer
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Done.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let tool_calls = result.message.tool_calls.clone();
            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
            }
            events.push(Ok(StreamEvent::Done(result)));
            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test]
    async fn test_streaming_injects_profile_id_into_tool_call(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory SQLite pool
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Create registry with capture tool
        let profile_id_received = Arc::new(Mutex::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ProfileIdCaptureTool {
            profile_id_received: profile_id_received.clone(),
        }));
        let registry = Arc::new(registry);

        // 3. Create mock LLM
        let call_count = Arc::new(Mutex::new(0));
        let llm = Arc::new(MockLLMWithToolCallNoProfile {
            call_count: call_count.clone(),
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        // 4. Call process_message_stream (streaming path)
        let (tx, _rx) = mpsc::channel(100);
        let result = orchestrator
            .process_message_stream("profile-id", "gestiona mi agenda", None, tx)
            .await;

        // 5. Verify: streaming completes successfully
        assert!(
            result.is_ok(),
            "Streaming should complete, got: {:?}",
            result
        );

        // 6. Verify: tool received profile_id
        assert!(
            *profile_id_received.lock().unwrap(),
            "Streaming path should inject profile_id into tool calls"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_calendar_description_includes_agenda_keywords(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Verify that the calendar tool description helps LLM route "agenda" correctly
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let tool = crate::tools::calendar::CalendarTool::new(pool);
        let desc = tool.description();
        assert!(
            desc.to_lowercase().contains("agenda"),
            "Calendar description should contain 'agenda', got: {}",
            desc
        );
        assert!(
            desc.contains("eventos") || desc.contains("citas"),
            "Calendar description should contain 'eventos' or 'citas', got: {}",
            desc
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_reminders_description_includes_alarm_keywords(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let tool = crate::tools::reminders::RemindersTool::new(pool);
        let desc = tool.description();
        assert!(
            desc.contains("alarmas") || desc.contains("avisos"),
            "Reminders description should contain 'alarmas' or 'avisos', got: {}",
            desc
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Mock LLM that captures ChatRequest to inspect system prompt
    // -----------------------------------------------------------------------

    struct SystemPromptCaptureLLM {
        captured: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for SystemPromptCaptureLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            if let Some(msg) = request.messages.first() {
                *self.captured.lock().unwrap() = Some(msg.content.clone());
            }
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "OK.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            if let Some(msg) = request.messages.first() {
                *self.captured.lock().unwrap() = Some(msg.content.clone());
            }
            let events: Vec<Result<StreamEvent, LLMError>> =
                vec![Ok(StreamEvent::Done(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "OK.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                }))];
            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test]
    async fn test_custom_system_prompt_from_db_is_used_in_stream(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create in-memory DB with migrations
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // 2. Seed default settings then OVERRIDE system_prompt with a custom value
        crate::db::repos::settings::SettingsRepo::seed_defaults(&pool).await?;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "system_prompt",
            "Eres un asistente de pruebas. Responde siempre en español.",
        )
        .await?;
        crate::db::repos::settings::SettingsRepo::set(&pool, "max_window_tokens", "10000").await?;

        // 3. Create mock LLM that captures the system prompt
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let llm = Arc::new(SystemPromptCaptureLLM {
            captured: captured.clone(),
        });

        // 4. Build orchestrator components
        let registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config.clone(),
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        // 5. Call process_message_stream
        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("test-profile", "Hola", None, tx)
            .await?;

        // Drain rx to ensure processing completed
        while rx.recv().await.is_some() {}

        // 6. Assert the custom system prompt was used
        let captured_prompt = captured.lock().unwrap().clone();
        assert_eq!(
            captured_prompt.as_deref(),
            Some("Eres un asistente de pruebas. Responde siempre en español."),
            "System prompt should be the custom value from DB, not the default template. Got: {:?}",
            captured_prompt
        );

        // Also verify it is NOT the default template
        if let Some(ref p) = captured_prompt {
            assert!(
                !p.contains("mayordomo británico"),
                "System prompt should NOT be the default template, got: {}",
                p
            );
        }

        Ok(())
    }

    #[test]
    fn test_format_browser_timestamp_with_timezone() {
        // 06:23 UTC on 2026-09-26 = 08:23 CEST (Europe/Madrid, UTC+2)
        let result = format_browser_timestamp("2026-09-26T06:23:55.149Z", "Europe/Madrid");
        assert!(result.is_some());
        let s = result.unwrap();
        // Should say "son las 8:23 de la mañana" (local time), NOT "6:23"
        assert!(s.contains("8:23"), "Expected local time 8:23, got: {}", s);
        assert!(s.contains("de la mañana"), "Expected morning, got: {}", s);
        assert!(s.contains("sábado"), "Expected sábado");
        assert!(s.contains("26 de septiembre de 2026"));
    }

    #[test]
    fn test_format_browser_timestamp_utc() {
        // UTC timestamp with UTC timezone should show UTC time
        let result = format_browser_timestamp("2026-09-26T06:23:55.149Z", "UTC");
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("6:23"), "Expected UTC time 6:23, got: {}", s);
    }

    // -----------------------------------------------------------------------
    // Stats recording tests (RED — orchestrator does NOT call record_request yet)
    // -----------------------------------------------------------------------

    /// Helper: create in-memory SQLite pool, run migrations, seed a default profile.
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("failed to create in-memory pool");

        sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();

        // Seed a default profile for FK references
        sqlx::query(
            "INSERT OR IGNORE INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    /// Reset the seeded profile name to the default so the code-composed
    /// user-name section is omitted. The legacy section-ordering tests predate
    /// the `user-name-in-prompt` change and seed `profile-1` with the real name
    /// `"Test"`; neutralising the name keeps them focused on the section they
    /// actually exercise.
    async fn omit_user_name_section(pool: &SqlitePool) {
        crate::db::repos::profiles::ProfilesRepo::update(
            pool,
            Some(crate::db::repos::profiles::DEFAULT_PROFILE_NAME),
            None,
            None,
        )
        .await
        .expect("failed to reset the profile name to the default");
    }

    /// Mock LLM that returns plain text immediately (no tool calls).
    struct SimpleTextLLM;

    #[async_trait::async_trait]
    impl LLMProvider for SimpleTextLLM {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Simple response.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                    cost: 0.0,
                }),
            })
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let content = result.message.content.clone();
            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
            for chunk in content
                .chars()
                .collect::<Vec<_>>()
                .chunks(10)
                .map(|c| c.iter().collect::<String>())
            {
                events.push(Ok(StreamEvent::Chunk(chunk)));
            }
            events.push(Ok(StreamEvent::Done(result)));
            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    /// Mock LLM that always fails with an HTTP error.
    struct AlwaysFailingLLM;

    #[async_trait::async_trait]
    impl LLMProvider for AlwaysFailingLLM {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            Err(LLMError::HttpError("fail".into()))
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            Err(LLMError::HttpError("fail".into()))
        }
    }

    #[tokio::test]
    async fn test_process_message_stream_records_stats() -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;

        let llm = Arc::new(SimpleTextLLM);
        let registry = Arc::new(ToolRegistry::new());
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        let (tx, _rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "hello", None, tx)
            .await?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&pool)
            .await?;
        assert!(
            count >= 1,
            "Expected at least 1 row in llm_requests after a streamed LLM call."
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_process_message_stream_records_stats_on_llm_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;

        let llm = Arc::new(AlwaysFailingLLM);
        let registry = Arc::new(ToolRegistry::new());
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        );

        let (tx, mut rx) = mpsc::channel(100);
        let result = orchestrator
            .process_message_stream("profile-1", "hello", None, tx)
            .await;
        while rx.recv().await.is_some() {}
        assert!(
            result.is_err(),
            "process_message_stream should return an error with AlwaysFailingLLM"
        );

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests WHERE status = 'error'")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            count, 1,
            "Expected 1 row with status='error' in llm_requests after a failed streamed LLM call."
        );

        Ok(())
    }

    // ─── Contract tests: el chat usa GENERATION_CHAT_* ─────────────────────

    /// Scenario: El chat envía una temperatura explícita y los defaults.
    #[tokio::test]
    async fn test_chat_uses_generation_defaults() {
        let pool = setup_test_db().await;

        let request = captured_chat_request(pool).await;

        assert_eq!(request.temperature, Some(0.7));
        assert!(
            request.reasoning.is_none(),
            "default (empty) chat reasoning must be None, got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(4096));
    }

    /// Scenario: El chat puede pedir un nivel de razonamiento.
    #[tokio::test]
    async fn test_chat_can_request_reasoning_level() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "GENERATION_CHAT_REASONING", "high")
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;

        assert!(
            matches!(
                request.reasoning,
                Some(ReasoningSpec::Effort(ReasoningEffort::High))
            ),
            "expected Effort(High), got {:?}",
            request.reasoning
        );
    }

    /// Scenario: `off` desactiva el razonamiento del chat.
    #[tokio::test]
    async fn test_chat_off_disables_reasoning() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "GENERATION_CHAT_REASONING", "off")
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;

        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "expected Off, got {:?}",
            request.reasoning
        );
    }

    /// Scenario: Los tokens máximos del chat vienen de settings.
    #[tokio::test]
    async fn test_chat_max_tokens_from_settings() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "GENERATION_CHAT_MAX_TOKENS", "8000")
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;

        assert_eq!(request.max_tokens, Some(8000));
    }

    /// Scenario: El camino de streaming usa los mismos parámetros.
    #[tokio::test]
    async fn test_chat_stream_path_uses_same_generation_params() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "GENERATION_CHAT_TEMPERATURE", "0.5")
            .await
            .unwrap();
        crate::db::repos::settings::SettingsRepo::set(&pool, "GENERATION_CHAT_REASONING", "medium")
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;

        assert_eq!(request.temperature, Some(0.5));
        assert!(
            matches!(
                request.reasoning,
                Some(ReasoningSpec::Effort(ReasoningEffort::Medium))
            ),
            "expected Effort(Medium), got {:?}",
            request.reasoning
        );
    }

    /// Scenario: Una temperatura no parseable cae al default con warning.
    #[tokio::test]
    async fn test_chat_unparseable_temperature_falls_back_to_default() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "GENERATION_CHAT_TEMPERATURE",
            "caliente",
        )
        .await
        .unwrap();

        let request = captured_chat_request(pool).await;

        assert_eq!(
            request.temperature,
            Some(0.7),
            "an unparseable temperature must fall back to 0.7"
        );
    }

    // ─── user-name-in-prompt: sección `# USUARIO` (RED, sin implementar) ────
    //
    // Note: `captured_chat_request` uses `setup_test_db`, which seeds
    // `profile-1` with `name = 'Test'`. The user-name section is composed in
    // code —never from a `system_prompt` placeholder— between the prompt and
    // the persistent-memory section, and is omitted without trace when the
    // name is blank or the default `Valet User`.

    /// Scenario: El nombre real se inyecta.
    #[tokio::test]
    async fn test_user_name_section_injected_from_profile() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "system_prompt", "PROMPT_BASE")
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;

        assert_eq!(
            request.messages[0].role, "system",
            "the first message must be the system message"
        );
        let content = &request.messages[0].content;
        assert!(
            content.contains("# USUARIO"),
            "the system message must contain the `# USUARIO` section, got: {content:?}"
        );
        assert!(
            content.contains(
                "El nombre del usuario es Test. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta."
            ),
            "the system message must contain the user-name guidance, got: {content:?}"
        );

        let prompt_pos = content.find("PROMPT_BASE").expect("prompt must be present");
        let user_pos = content
            .find("# USUARIO")
            .expect("the `# USUARIO` section must be present");
        assert!(
            prompt_pos < user_pos,
            "the prompt must precede the `# USUARIO` section, got: {content:?}"
        );
        assert!(
            content.starts_with("PROMPT_BASE"),
            "the system message must start with the prompt, got: {content:?}"
        );
    }

    /// Scenario: El nombre de usuario precede a la memoria persistente.
    #[tokio::test]
    async fn test_user_name_section_before_persistent_memory() {
        let pool = setup_test_db().await;
        seed_persistent_state(
            &pool,
            r#"{"schema_version":1,"user_profile":{"city":"Madrid"}}"#,
        )
        .await;

        let request = captured_chat_request(pool).await;
        let content = &request.messages[0].content;

        let user_pos = content
            .find("# USUARIO")
            .expect("the `# USUARIO` section must be present");
        let persistent_pos = content
            .find("# MEMORIA PERSISTENTE (CAPA C)")
            .expect("the persistent-memory section must be present");
        assert!(
            user_pos < persistent_pos,
            "`# USUARIO` must precede the persistent-memory section, got: {content:?}"
        );
    }

    /// Scenario: Nombre vacío no deja rastro.
    #[tokio::test]
    async fn test_blank_user_name_omits_section() {
        let pool = setup_test_db().await;
        crate::db::repos::profiles::ProfilesRepo::update(&pool, Some("   "), None, None)
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;
        let content = &request.messages[0].content;

        assert!(
            !content.contains("# USUARIO"),
            "a blank name must leave no `# USUARIO` trace, got: {content:?}"
        );
    }

    /// Scenario: El nombre por defecto no se inyecta.
    #[tokio::test]
    async fn test_default_user_name_omits_section() {
        let pool = setup_test_db().await;
        crate::db::repos::profiles::ProfilesRepo::update(&pool, Some("Valet User"), None, None)
            .await
            .unwrap();

        let request = captured_chat_request(pool).await;
        let content = &request.messages[0].content;

        assert!(
            !content.contains("# USUARIO"),
            "the default `Valet User` name must leave no `# USUARIO` trace, got: {content:?}"
        );
    }

    /// Scenario: perfil ausente no aborta la petición.
    #[tokio::test]
    async fn test_missing_profile_omits_section_and_succeeds() {
        let pool = setup_test_db().await;
        sqlx::query("DELETE FROM profiles WHERE id = 'profile-1'")
            .execute(&pool)
            .await
            .unwrap();

        // `captured_chat_request` unwraps `process_message_stream`, so reaching
        // the assertions below proves the request did not panic.
        let request = captured_chat_request(pool).await;
        let content = &request.messages[0].content;

        assert!(
            !content.contains("# USUARIO"),
            "a missing profile must leave no `# USUARIO` trace, got: {content:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Human-in-the-loop approval: end-to-end pause/resume
    // -----------------------------------------------------------------------

    /// Tool that only runs when explicitly approved, counting executions.
    struct ExplicitApprovalTool {
        executed: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Tool for ExplicitApprovalTool {
        fn name(&self) -> &'static str {
            "explicit_tool"
        }

        fn description(&self) -> &'static str {
            "A tool that requires explicit human approval"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> Permission {
            Permission::ExplicitApproval
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            self.executed.fetch_add(1, Ordering::SeqCst);
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({"deleted": true}),
                message: None,
            })
        }
    }

    /// Mock LLM: the first call requests `explicit_tool`; later calls answer in
    /// plain text. The message list of the *second* call is captured so tests
    /// can inspect what the LLM saw after the approval decision.
    struct MockLLMExplicitApproval {
        call_count: Arc<Mutex<usize>>,
        second_call_messages: Arc<Mutex<Option<Vec<ChatMessage>>>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMExplicitApproval {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Understood.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
            })
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let n = {
                let mut count = self.call_count.lock().unwrap();
                *count += 1;
                *count
            };

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();

            if n == 1 {
                let tool_call = ToolCall {
                    id: "call-approval-1".into(),
                    name: "explicit_tool".into(),
                    arguments: serde_json::json!({"operation": "delete"}),
                };
                events.push(Ok(StreamEvent::ToolCall(tool_call.clone())));
                events.push(Ok(StreamEvent::Done(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: String::new(),
                        tool_calls: Some(vec![tool_call]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })));
            } else {
                if n == 2 {
                    *self.second_call_messages.lock().unwrap() = Some(request.messages.clone());
                }
                let text = "The action was handled.";
                for chunk in text
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
                events.push(Ok(StreamEvent::Done(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: text.into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })));
            }

            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// SSE events captured during an approval flow, with small query helpers.
    struct ApprovalHarness {
        events: Vec<SSEEvent>,
    }

    impl ApprovalHarness {
        fn has_approval_result(&self, approved: bool) -> bool {
            self.events.iter().any(|e| {
                matches!(
                    e,
                    SSEEvent::ApprovalResult { approved: a, .. } if *a == approved
                )
            })
        }

        fn has_done(&self) -> bool {
            self.events
                .iter()
                .any(|e| matches!(e, SSEEvent::Done { .. }))
        }

        fn has_error(&self) -> bool {
            self.events
                .iter()
                .any(|e| matches!(e, SSEEvent::Error { .. }))
        }
    }

    /// Drive `process_message_stream` in a background task and resolve the first
    /// approval request with `approved`, returning the emitted events and the
    /// orchestrator's final result.
    async fn run_approval_flow(
        pool: SqlitePool,
        executed: Arc<AtomicUsize>,
        second_call_messages: Arc<Mutex<Option<Vec<ChatMessage>>>>,
        approved: bool,
    ) -> (ApprovalHarness, Result<(), Box<dyn std::error::Error>>) {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(ExplicitApprovalTool {
            executed: executed.clone(),
        }));
        let registry = Arc::new(registry);

        let llm = Arc::new(MockLLMExplicitApproval {
            call_count: Arc::new(Mutex::new(0)),
            second_call_messages,
        });
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig::default();

        let orchestrator = Arc::new(Orchestrator::new(
            llm,
            registry,
            guardrails.clone(),
            context_builder,
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        ));

        let (tx, mut rx) = mpsc::channel(100);
        let orch = orchestrator.clone();
        let handle = tokio::spawn(async move {
            orch.process_message_stream("profile-1", "delete something", None, tx)
                .await
        });

        let mut events = Vec::new();
        while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            if let SSEEvent::ApprovalRequired { request_id, .. } = &event {
                guardrails
                    .resolve_approval(request_id, approved)
                    .expect("resolve_approval must succeed");
            }
            let done = matches!(event, SSEEvent::Done { .. });
            events.push(event);
            if done {
                break;
            }
        }

        let join = handle.await.expect("orchestrator task must not panic");
        let result: Result<(), Box<dyn std::error::Error>> = join.map_err(|e| e.into());
        (ApprovalHarness { events }, result)
    }

    #[tokio::test]
    async fn test_approval_approved_executes_tool_and_finishes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        let executed = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(Mutex::new(None));

        let (harness, result) = run_approval_flow(pool, executed.clone(), second, true).await;

        result?;
        assert_eq!(
            executed.load(Ordering::SeqCst),
            1,
            "the approved tool must execute exactly once"
        );
        assert!(
            harness.has_approval_result(true),
            "an ApprovalResult with approved: true must be emitted"
        );
        assert!(harness.has_done(), "the stream must end with Done");
        assert!(
            !harness.has_error(),
            "no SSE error event must be emitted for the approval path"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_approval_denied_does_not_execute_and_finishes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = setup_test_db().await;
        let executed = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(Mutex::new(None));

        let (harness, result) =
            run_approval_flow(pool, executed.clone(), second.clone(), false).await;

        result?;
        assert_eq!(
            executed.load(Ordering::SeqCst),
            0,
            "a denied tool must not execute"
        );
        assert!(
            harness.has_approval_result(false),
            "an ApprovalResult with approved: false must be emitted"
        );
        assert!(harness.has_done(), "the stream must end with Done");
        assert!(
            !harness.has_error(),
            "no SSE error event must be emitted for the denial path"
        );

        // The LLM must have received a `role:"tool"` rejection message.
        let messages = second
            .lock()
            .unwrap()
            .clone()
            .expect("the second LLM call must have been captured");
        assert!(
            messages
                .iter()
                .any(|m| m.role == "tool" && m.content.contains("not approved")),
            "the rejection message must reach the LLM, got: {messages:?}"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Skill router integration (skill-router, group 6)
    // -----------------------------------------------------------------------

    /// Decisions double that counts invocations and returns prefixed
    /// probabilities. `fail` makes it return an error (fail-open case).
    struct CountingDecisions {
        calls: Arc<AtomicUsize>,
        answers: std::collections::HashMap<String, f32>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl DecisionsProvider for CountingDecisions {
        async fn decide(&self, _request: DecisionsRequest) -> Result<DecisionsResponse, LLMError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(LLMError::HttpError("boom".into()));
            }
            Ok(DecisionsResponse {
                answers: self.answers.clone(),
                input_tokens: 1,
                output_tokens: 1,
                cost: 0.0,
            })
        }
    }

    fn counting_decisions(
        calls: Arc<AtomicUsize>,
        answers: &[(&str, f32)],
        fail: bool,
    ) -> Arc<dyn DecisionsProvider> {
        Arc::new(CountingDecisions {
            calls,
            answers: answers.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            fail,
        })
    }

    /// Build an orchestrator with the production registry (13 tools) and a mock
    /// LLM that captures the full chat request, attaching the given classifier.
    async fn build_routed_orchestrator(
        pool: SqlitePool,
        captured: Arc<Mutex<Option<ChatRequest>>>,
        decisions: Option<Arc<dyn DecisionsProvider>>,
    ) -> Orchestrator {
        let llm = Arc::new(FullChatRequestCaptureLLM { captured });
        let registry = Arc::new(crate::build_tool_registry(&pool));
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig {
            enable_reflection: false,
            ..Default::default()
        };
        Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        )
        .with_decisions(decisions)
    }

    /// Run one turn on a routed orchestrator and return the captured request.
    async fn run_routed_turn(
        pool: SqlitePool,
        decisions: Option<Arc<dyn DecisionsProvider>>,
    ) -> ChatRequest {
        let captured: Arc<Mutex<Option<ChatRequest>>> = Arc::new(Mutex::new(None));
        let orchestrator = build_routed_orchestrator(pool, captured.clone(), decisions).await;

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "Hola", None, tx)
            .await
            .expect("process_message_stream must succeed");
        while rx.recv().await.is_some() {}

        let request = captured
            .lock()
            .unwrap()
            .clone()
            .expect("the chat request must have been captured");
        request
    }

    /// Tool names advertised by a captured request, sorted for set comparison.
    fn tool_names(request: &ChatRequest) -> Vec<String> {
        let mut names: Vec<String> = request
            .tools
            .as_ref()
            .map(|defs| defs.iter().map(|d| d.name.clone()).collect())
            .unwrap_or_default();
        names.sort_unstable();
        names
    }

    /// The names of every tool enabled in the production registry, sorted.
    async fn all_enabled_tool_names(pool: &SqlitePool) -> Vec<String> {
        let mut names: Vec<String> = crate::build_tool_registry(pool)
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        names.sort_unstable();
        names
    }

    /// The `role:"system"` message of a captured request.
    fn system_message(request: &ChatRequest) -> &str {
        request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .expect("a system message must be present")
    }

    /// R5: with the router on and `agenda` selected, the request advertises
    /// exactly the core plus `calendar`.
    #[tokio::test]
    async fn routed_turn_exposes_only_core_and_selected_skills() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("agenda", 0.9)], false);

        let request = run_routed_turn(pool, Some(decisions)).await;

        let mut expected = vec![
            "calendar".to_string(),
            "get_current_time".to_string(),
            "get_current_location".to_string(),
        ];
        expected.sort_unstable();

        assert_eq!(
            tool_names(&request),
            expected,
            "an agenda turn must advertise exactly the core plus calendar"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the classifier must run exactly once per turn"
        );
    }

    /// Mock LLM that emits a tool call on its first two calls, then answers.
    struct MockLLMWithTwoToolCallsThenAnswer {
        call_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockLLMWithTwoToolCallsThenAnswer {
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count <= 2 {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: format!("Checking ({})", *count),
                        tool_calls: Some(vec![ToolCall {
                            id: format!("call-{}", *count),
                            name: "weather".into(),
                            arguments: serde_json::json!({
                                "latitude": 40.4168,
                                "longitude": -3.7038
                            }),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            } else {
                Ok(ChatResponse {
                    message: ChatMessage {
                        role: "assistant".into(),
                        content: "Listo.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: ChatRequest,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>
        {
            let result = self.chat(request).await?;
            let tool_calls = result.message.tool_calls.clone();

            let mut events: Vec<Result<StreamEvent, LLMError>> = Vec::new();
            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(StreamEvent::ToolCall(tc)));
                }
            } else {
                for chunk in result
                    .message
                    .content
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(10)
                    .map(|c| c.iter().collect::<String>())
                {
                    events.push(Ok(StreamEvent::Chunk(chunk)));
                }
            }
            events.push(Ok(StreamEvent::Done(result)));

            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// R2: a turn spanning three ReAct iterations decides exactly once.
    #[tokio::test]
    async fn routing_decides_once_per_turn_across_react_iterations() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("clima", 0.9)], false);

        let llm = Arc::new(MockLLMWithTwoToolCallsThenAnswer {
            call_count: Arc::new(Mutex::new(0)),
        });
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockWeatherTool));
        let registry = Arc::new(registry);
        let guardrails = Arc::new(Guardrails::new(registry.clone()));
        let context_builder = Arc::new(ContextBuilder::new());
        let config = OrchestratorConfig {
            enable_reflection: false,
            ..Default::default()
        };
        let orchestrator = Orchestrator::new(
            llm,
            registry,
            guardrails,
            context_builder,
            config,
            pool,
            None,
            None,
            Arc::new(RwLock::new(None)),
        )
        .with_decisions(Some(decisions));

        let (tx, mut rx) = mpsc::channel(100);
        orchestrator
            .process_message_stream("profile-1", "¿Qué tiempo hace?", None, tx)
            .await
            .expect("process_message_stream must succeed");
        while rx.recv().await.is_some() {}

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the classifier must run once even across three ReAct iterations"
        );
    }

    /// R4/D6(a): a classifier error exposes all enabled tools.
    #[tokio::test]
    async fn classifier_error_fails_open_with_all_tools() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();
        let expected = all_enabled_tool_names(&pool).await;

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[], true);

        let request = run_routed_turn(pool, Some(decisions)).await;

        assert_eq!(
            tool_names(&request),
            expected,
            "a classifier error must expose all enabled tools"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the classifier must have been attempted"
        );
    }

    /// R4/D6(b): with every probability below the threshold only the core shows.
    #[tokio::test]
    async fn all_below_threshold_exposes_core_only() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions =
            counting_decisions(calls.clone(), &[("agenda", 0.05), ("entorno", 0.09)], false);

        let request = run_routed_turn(pool, Some(decisions)).await;

        let mut expected = vec![
            "get_current_time".to_string(),
            "get_current_location".to_string(),
        ];
        expected.sort_unstable();

        assert_eq!(
            tool_names(&request),
            expected,
            "a conversational turn must expose only the core"
        );
    }

    /// R4/D6(c): a disabled router exposes all enabled tools and never calls the
    /// classifier. The disabled state is set explicitly: it is no longer the default.
    #[tokio::test]
    async fn router_disabled_exposes_all_tools() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "false")
            .await
            .unwrap();
        let expected = all_enabled_tool_names(&pool).await;

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("agenda", 1.0)], false);

        let request = run_routed_turn(pool, Some(decisions)).await;

        assert_eq!(
            tool_names(&request),
            expected,
            "a disabled router must expose all enabled tools"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a disabled router must not call the classifier"
        );
    }

    /// R4/D6(d): without a decisions client the router fails open.
    #[tokio::test]
    async fn missing_decisions_client_exposes_all_tools() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();
        let expected = all_enabled_tool_names(&pool).await;

        let request = run_routed_turn(pool, None).await;

        assert_eq!(
            tool_names(&request),
            expected,
            "without a classifier the router must expose all enabled tools"
        );
    }

    /// R6: only the fragments of the active skills are injected, and the base
    /// prompt is left untouched.
    #[tokio::test]
    async fn only_active_skill_fragments_are_injected() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "SKILL_AGENDA_PROMPT",
            "FRAGMENTO_AGENDA_UNICO",
        )
        .await
        .unwrap();
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "SKILL_TAREAS_PROMPT",
            "FRAGMENTO_TAREAS_UNICO",
        )
        .await
        .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("agenda", 0.9)], false);

        let request = run_routed_turn(pool, Some(decisions)).await;
        let system = system_message(&request);

        assert!(
            system.contains("FRAGMENTO_AGENDA_UNICO"),
            "the active agenda fragment must be injected, got: {system:?}"
        );
        assert!(
            !system.contains("FRAGMENTO_TAREAS_UNICO"),
            "an inactive skill fragment must not be injected, got: {system:?}"
        );
    }

    /// D4/R8: a routed turn with a successful classifier persists exactly one
    /// `kind='router'` row with `status='success'` and the router's model. The
    /// obsolete characterisation this replaces asserted the opposite (that the
    /// classifier never wrote a row); the new contract inverts it.
    #[tokio::test]
    async fn routed_turn_persists_router_success_row() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("agenda", 0.9)], false);
        let _request = run_routed_turn(pool.clone(), Some(decisions)).await;

        let router_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM llm_requests \
             WHERE kind = 'router' AND status = 'success' AND model = 'typesafe/jev-1.13'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            router_rows, 1,
            "a routed turn must persist exactly one success row for the classifier"
        );
    }

    /// D5: a classifier failure is persisted as a `kind='router'` row with
    /// `status='error'`.
    #[tokio::test]
    async fn routed_turn_persists_router_error_row() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "true")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[], true);
        let _request = run_routed_turn(pool.clone(), Some(decisions)).await;

        let error_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM llm_requests WHERE kind = 'router' AND status = 'error'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            error_rows, 1,
            "a classifier failure must be persisted as a router error row"
        );
    }

    /// D4: with the router off there was no classifier call, so no `kind='router'`
    /// row is persisted.
    #[tokio::test]
    async fn disabled_router_persists_no_router_row() {
        let pool = setup_test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&pool, "ROUTER_ENABLED", "false")
            .await
            .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let decisions = counting_decisions(calls.clone(), &[("agenda", 1.0)], false);
        let _request = run_routed_turn(pool.clone(), Some(decisions)).await;

        let router_rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests WHERE kind = 'router'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            router_rows, 0,
            "a disabled router never calls the classifier, so it must persist no router row"
        );
    }
}
