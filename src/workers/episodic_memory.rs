use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use sqlx::Row;
use sqlx::SqlitePool;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing;
use uuid::Uuid;

use crate::db::repos::memory::MemoryRepo;
use crate::db::repos::persistent_memory::PersistentMemoryRepo;
use crate::db::repos::stats::StatsRepo;
use crate::embeddings::EmbeddingProvider;
use crate::llm::provider::{ChatMessage, ChatRequest, LLMProvider, ResponseFormat};
use crate::models::message::estimate_markdown_tokens_heuristic;
use crate::models::stats::CallKind;
use crate::persistent_memory::{
    evaluate_compressed, payload_token_count, resolve_updated_at, validate_payload,
    CompressionOutcome,
};

/// A lightweight representation of a message for batch processing.
#[derive(Debug, Clone)]
struct UnindexedMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub tokens_count: usize,
    pub created_at: String,
}

/// Configuration for the [`EpisodicMemoryWorker`].
#[derive(Debug, Clone)]
pub struct EpisodicMemoryConfig {
    /// Maximum accumulated tokens in a batch before forcing processing
    /// (default: 2000).
    pub batch_tokens: usize,
    /// Minutes of inactivity after which a partial batch is flushed
    /// (default: 30).
    pub inactivity_minutes: i64,
    /// Number of messages to include before/after the batch for context
    /// (default: 2).
    pub overlap: i64,
    /// Polling interval in minutes for periodic evaluation
    /// (default: 30).
    pub poll_interval_minutes: u64,
    /// LLM model used for generating episodic memory cards
    /// (default: `"mistralai/mistral-small-24b-instruct-2501"`).
    pub model: String,
    /// LLM model used by the persistent-memory consolidator (and its single
    /// compression pass). Comes from `SEMANTIC_MODEL` / `MEMORY_MODEL`.
    pub semantic_model: String,
}

impl Default for EpisodicMemoryConfig {
    fn default() -> Self {
        Self {
            batch_tokens: 2000,
            inactivity_minutes: 30,
            overlap: 2,
            poll_interval_minutes: 30,
            model: "mistralai/mistral-small-24b-instruct-2501".into(),
            semantic_model: "mistralai/mistral-small-24b-instruct-2501".into(),
        }
    }
}

/// Minimal archivist prompt used only when `settings.archivist_prompt` is
/// missing or empty. The real prompt lives in the database (seeded by
/// migration `20260929000001_prompts.sql`) and must contain the
/// `{{ BLOQUE_DE_MENSAJES }}` placeholder.
const DEFAULT_ARCHIVIST_PROMPT_FALLBACK: &str = "System: Eres un archivista de memoria. Resume la conversación en una ficha concisa.\n\nConversación a procesar:\n{{ BLOQUE_DE_MENSAJES }}";

/// Marker present in every consolidator/compression system prompt. Lets the
/// semantic call be told apart from the episodic one (also used by tests).
const SEMANTIC_CALL_MARKER: &str = "consolidador de memoria persistente";

/// Marker present only in the compression prompt.
const COMPRESSION_CALL_MARKER: &str = "PRESUPUESTO DE MEMORIA PERSISTENTE";

/// Fallback consolidator prompt template, used only when
/// `settings.consolidator_prompt` is missing, empty or unreadable. The marker
/// placeholder is replaced at runtime by [`SEMANTIC_CALL_MARKER`].
const DEFAULT_CONSOLIDATOR_PROMPT_TEMPLATE: &str = r#"System: Eres el __SEMANTIC_MARKER__ de Valet. Analizas una conversación reciente entre el usuario y su asistente, y actualizas de forma acumulativa el Perfil de Usuario y las Reglas de Comportamiento.

Devuelve EXCLUSIVAMENTE un objeto JSON válido, sin bloques de código ni texto adicional:
{
  "schema_version": 1,
  "user_profile": { },
  "system_rules": [ ]
}

Organiza user_profile con estas secciones, incluyendo ÚNICAMENTE las que tengan contenido real (no emitas secciones vacías):
- identity: nombre, idioma y ubicación (solo si se afirman explícitamente).
- preferences_and_tastes: objeto con communication_style (tono, detalle, trato tú/usted, idioma, términos a evitar), technology_and_tools, lifestyle_and_leisure y dislikes_and_dealbreakers (lo que detesta o evita).
- lifestyle_and_routines: horarios, hábitos y eventos recurrentes.
- productivity_and_workflow: metodologías y autonomía delegada.
- interests_and_knowledge: proyectos activos y temas de interés.
- relationships_and_entities: personas, proyectos o entidades clave.

Reglas de consolidación:
- Filtro de permanencia: guarda solo preferencias estables; ignora lo efímero de un turno.
- Registra tanto lo que le gusta como lo que rechaza.
- Sobrescritura: ante contradicción prevalece lo nuevo; elimina el dato antiguo.
- No dupliques: si un dato ya está en user_profile, no lo repitas en system_rules ni como regla equivalente.
- system_rules: SOLO instrucciones explícitas del usuario dirigidas al asistente, en forma imperativa y atómica. El trato, el idioma o los términos a evitar pertenecen a communication_style, no a system_rules.
- No inventes ni infieras: no añadas datos (p. ej. huso horario) que la conversación no afirme con claridad.

Estado persistente actual:
{{ ESTADO_ACTUAL }}

Bloque de mensajes:
{{ BLOQUE_DE_MENSAJES }}"#;

/// Number of attempts for the initial consolidation (one retry).
const INITIAL_CONSOLIDATION_ATTEMPTS: usize = 2;

/// Compression prompt template for the single compression pass. The marker
/// placeholder is replaced at runtime by [`COMPRESSION_CALL_MARKER`].
const COMPRESSION_PROMPT_TEMPLATE: &str = "\
La memoria persistente supera el __COMPRESSION_MARKER__.
Devuelve EXCLUSIVAMENTE un objeto JSON con la misma forma (schema_version = 1, user_profile y system_rules),
comprimido para reducir tokens, conservando la información esencial y sin añadir claves nuevas.

Estado persistente actual:
{{ ESTADO_ACTUAL }}";

/// Build the minimal fallback consolidator prompt (with both placeholders).
fn default_consolidator_prompt() -> String {
    DEFAULT_CONSOLIDATOR_PROMPT_TEMPLATE.replace("__SEMANTIC_MARKER__", SEMANTIC_CALL_MARKER)
}

/// Build the compression prompt.
fn compression_prompt() -> String {
    COMPRESSION_PROMPT_TEMPLATE.replace("__COMPRESSION_MARKER__", COMPRESSION_CALL_MARKER)
}

/// Outcome of the consolidator for one pass.
#[derive(Debug, Clone, PartialEq)]
enum Consolidation {
    /// A new Layer C state is ready to be persisted in the pass transaction.
    Write {
        payload: serde_json::Value,
        updated_at: String,
    },
    /// The state was refused because it exceeds the absolute ceiling: the
    /// previous one is kept (no write) and the pass continues. This is **not**
    /// a consolidator failure.
    KeepPrevious,
}

/// A precomputed Layer C write, ready to go inside the pass transaction.
struct PersistentWrite {
    payload: String,
    updated_at: String,
}

/// Failure of the **initial** consolidation step (the one that produces the
/// Layer C state). Any of these aborts the pass: nothing is written, nothing is
/// marked, and the cooldown starts.
///
/// Size management is deliberately **not** represented here: a compression
/// failure is never a pass failure, it degrades (see `consolidate_state`).
#[derive(Debug, thiserror::Error)]
enum ConsolidationError {
    #[error("persistent-memory database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("consolidator LLM call failed: {0}")]
    Llm(String),
    #[error("consolidator returned an invalid state: {0}")]
    Invalid(String),
}

/// Compute the cooldown (in seconds) applied after a failed LLM attempt.
///
/// Per spec this is `max(poll_interval / 2, 30s)`.
fn cooldown_secs(config: &EpisodicMemoryConfig) -> i64 {
    (config.poll_interval_minutes as i64 * 60 / 2).max(30)
}

/// A structured memory card extracted from an LLM response.
#[derive(Debug, Clone)]
struct MemoryCard {
    pub date_context: String,
    pub topics: String,
    pub facts: String,
    pub synthesis: String,
}

/// Background worker that groups unindexed messages into episodic memory
/// cards ("fichas") via an LLM.
///
/// The worker is triggered by two sources:
/// - A signal on the `memory_rx` channel (sent when new messages are created).
/// - A periodic timer (`poll_interval_minutes`).
pub struct EpisodicMemoryWorker;

impl EpisodicMemoryWorker {
    /// Start the episodic memory worker in a new tokio task.
    ///
    /// The worker loops forever (or until `shutdown_rx` fires), selecting
    /// on a channel signal, an interval tick, and a shutdown signal.
    /// On each trigger it calls [`evaluate`](Self::evaluate) to process any
    /// unindexed messages.
    pub fn start(
        db: SqlitePool,
        llm_provider: Arc<dyn LLMProvider>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        mut memory_rx: mpsc::Receiver<()>,
        mut shutdown_rx: broadcast::Receiver<()>,
        config: EpisodicMemoryConfig,
    ) -> JoinHandle<()> {
        let poll_interval = std::time::Duration::from_secs(config.poll_interval_minutes * 60);
        // Per-instance rate limiter: Unix timestamp (seconds) of the last
        // failed LLM attempt. Never shared between workers.
        let last_llm_attempt = Arc::new(AtomicI64::new(0));
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            // Tick immediately on start
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = memory_rx.recv() => {
                        tracing::debug!("EpisodicMemoryWorker triggered by channel signal");
                        Self::evaluate(&db, llm_provider.clone(), embedding_provider.clone(), &config, &last_llm_attempt).await;
                    }
                    _ = interval.tick() => {
                        tracing::debug!("EpisodicMemoryWorker triggered by timer");
                        Self::evaluate(&db, llm_provider.clone(), embedding_provider.clone(), &config, &last_llm_attempt).await;
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("EpisodicMemoryWorker shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Core evaluation logic: query unindexed messages, check conditions,
    /// build a batch (with overlap), call the LLM, and persist the result.
    pub(crate) async fn evaluate(
        db: &SqlitePool,
        llm_provider: Arc<dyn LLMProvider>,
        embedding_provider: Arc<dyn EmbeddingProvider>,
        config: &EpisodicMemoryConfig,
        last_llm_attempt: &Arc<AtomicI64>,
    ) {
        // 1. Query unindexed messages
        let unindexed = match Self::query_unindexed_messages(db).await {
            Ok(msgs) => msgs,
            Err(e) => {
                tracing::error!(error = %e, "EpisodicMemoryWorker: failed to query unindexed messages");
                return;
            }
        };

        if unindexed.is_empty() {
            tracing::debug!("EpisodicMemoryWorker: no unindexed messages to process");
            return;
        }

        // 2. Check conditions
        let total_tokens: usize = unindexed.iter().map(|m| m.tokens_count).sum();
        let should_process = Self::should_process_batch(&unindexed, total_tokens, config).await;

        if !should_process {
            tracing::debug!(
                total_tokens = %total_tokens,
                batch_tokens = %config.batch_tokens,
                "EpisodicMemoryWorker: batch does not meet processing conditions"
            );
            return;
        }

        // 3. Build batch with overlap
        let (primary, overlap_before, overlap_after) =
            match Self::build_batch_with_overlap(db, &unindexed, config).await {
                Ok(batch) => batch,
                Err(e) => {
                    tracing::error!(error = %e, "EpisodicMemoryWorker: failed to build batch");
                    return;
                }
            };

        if primary.is_empty() {
            tracing::warn!("EpisodicMemoryWorker: primary batch is empty, skipping");
            return;
        }

        // 4. Build the message block for the LLM
        let message_block = Self::format_message_block(&primary, &overlap_before, &overlap_after);

        // 5. Rate-limit: skip LLM call if we just failed recently
        let now_ts = chrono::Utc::now().timestamp();
        let last_attempt = last_llm_attempt.load(Ordering::Relaxed);
        let cooldown = cooldown_secs(config);

        if now_ts - last_attempt < cooldown && last_attempt > 0 {
            // We already know `unindexed` is non-empty (checked at the top), so
            // the batch is still waiting: skip the LLM call.
            tracing::warn!(
                seconds_since_last_attempt = %(now_ts - last_attempt),
                "EpisodicMemoryWorker: skipping LLM call (rate-limited after previous failure)"
            );
            return;
        }

        // 5b. Extraction 1/2: the episodic card (Layer B).
        let card = match Self::call_llm(db, &llm_provider, config, &message_block).await {
            Some(card) => card,
            None => {
                last_llm_attempt.store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
                tracing::error!(
                    "EpisodicMemoryWorker: LLM call failed or returned unparseable response"
                );
                return;
            }
        };

        // 5c. Extraction 2/2: the persistent state (Layer C) from the SAME
        // batch, before writing anything. A failure here must leave neither
        // the card nor the state behind.
        let consolidation =
            match Self::consolidate_state(db, &llm_provider, config, &message_block).await {
                Ok(outcome) => outcome,
                Err(e) => {
                    last_llm_attempt.store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
                    tracing::error!(
                        error = %e,
                        "EpisodicMemoryWorker: state consolidation failed; aborting the pass"
                    );
                    return;
                }
            };

        // Both extractions succeeded: clear the failure tracker.
        last_llm_attempt.store(0, Ordering::Relaxed);

        let persistent_write = match consolidation {
            Consolidation::Write {
                payload,
                updated_at,
            } => Some(PersistentWrite {
                payload: serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{\"schema_version\":1}".to_string()),
                updated_at,
            }),
            // A ceiling rejection keeps the previous state but is not a
            // failure: the pass continues and the card is still written.
            Consolidation::KeepPrevious => None,
        };

        // 6. Persist Layer B + Layer C + the index mark in ONE transaction.
        if let Err(e) = Self::persist(
            db,
            &embedding_provider,
            &card,
            &primary,
            persistent_write.as_ref(),
        )
        .await
        {
            // A failed persist must also start the cooldown window, otherwise
            // the next poll would re-call the LLM for the same batch (cost loop).
            last_llm_attempt.store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
            tracing::error!(
                error = %e,
                "EpisodicMemoryWorker: failed to persist the pass (card + state + mark)"
            );
        }
    }

    // ─── Private helpers ──────────────────────────────────────────────────

    /// Query all unindexed messages ordered by `created_at ASC`.
    /// Uses a conservative limit to avoid loading too many at once.
    async fn query_unindexed_messages(
        db: &SqlitePool,
    ) -> Result<Vec<UnindexedMessage>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, role, content, tokens_count, created_at
             FROM messages
             WHERE is_indexed = 0
             ORDER BY created_at ASC
             LIMIT 500",
        )
        .fetch_all(db)
        .await?;

        let messages = rows
            .iter()
            .map(|r| UnindexedMessage {
                id: r.get(0),
                role: r.get(1),
                content: r.get(2),
                tokens_count: r.get::<i64, _>(3) as usize,
                created_at: r.get(4),
            })
            .collect();

        Ok(messages)
    }

    /// Determine whether the current batch should be processed.
    async fn should_process_batch(
        unindexed: &[UnindexedMessage],
        total_tokens: usize,
        config: &EpisodicMemoryConfig,
    ) -> bool {
        // Condition A: token budget met
        if total_tokens >= config.batch_tokens {
            return true;
        }

        // Condition B: inactivity timeout (oldest message exceeds threshold)
        if let Some(oldest) = unindexed.first() {
            let now = chrono::Utc::now();
            let oldest_time = match chrono::DateTime::parse_from_rfc3339(&oldest.created_at) {
                Ok(t) => t.with_timezone(&chrono::Utc),
                Err(_) => return false,
            };
            let elapsed_minutes = (now - oldest_time).num_minutes();
            if elapsed_minutes >= config.inactivity_minutes {
                return true;
            }
        }

        false
    }

    /// Build the primary batch (within token budget) plus overlap messages
    /// before and after.
    async fn build_batch_with_overlap(
        db: &SqlitePool,
        unindexed: &[UnindexedMessage],
        config: &EpisodicMemoryConfig,
    ) -> Result<
        (
            Vec<UnindexedMessage>,
            Vec<UnindexedMessage>,
            Vec<UnindexedMessage>,
        ),
        sqlx::Error,
    > {
        // Primary batch: messages within token budget
        let mut cumulative = 0usize;
        let mut primary_end = 0;
        for (i, msg) in unindexed.iter().enumerate() {
            cumulative += msg.tokens_count;
            if cumulative >= config.batch_tokens {
                primary_end = i + 1;
                break;
            }
        }
        if primary_end == 0 {
            primary_end = unindexed.len();
        }
        let primary = unindexed[..primary_end].to_vec();

        if primary.is_empty() {
            return Ok((vec![], vec![], vec![]));
        }

        let first_id = &primary[0].id;
        let last_id = &primary[primary.len() - 1].id;

        // Overlap BEFORE: messages with created_at < first primary's created_at
        let overlap_rows = sqlx::query(
            "SELECT id, role, content, tokens_count, created_at
             FROM messages
             WHERE created_at < (SELECT created_at FROM messages WHERE id = ?1)
             ORDER BY created_at DESC
             LIMIT ?2",
        )
        .bind(first_id)
        .bind(config.overlap)
        .fetch_all(db)
        .await?;

        let mut overlap_before: Vec<UnindexedMessage> = overlap_rows
            .iter()
            .map(|r| UnindexedMessage {
                id: r.get(0),
                role: r.get(1),
                content: r.get(2),
                tokens_count: r.get::<i64, _>(3) as usize,
                created_at: r.get(4),
            })
            .collect();
        // Reverse so they appear chronologically
        overlap_before.reverse();

        // Overlap AFTER: messages with created_at > last primary's created_at
        let overlap_rows = sqlx::query(
            "SELECT id, role, content, tokens_count, created_at
             FROM messages
             WHERE created_at > (SELECT created_at FROM messages WHERE id = ?1)
             ORDER BY created_at ASC
             LIMIT ?2",
        )
        .bind(last_id)
        .bind(config.overlap)
        .fetch_all(db)
        .await?;

        let overlap_after: Vec<UnindexedMessage> = overlap_rows
            .iter()
            .map(|r| UnindexedMessage {
                id: r.get(0),
                role: r.get(1),
                content: r.get(2),
                tokens_count: r.get::<i64, _>(3) as usize,
                created_at: r.get(4),
            })
            .collect();

        Ok((primary, overlap_before, overlap_after))
    }

    /// Format the message block for the LLM prompt, tagging overlap messages.
    fn format_message_block(
        primary: &[UnindexedMessage],
        overlap_before: &[UnindexedMessage],
        overlap_after: &[UnindexedMessage],
    ) -> String {
        let mut parts = Vec::new();

        if !overlap_before.is_empty() {
            parts.push("[INICIO DE CONTEXTO ANTERIOR (overlap)]".to_string());
            for msg in overlap_before {
                parts.push(format!("[{}]: {}", msg.role, msg.content));
            }
            parts.push("[FIN DE CONTEXTO ANTERIOR (overlap)]".to_string());
        }

        parts.push("[INICIO DE BLOQUE PRINCIPAL A ARCHIVAR]".to_string());
        for msg in primary {
            parts.push(format!("[{}]: {}", msg.role, msg.content));
        }
        parts.push("[FIN DE BLOQUE PRINCIPAL A ARCHIVAR]".to_string());

        if !overlap_after.is_empty() {
            parts.push("[INICIO DE CONTEXTO POSTERIOR (overlap)]".to_string());
            for msg in overlap_after {
                parts.push(format!("[{}]: {}", msg.role, msg.content));
            }
            parts.push("[FIN DE CONTEXTO POSTERIOR (overlap)]".to_string());
        }

        parts.join("\n")
    }

    /// Call the LLM with the archivist prompt and message block.
    /// Parses the structured response into a `MemoryCard`.
    async fn call_llm(
        db: &SqlitePool,
        llm_provider: &Arc<dyn LLMProvider>,
        config: &EpisodicMemoryConfig,
        message_block: &str,
    ) -> Option<MemoryCard> {
        // The archivist prompt lives in `settings` (seeded by migration); fall
        // back to a minimal prompt if it is missing, empty, or unreadable.
        let archivist_prompt =
            match crate::db::repos::settings::SettingsRepo::get(db, "archivist_prompt").await {
                Ok(Some(p)) if !p.trim().is_empty() => p,
                Ok(_) => {
                    tracing::warn!(
                        "settings.archivist_prompt missing or empty; using minimal fallback"
                    );
                    DEFAULT_ARCHIVIST_PROMPT_FALLBACK.to_string()
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to read settings.archivist_prompt; using minimal fallback"
                    );
                    DEFAULT_ARCHIVIST_PROMPT_FALLBACK.to_string()
                }
            };
        let system_content = archivist_prompt.replace("{{ BLOQUE_DE_MENSAJES }}", message_block);

        tracing::debug!(
            prompt_preview = %system_content.chars().take(200).collect::<String>(),
            prompt_len = %system_content.len(),
            model = %config.model,
            "EpisodicMemoryWorker: LLM request prompt preview"
        );

        let generation = crate::generation::read_generation_params(
            db,
            crate::generation::GenerationRole::Memory,
        )
        .await;

        let request = ChatRequest {
            model: config.model.clone(),
            messages: vec![ChatMessage {
                role: "system".into(),
                content: system_content,
                tool_calls: None,
                tool_result: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(generation.temperature),
            max_tokens: Some(generation.max_tokens),
            stream: false,
            reasoning: generation.reasoning,
            response_format: None,
        };

        let start = std::time::Instant::now();
        let response = match llm_provider.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                let _ = StatsRepo::record_request(
                    db,
                    CallKind::Archivist,
                    &Uuid::new_v4().to_string(),
                    &config.model,
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
                    None,
                )
                .await;
                tracing::error!(error = %e, "EpisodicMemoryWorker: LLM chat failed");
                return None;
            }
        };

        // Record stats for this LLM call
        let duration_ms = start.elapsed().as_millis() as i64;
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
            db,
            CallKind::Archivist,
            &Uuid::new_v4().to_string(),
            &config.model,
            None,
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
            Some(duration_ms),
            "success",
            None,
            None,
            None,
        )
        .await;

        let content = &response.message.content;
        let content_preview = content.chars().take(300).collect::<String>();
        let card = Self::parse_memory_card(content);

        if card.is_none() {
            tracing::debug!(
                content_len = %content.len(),
                content_preview = %content_preview,
                "EpisodicMemoryWorker: LLM response content preview (unparseable)"
            );
        }

        card
    }

    /// Parse the structured LLM response into a `MemoryCard`.
    ///
    /// Expected format:
    /// ```text
    /// - FECHA/CONTEXTO: ...
    /// - TEMAS TRATADOS: ...
    /// - HECHOS Y DECISIONES: ...
    /// - SÍNTESIS: ...
    /// ```
    fn parse_memory_card(response: &str) -> Option<MemoryCard> {
        let mut date_context = String::new();
        let mut topics = String::new();
        let mut facts = String::new();
        let mut synthesis = String::new();

        let mut current_section: Option<&mut String> = None;

        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(rest) = trimmed
                .strip_prefix("- FECHA/CONTEXTO:")
                .or_else(|| trimmed.strip_prefix("- FECHA/CONTEXTO :"))
                .or_else(|| trimmed.strip_prefix("FECHA/CONTEXTO:"))
                .or_else(|| trimmed.strip_prefix("FECHA/CONTEXTO :"))
            {
                date_context = rest.trim().to_string();
                current_section = Some(&mut date_context);
                continue;
            }

            if let Some(rest) = trimmed
                .strip_prefix("- TEMAS TRATADOS:")
                .or_else(|| trimmed.strip_prefix("- TEMAS TRATADOS :"))
                .or_else(|| trimmed.strip_prefix("TEMAS TRATADOS:"))
                .or_else(|| trimmed.strip_prefix("TEMAS TRATADOS :"))
            {
                topics = rest.trim().to_string();
                current_section = Some(&mut topics);
                continue;
            }

            if let Some(rest) = trimmed
                .strip_prefix("- HECHOS Y DECISIONES:")
                .or_else(|| trimmed.strip_prefix("- HECHOS Y DECISIONES :"))
                .or_else(|| trimmed.strip_prefix("HECHOS Y DECISIONES:"))
                .or_else(|| trimmed.strip_prefix("HECHOS Y DECISIONES :"))
            {
                facts = rest.trim().to_string();
                current_section = Some(&mut facts);
                continue;
            }

            if let Some(rest) = trimmed
                .strip_prefix("- SÍNTESIS:")
                .or_else(|| trimmed.strip_prefix("- SÍNTESIS :"))
                .or_else(|| trimmed.strip_prefix("SÍNTESIS:"))
                .or_else(|| trimmed.strip_prefix("SÍNTESIS :"))
            {
                synthesis = rest.trim().to_string();
                current_section = Some(&mut synthesis);
                continue;
            }

            // Continuation line for the current section
            if let Some(ref mut section) = current_section {
                if !section.is_empty() {
                    section.push(' ');
                    section.push_str(trimmed);
                }
            }
        }

        // We need at least one meaningful section
        if date_context.is_empty() && topics.is_empty() && facts.is_empty() && synthesis.is_empty()
        {
            return None;
        }

        Some(MemoryCard {
            date_context,
            topics,
            facts,
            synthesis,
        })
    }

    // ─── Capa C: consolidation (budget, ceiling, prompt) ───────────────────

    /// Perform one semantic LLM call (consolidation or compression) and record
    /// its stats with `profile_id = NULL`, like every other worker call.
    async fn call_semantic_chat(
        db: &SqlitePool,
        llm_provider: &Arc<dyn LLMProvider>,
        config: &EpisodicMemoryConfig,
        system_content: String,
    ) -> Result<String, ConsolidationError> {
        let generation = crate::generation::read_generation_params(
            db,
            crate::generation::GenerationRole::Semantic,
        )
        .await;

        let request = ChatRequest {
            model: config.semantic_model.clone(),
            messages: vec![ChatMessage {
                role: "system".into(),
                content: system_content,
                tool_calls: None,
                tool_result: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(generation.temperature),
            max_tokens: Some(generation.max_tokens),
            stream: false,
            reasoning: generation.reasoning,
            response_format: Some(ResponseFormat::JsonObject),
        };

        let start = std::time::Instant::now();
        let response = match llm_provider.chat(request).await {
            Ok(response) => response,
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as i64;
                let _ = StatsRepo::record_request(
                    db,
                    CallKind::Consolidator,
                    &Uuid::new_v4().to_string(),
                    &config.semantic_model,
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
                    None,
                )
                .await;
                return Err(ConsolidationError::Llm(format!(
                    "semantic LLM call failed: {e}"
                )));
            }
        };

        let duration_ms = start.elapsed().as_millis() as i64;
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
            db,
            CallKind::Consolidator,
            &Uuid::new_v4().to_string(),
            &config.semantic_model,
            None,
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
            Some(duration_ms),
            "success",
            None,
            None,
            None,
        )
        .await;

        Ok(response.message.content)
    }

    /// Consolidate the Layer C state from the same batch used for the episodic
    /// card.
    ///
    /// Returns `Err(ConsolidationError)` only when the **initial** consolidation
    /// fails (LLM error, non-JSON, invalid schema or DB error); the caller then
    /// aborts the whole pass.
    ///
    /// Size management never aborts: if the state exceeds the budget, a single
    /// compression is attempted, and if that fails or still exceeds the ceiling,
    /// the uncompressed state is evaluated against the ceiling — stored when it
    /// fits (with a warning) or the previous state is kept (with a warning).
    async fn consolidate_state(
        db: &SqlitePool,
        llm_provider: &Arc<dyn LLMProvider>,
        config: &EpisodicMemoryConfig,
        message_block: &str,
    ) -> Result<Consolidation, ConsolidationError> {
        // Previous state; its absence is a valid empty state.
        let previous = PersistentMemoryRepo::get(db).await?;
        let current_payload = match previous.as_ref() {
            Some(entry) => match serde_json::from_str::<serde_json::Value>(&entry.payload) {
                Ok(payload) => payload,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "the stored persistent-memory payload is not valid JSON; treating it as empty"
                    );
                    serde_json::json!({ "schema_version": 1 })
                }
            },
            None => serde_json::json!({ "schema_version": 1 }),
        };

        // The prompt lives in `settings` (seeded by migration), with a minimal
        // fallback when it is missing, empty or unreadable.
        let prompt =
            match crate::db::repos::settings::SettingsRepo::get(db, "consolidator_prompt").await {
                Ok(Some(p)) if !p.trim().is_empty() => p,
                Ok(_) => {
                    tracing::warn!(
                        "settings.consolidator_prompt missing or empty; using minimal fallback"
                    );
                    default_consolidator_prompt()
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "failed to read settings.consolidator_prompt; using minimal fallback"
                    );
                    default_consolidator_prompt()
                }
            };
        let current_json = serde_json::to_string(&current_payload)
            .map_err(|e| ConsolidationError::Invalid(e.to_string()))?;
        let system_content = prompt
            .replace("{{ ESTADO_ACTUAL }}", &current_json)
            .replace("{{ BLOQUE_DE_MENSAJES }}", message_block);

        // Initial consolidation: a failure here aborts the pass. It gets a
        // single retry when the model returns no valid state.
        let validated =
            Self::initial_consolidation(db, llm_provider, config, &system_content).await?;

        // Size management: never aborts.
        let budget = crate::persistent_memory::read_budget(db).await;
        let tokens = payload_token_count(&validated);
        if tokens <= budget {
            return Ok(Self::seal_consolidation(previous.as_ref(), validated));
        }

        tracing::warn!(
            tokens,
            budget,
            "persistent memory exceeds the token budget; requesting one compression"
        );

        match Self::try_compress(db, llm_provider, config, &validated).await {
            Some(compressed) => {
                let compressed_tokens = payload_token_count(&compressed);
                match evaluate_compressed(compressed_tokens, budget) {
                    CompressionOutcome::Reject => {
                        tracing::warn!(
                            tokens = compressed_tokens,
                            budget,
                            ceiling = crate::persistent_memory::absolute_ceiling(budget),
                            "the compressed state exceeds the absolute ceiling; keeping the previous state"
                        );
                        Ok(Consolidation::KeepPrevious)
                    }
                    CompressionOutcome::StoreWithWarning => {
                        tracing::warn!(
                            tokens = compressed_tokens,
                            budget,
                            "the compressed state still exceeds the budget; storing with warning"
                        );
                        Ok(Self::seal_consolidation(previous.as_ref(), compressed))
                    }
                    CompressionOutcome::Store => {
                        Ok(Self::seal_consolidation(previous.as_ref(), compressed))
                    }
                }
            }
            None => {
                // The compression failed or produced nothing usable. Size
                // management must NOT abort the pass: evaluate the uncompressed
                // state against the ceiling instead.
                match evaluate_compressed(tokens, budget) {
                    CompressionOutcome::Reject => {
                        tracing::warn!(
                            tokens,
                            budget,
                            ceiling = crate::persistent_memory::absolute_ceiling(budget),
                            "compression failed and the uncompressed state exceeds the ceiling; keeping the previous state"
                        );
                        Ok(Consolidation::KeepPrevious)
                    }
                    CompressionOutcome::StoreWithWarning => {
                        tracing::warn!(
                            tokens,
                            budget,
                            "compression failed; storing the uncompressed state with warning"
                        );
                        Ok(Self::seal_consolidation(previous.as_ref(), validated))
                    }
                    CompressionOutcome::Store => {
                        // Unreachable in practice (`tokens > budget`): kept so
                        // the classification stays exhaustive.
                        Ok(Self::seal_consolidation(previous.as_ref(), validated))
                    }
                }
            }
        }
    }

    /// Build a `Consolidation::Write` for `payload`, resolving the Rust-owned
    /// `updated_at` against the previous state.
    fn seal_consolidation(
        previous: Option<&crate::models::PersistentMemory>,
        payload: serde_json::Value,
    ) -> Consolidation {
        let now = chrono::Utc::now().to_rfc3339();
        let updated_at = resolve_updated_at(previous, &payload, &now);
        Consolidation::Write {
            payload,
            updated_at,
        }
    }

    /// One compression attempt. Returns `None` on any failure — LLM error,
    /// non-JSON response, invalid schema — because size management must never
    /// abort the pass (the caller degrades to the uncompressed state).
    async fn try_compress(
        db: &SqlitePool,
        llm_provider: &Arc<dyn LLMProvider>,
        config: &EpisodicMemoryConfig,
        state: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        let serialized = serde_json::to_string(state).ok()?;
        let prompt = compression_prompt().replace("{{ ESTADO_ACTUAL }}", &serialized);

        let raw = match Self::call_semantic_chat(db, llm_provider, config, prompt).await {
            Ok(raw) => raw,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "compression LLM call failed; degrading to the uncompressed state"
                );
                return None;
            }
        };

        let candidate = match Self::extract_json_object(&raw) {
            Some(candidate) => candidate,
            None => {
                tracing::warn!(
                    "compression returned no JSON object; degrading to the uncompressed state"
                );
                return None;
            }
        };

        match validate_payload(&candidate) {
            Ok(validated) => Some(validated),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "compression returned an invalid state; degrading to the uncompressed state"
                );
                None
            }
        }
    }

    /// Short single-line preview of an LLM response, for diagnostics.
    fn content_preview(content: &str) -> String {
        content
            .chars()
            .take(200)
            .collect::<String>()
            .replace('\n', " ")
    }

    /// Run the initial consolidation, retrying **once** when the response yields no
    /// valid state (empty, non-JSON or schema-invalid). An LLM transport error is
    /// not retried: it propagates immediately.
    async fn initial_consolidation(
        db: &SqlitePool,
        llm_provider: &Arc<dyn LLMProvider>,
        config: &EpisodicMemoryConfig,
        system_content: &str,
    ) -> Result<serde_json::Value, ConsolidationError> {
        let mut last_error = String::new();
        for attempt in 1..=INITIAL_CONSOLIDATION_ATTEMPTS {
            let raw =
                Self::call_semantic_chat(db, llm_provider, config, system_content.to_string())
                    .await?;
            let content_len = raw.len();
            match Self::extract_json_object(&raw) {
                Some(candidate) => match validate_payload(&candidate) {
                    Ok(validated) => return Ok(validated),
                    Err(e) => {
                        last_error = format!(
                            "invalid state (content_len={content_len}, preview=\"{}\"): {e}",
                            Self::content_preview(&raw)
                        );
                    }
                },
                None => {
                    last_error = format!(
                        "consolidator returned no JSON object (content_len={content_len}, preview=\"{}\")",
                        Self::content_preview(&raw)
                    );
                }
            }
            if attempt < INITIAL_CONSOLIDATION_ATTEMPTS {
                tracing::warn!(
                    attempt,
                    content_len,
                    "initial consolidation produced no valid state; retrying once"
                );
            }
        }
        Err(ConsolidationError::Invalid(last_error))
    }

    /// Tolerantly extract a JSON object from an LLM response (it may be wrapped
    /// in prose or markdown fences). Returns `None` when there is no object.
    fn extract_json_object(content: &str) -> Option<serde_json::Value> {
        let trimmed = content.trim();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.is_object() {
                return Some(value);
            }
        }
        let start = trimmed.find('{')?;
        let end = trimmed.rfind('}')?;
        if end <= start {
            return None;
        }
        serde_json::from_str::<serde_json::Value>(&trimmed[start..=end])
            .ok()
            .filter(serde_json::Value::is_object)
    }

    /// Persist the memory card: store in `memory`, generate embedding,
    /// store in `vec_memory`, and update the indexed messages.
    async fn persist(
        db: &SqlitePool,
        embedding_provider: &Arc<dyn EmbeddingProvider>,
        card: &MemoryCard,
        primary: &[UnindexedMessage],
        persistent: Option<&PersistentWrite>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Build the canonical ficha text
        let ficha = format!(
            "- FECHA/CONTEXTO: {}\n- TEMAS TRATADOS: {}\n- HECHOS Y DECISIONES: {}\n- SÍNTESIS: {}",
            card.date_context, card.topics, card.facts, card.synthesis,
        );

        let tokens_count = estimate_markdown_tokens_heuristic(&ficha);

        // Metadata: include the primary message IDs as reference.
        //
        // The card also records **when the facts happened**, not just when it
        // was written: `first_message_at` is the `created_at` of the oldest
        // origin message and `last_message_at` that of the newest. `primary` is
        // already ordered chronologically (`query_unindexed_messages` uses
        // `ORDER BY created_at ASC`), so the first and last slices are exactly
        // the oldest and newest origin messages.
        let primary_ids: Vec<&str> = primary.iter().map(|m| m.id.as_str()).collect();
        let first_message_at = primary
            .first()
            .map(|m| m.created_at.clone())
            .unwrap_or_default();
        let last_message_at = primary
            .last()
            .map(|m| m.created_at.clone())
            .unwrap_or_default();
        let metadata = serde_json::json!({
            "source": "episodic_worker",
            "primary_message_ids": primary_ids,
            "date_context": card.date_context,
            "first_message_at": first_message_at,
            "last_message_at": last_message_at,
        });

        // 1. Generate embedding first (network call, no DB writes on failure).
        let embedding = match embedding_provider.embed(&ficha).await {
            Ok(emb) => emb,
            Err(e) => {
                return Err(format!("embedding generation failed: {}", e).into());
            }
        };
        let embedding_json = serde_json::to_string(&embedding)?;

        // 2. Persist `memory` + `vec_memory` + message updates atomically.
        let mut tx = db.begin().await?;

        let memory = MemoryRepo::create_in_tx(&mut tx, &ficha, tokens_count, &metadata).await?;

        sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES (?1, vec_f32(?2))")
            .bind(&memory.id)
            .bind(&embedding_json)
            .execute(&mut *tx)
            .await?;

        // Layer C: the consolidated state (when not rejected by the ceiling).
        if let Some(write) = persistent {
            PersistentMemoryRepo::upsert_in_tx(&mut tx, &write.payload, &write.updated_at).await?;
        }

        for msg in primary {
            sqlx::query("UPDATE messages SET is_indexed = 1, summary_ref = ?1 WHERE id = ?2")
                .bind(&memory.id)
                .bind(&msg.id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        tracing::info!(
            memory_id = %memory.id,
            primary_count = %primary.len(),
            tokens = %tokens_count,
            "EpisodicMemoryWorker: persisted memory card"
        );

        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Tests (RED → GREEN)
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use crate::embeddings::provider::EmbeddingError;
    use crate::llm::provider::{
        ChatMessage, ChatRequest, ChatResponse, LLMError, ReasoningEffort, ReasoningSpec,
        ResponseFormat, TokenUsage,
    };
    use async_trait::async_trait;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::{Arc, Mutex};

    // ─── Mock LLM Provider ────────────────────────────────────────────────

    /// A valid default consolidated state returned by the semantic (Layer C)
    /// call.
    const DEFAULT_STATE_RESPONSE: &str =
        r#"{"schema_version":1,"user_profile":{"note":"test"},"system_rules":["regla de prueba"]}"#;

    struct MockEpisodicLLM {
        pub chat_calls: Arc<Mutex<Vec<ChatRequest>>>,
        pub chat_response: String,
        pub state_response: String,
        pub compression_response: String,
        pub fail_semantic: bool,
        pub state_sequence: Arc<Mutex<std::collections::VecDeque<String>>>,
    }

    #[async_trait]
    impl LLMProvider for MockEpisodicLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            let system_content = request
                .messages
                .first()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            self.chat_calls.lock().unwrap().push(request);

            let is_semantic = system_content.contains(SEMANTIC_CALL_MARKER)
                || system_content.contains(COMPRESSION_CALL_MARKER);

            let content = if is_semantic {
                if self.fail_semantic {
                    return Err(LLMError::HttpError("semantic backend down".into()));
                }
                if system_content.contains(COMPRESSION_CALL_MARKER) {
                    self.compression_response.clone()
                } else {
                    let popped = self.state_sequence.lock().unwrap().pop_front();
                    popped.unwrap_or_else(|| self.state_response.clone())
                }
            } else {
                self.chat_response.clone()
            };

            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content,
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: Some(TokenUsage {
                    prompt_tokens: 500,
                    completion_tokens: 200,
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                    cost: 0.0,
                }),
            })
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> Result<
            std::pin::Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<crate::llm::provider::StreamEvent, LLMError>,
                        > + Send,
                >,
            >,
            LLMError,
        > {
            unimplemented!("chat_stream not used in tests")
        }
    }

    impl MockEpisodicLLM {
        fn new(chat_response: &str) -> Self {
            Self {
                chat_calls: Arc::new(Mutex::new(Vec::new())),
                chat_response: chat_response.to_string(),
                state_response: DEFAULT_STATE_RESPONSE.to_string(),
                compression_response: DEFAULT_STATE_RESPONSE.to_string(),
                fail_semantic: false,
                state_sequence: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            }
        }

        /// Override the consolidator response.
        fn semantic(mut self, state: &str) -> Self {
            self.state_response = state.to_string();
            self
        }

        /// Override the compression response.
        fn compression(mut self, compressed: &str) -> Self {
            self.compression_response = compressed.to_string();
            self
        }

        /// Queue per-call consolidator responses (first call first). Falls back to
        /// `state_response` once the queue is exhausted.
        fn state_sequence(mut self, responses: Vec<&str>) -> Self {
            self.state_sequence = Arc::new(Mutex::new(
                responses.into_iter().map(String::from).collect(),
            ));
            self
        }

        /// Make every semantic (consolidator/compression) call fail.
        fn failing_semantic(mut self) -> Self {
            self.fail_semantic = true;
            self
        }

        fn wrap(self) -> Arc<dyn LLMProvider> {
            Arc::new(self)
        }
    }

    // ─── Mock Embedding Provider ──────────────────────────────────────────

    struct MockEmbeddingProvider {
        pub embed_calls: Arc<Mutex<Vec<String>>>,
        pub embed_response: Vec<f32>,
        pub embed_error: bool,
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbeddingProvider {
        async fn embed(&self, input: &str) -> Result<Vec<f32>, EmbeddingError> {
            self.embed_calls.lock().unwrap().push(input.to_string());
            if self.embed_error {
                return Err(EmbeddingError::Api("embedding backend down".into()));
            }
            Ok(self.embed_response.clone())
        }
    }

    impl MockEmbeddingProvider {
        fn new() -> Self {
            Self {
                embed_calls: Arc::new(Mutex::new(Vec::new())),
                embed_response: v1024(&[0.1, 0.2, 0.3]),
                embed_error: false,
            }
        }

        fn wrap(self) -> Arc<dyn EmbeddingProvider> {
            Arc::new(self)
        }
    }

    /// Pad a leading slice to the 1024 dimensions the `vec0` table declares.
    fn v1024(leading: &[f32]) -> Vec<f32> {
        let mut v = leading.to_vec();
        v.resize(1024, 0.0);
        v
    }

    /// Convenience: a fresh mock embedding provider that succeeds.
    fn embedding_provider() -> Arc<dyn EmbeddingProvider> {
        MockEmbeddingProvider::new().wrap()
    }

    // ─── Test helpers ─────────────────────────────────────────────────────

    async fn test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory database");
        run_migrations(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    /// Fresh per-instance rate-limiter for a test call to `evaluate`.
    fn no_attempt() -> Arc<AtomicI64> {
        Arc::new(AtomicI64::new(0))
    }

    /// Insert a message directly for testing, with full control over fields.
    async fn insert_message(
        pool: &SqlitePool,
        role: &str,
        content: &str,
        tokens_count: usize,
        is_indexed: bool,
        created_at: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO messages (id, role, content, tokens_count, is_indexed, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(role)
        .bind(content)
        .bind(tokens_count as i64)
        .bind(is_indexed)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("Failed to insert test message");
        id
    }

    async fn count_unindexed(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE is_indexed = 0")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_memory(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM memory")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_vec_memory(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM vec_memory")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn count_persistent_memory(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM persistent_memory")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn set_persistent_budget(pool: &SqlitePool, value: &str) {
        crate::db::repos::settings::SettingsRepo::set(
            pool,
            "PERSISTENT_MEMORY_BUDGET_TOKENS",
            value,
        )
        .await
        .unwrap();
    }

    /// A valid Layer C state big enough to exceed a small budget.
    fn big_state() -> String {
        let rules: Vec<String> = (0..120)
            .map(|i| {
                format!(
                    "Regla número {i} con texto suficiente para inflar el recuento de tokens del estado persistente"
                )
            })
            .collect();
        serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "user_profile": {"note": "perfil de prueba"},
            "system_rules": rules,
        }))
        .unwrap()
    }

    const SAMPLE_LLM_RESPONSE: &str = "\
- FECHA/CONTEXTO: 26 de septiembre de 2026 — Configuración de infraestructura
- TEMAS TRATADOS: Podman, Docker Compose, PostgreSQL, configuración de red, variables de entorno
- HECHOS Y DECISIONES: Se configuró Podman con docker-compose.yml. Se expuso el puerto 5432 para PostgreSQL. Se decidió usar la red `valet_net` con driver bridge. Se estableció la variable `POSTGRES_DB=valet`.
- SÍNTESIS: El equipo configuró el entorno de desarrollo con Podman, definiendo los servicios de base de datos y aplicación en un docker-compose.yml. Se resolvieron problemas de conexión entre contenedores ajustando las redes virtuales.";

    // ─── 3.1 / 3.2: Worker loop ───────────────────────────────────────────

    #[tokio::test]
    async fn test_worker_loop_receives_channel_signal() {
        let db = test_db().await;
        let (memory_tx, memory_rx) = mpsc::channel::<()>(16);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        // Insert some unindexed messages so evaluate has work to do
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Message {}", i), 500, false, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let _handle = EpisodicMemoryWorker::start(
            db.clone(),
            provider,
            embedding_provider(),
            memory_rx,
            shutdown_rx,
            EpisodicMemoryConfig {
                poll_interval_minutes: 999, // long interval to avoid timer trigger
                ..Default::default()
            },
        );

        // Send signal
        memory_tx.send(()).await.unwrap();

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // The LLM must have been called at least once
        let calls = chat_calls.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "LLM provider chat() should have been called after channel signal"
        );

        // Clean up
        let _ = shutdown_tx.send(());
    }

    #[tokio::test]
    async fn test_worker_loop_shutdown() {
        let db = test_db().await;
        let (_, memory_rx) = mpsc::channel::<()>(16);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();

        let handle = EpisodicMemoryWorker::start(
            db.clone(),
            mock,
            embedding_provider(),
            memory_rx,
            shutdown_rx,
            EpisodicMemoryConfig::default(),
        );

        // Send shutdown
        shutdown_tx.send(()).unwrap();

        // The handle should complete (join) within a reasonable time
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

        assert!(
            result.is_ok(),
            "Worker handle should complete after shutdown signal"
        );
    }

    // ─── 3.3 / 3.4: Query unindexed messages ─────────────────────────────

    #[tokio::test]
    async fn test_evaluate_selects_unindexed_messages() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        // 5 unindexed messages of 500 tokens each (2500 total > 2000 batch)
        for i in 0..5 {
            insert_message(
                &db,
                "user",
                &format!("Message content {}", i),
                500,
                false,
                &now,
            )
            .await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        let calls = chat_calls.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "LLM should have been called when unindexed messages exist"
        );
    }

    #[tokio::test]
    async fn test_evaluate_no_unindexed_messages() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        // All indexed
        for i in 0..3 {
            insert_message(&db, "user", &format!("Message {}", i), 100, true, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        let calls = chat_calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "LLM should NOT be called when all messages are indexed"
        );
    }

    // ─── Batch size threshold ────────────────────────────────────────────

    #[tokio::test]
    async fn test_evaluate_batch_size_threshold() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        // 7 messages of 300 tokens each = 2100 >= 2000 batch
        for i in 0..7 {
            insert_message(&db, "user", &format!("Message {}", i), 300, false, &now).await;
        }

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        assert_eq!(count_memory(&db).await, 1, "Should create one memory card");
    }

    #[tokio::test]
    async fn test_evaluate_below_batch_threshold_and_recent() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        // 3 messages of 200 tokens each = 600 < 2000 batch, and recent
        for i in 0..3 {
            insert_message(
                &db,
                "user",
                &format!("Recent short msg {}", i),
                200,
                false,
                &now,
            )
            .await;
        }

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                batch_tokens: 2000,
                inactivity_minutes: 30,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        assert_eq!(
            count_memory(&db).await,
            0,
            "Should NOT create memory when batch is below threshold and recent"
        );
    }

    // ─── Inactivity timeout ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_evaluate_inactivity_triggers_batch() {
        let db = test_db().await;
        let old_time = (chrono::Utc::now() - chrono::Duration::minutes(45)).to_rfc3339();

        // 3 messages of 200 tokens each (600 < 2000 batch), but old
        for i in 0..3 {
            insert_message(
                &db,
                "user",
                &format!("Old message {}", i),
                200,
                false,
                &old_time,
            )
            .await;
        }

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                batch_tokens: 2000,
                inactivity_minutes: 30,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        assert_eq!(
            count_memory(&db).await,
            1,
            "Should create memory card via inactivity trigger"
        );
    }

    // ─── 3.5 / 3.6: Batch with overlap ───────────────────────────────────

    #[tokio::test]
    async fn test_evaluate_batch_with_overlap() {
        let db = test_db().await;
        let base_time = chrono::Utc::now() - chrono::Duration::hours(1);

        // Overlap BEFORE (already indexed)
        insert_message(
            &db,
            "user",
            "Overlap before 1",
            100,
            true,
            &(base_time - chrono::Duration::minutes(10)).to_rfc3339(),
        )
        .await;
        insert_message(
            &db,
            "assistant",
            "Overlap before 2",
            100,
            true,
            &(base_time - chrono::Duration::minutes(9)).to_rfc3339(),
        )
        .await;

        // 7 PRIMARY messages (unindexed, 300 tokens each = 2100 total)
        for i in 0..7 {
            insert_message(
                &db,
                if i % 2 == 0 { "user" } else { "assistant" },
                &format!("Primary message {}", i),
                300,
                false,
                &(base_time + chrono::Duration::minutes(i)).to_rfc3339(),
            )
            .await;
        }

        // Overlap AFTER
        insert_message(
            &db,
            "user",
            "Overlap after 1",
            100,
            false,
            &(base_time + chrono::Duration::minutes(10)).to_rfc3339(),
        )
        .await;
        insert_message(
            &db,
            "assistant",
            "Overlap after 2",
            100,
            false,
            &(base_time + chrono::Duration::minutes(11)).to_rfc3339(),
        )
        .await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                batch_tokens: 2000,
                overlap: 2,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        let calls = chat_calls.lock().unwrap();
        assert!(!calls.is_empty(), "LLM should have been called");

        // The system message must contain overlap markers
        let request = &calls[0];
        let system_msg = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .expect("Should have a system message");

        assert!(
            system_msg.content.contains("CONTEXTO ANTERIOR (overlap)"),
            "Should include overlap BEFORE markers"
        );
        assert!(
            system_msg.content.contains("CONTEXTO POSTERIOR (overlap)"),
            "Should include overlap AFTER markers"
        );
        assert!(
            system_msg.content.contains("BLOQUE PRINCIPAL A ARCHIVAR"),
            "Should include primary block markers"
        );

        // Verify specific overlap messages are in the block
        assert!(
            system_msg.content.contains("Overlap before 1"),
            "Should contain overlap before message"
        );
        assert!(
            system_msg.content.contains("Overlap after 1"),
            "Should contain overlap after message"
        );
    }

    // ─── 3.7 / 3.8: LLM generates ficha ───────────────────────────────────

    #[tokio::test]
    async fn test_evaluate_parses_llm_response() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        for i in 0..5 {
            insert_message(&db, "user", &format!("Message {}", i), 500, false, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        {
            let calls = chat_calls.lock().unwrap();
            assert!(!calls.is_empty(), "LLM should have been called");
        }

        // Memory card should have been created
        assert_eq!(count_memory(&db).await, 1, "One memory card should exist");

        // The content must contain parsed sections
        let (memories, _total) = MemoryRepo::list(&db, 10, 0).await.unwrap();
        assert_eq!(memories.len(), 1);
        let content = &memories[0].content;
        assert!(content.contains("Podman"), "Should contain parsed topic");
        assert!(
            content.contains("FECHA/CONTEXTO"),
            "Should contain date context section"
        );
        assert!(
            content.contains("SÍNTESIS"),
            "Should contain synthesis section"
        );
    }

    #[tokio::test]
    async fn test_evaluate_unparseable_response_does_not_create_memory() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        for i in 0..5 {
            insert_message(&db, "user", &format!("Message {}", i), 500, false, &now).await;
        }

        let provider = MockEpisodicLLM::new("Esto no tiene el formato esperado.").wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        assert_eq!(
            count_memory(&db).await,
            0,
            "No memory should be created for unparseable response"
        );
    }

    // ─── 3.9 / 3.10: Persistencia completa ───────────────────────────────

    #[tokio::test]
    async fn test_evaluate_persists_memory_embedding_and_updates() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();

        // 7 unindexed messages of 300 tokens each (2100 > 2000 batch)
        let mut msg_ids = Vec::new();
        for i in 0..7 {
            let id = insert_message(&db, "user", &format!("Message {}", i), 300, false, &now).await;
            msg_ids.push(id);
        }

        // Also insert an overlap BEFORE message (indexed) — should remain unchanged
        let overlap_id = insert_message(
            &db,
            "assistant",
            "Earlier context message",
            100,
            true,
            &(chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339(),
        )
        .await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let embed_mock = MockEmbeddingProvider::new();
        let embed_calls = embed_mock.embed_calls.clone();
        let embedding_provider = embed_mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider,
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        // 1. LLM chat was called
        assert!(
            !chat_calls.lock().unwrap().is_empty(),
            "LLM chat should have been called"
        );

        // 2. Embedding was generated via the EmbeddingProvider
        {
            let embed_calls = embed_calls.lock().unwrap();
            assert!(
                !embed_calls.is_empty(),
                "EmbeddingProvider embed should have been called"
            );
            assert!(
                embed_calls[0].contains("FECHA/CONTEXTO"),
                "Embedding input should be the ficha text"
            );
        }

        // 3. Memory row exists
        assert_eq!(count_memory(&db).await, 1, "Should have one memory row");

        // 4. Vec_memory row exists
        assert_eq!(
            count_vec_memory(&db).await,
            1,
            "Should have one vec_memory row"
        );

        // Verify embedding content.
        //
        // CHANGED ON PURPOSE (invariant exception 2): `vec0` stores the vector
        // as a binary BLOB, not JSON text, so it is read back as bytes and the
        // first f32 is decoded little-endian. The assertion is the same.
        let emb_bytes: Vec<u8> = sqlx::query_scalar("SELECT embedding FROM vec_memory LIMIT 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            emb_bytes.len(),
            1024 * 4,
            "Embedding vector should be 1024 f32s (4096 bytes)"
        );
        let first = f32::from_le_bytes(emb_bytes[0..4].try_into().unwrap());
        assert!(
            (first - 0.1).abs() < 0.01,
            "First embedding component should be 0.1"
        );

        // 5. All 7 primary messages are now indexed
        assert_eq!(
            count_unindexed(&db).await,
            0,
            "All messages should be indexed"
        );

        // Verify each primary message has is_indexed = 1 and summary_ref set
        for msg_id in &msg_ids {
            let row = sqlx::query("SELECT is_indexed, summary_ref FROM messages WHERE id = ?1")
                .bind(msg_id)
                .fetch_one(&db)
                .await
                .expect("Message should exist");

            let is_indexed: bool = row.get(0);
            let summary_ref: Option<String> = row.get(1);

            assert!(is_indexed, "Message {} should be indexed", msg_id);
            assert!(
                summary_ref.is_some(),
                "Message {} should have summary_ref set",
                msg_id
            );
        }

        // 6. Overlap message (already indexed) must NOT have its is_indexed state changed
        let overlap_row = sqlx::query("SELECT is_indexed, summary_ref FROM messages WHERE id = ?1")
            .bind(&overlap_id)
            .fetch_one(&db)
            .await
            .expect("Overlap message should exist");

        let overlap_indexed: bool = overlap_row.get(0);
        assert!(
            overlap_indexed,
            "Overlap message should still be indexed (was indexed before)"
        );
    }

    // ─── Block 2: first_message_at / last_message_at (RED → GREEN) ────────

    /// The `EpisodicMemoryWorker` SHALL derive, from
    /// `metadata.primary_message_ids`, the `created_at` of the oldest and the
    /// newest origin messages and write them into the card's `metadata` as
    /// `first_message_at` and `last_message_at`.
    #[tokio::test]
    async fn test_persist_records_first_and_last_message_at_from_origin_messages() {
        let db = test_db().await;

        // A batch of origin messages spanning the real interval from the
        // proposal: 2026-09-29T17:13:00Z → 2026-09-30T17:37:00Z.
        let times = [
            "2026-09-29T17:13:00Z",
            "2026-09-29T20:00:00Z",
            "2026-09-30T09:15:00Z",
            "2026-09-30T12:00:00Z",
            "2026-09-30T17:37:00Z",
        ];
        for (i, t) in times.iter().enumerate() {
            insert_message(&db, "user", &format!("Mensaje {}", i), 100, false, t).await;
        }

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();

        // `inactivity_minutes: 0` forces the batch to be processed regardless of
        // the wall clock, and 5×100 < 2000 tokens keeps every message in the
        // primary batch, so all of them count as origin messages.
        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                inactivity_minutes: 0,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        let (memories, _) = MemoryRepo::list(&db, 10, 0).await.unwrap();
        assert_eq!(memories.len(), 1, "exactly one card must be persisted");
        let metadata = &memories[0].metadata;

        assert_eq!(
            metadata["first_message_at"], "2026-09-29T17:13:00Z",
            "first_message_at must be the oldest origin message's created_at"
        );
        assert_eq!(
            metadata["last_message_at"], "2026-09-30T17:37:00Z",
            "last_message_at must be the newest origin message's created_at"
        );
    }

    /// Adding the two new keys SHALL NOT lose or rename anything: `source`,
    /// `primary_message_ids` and `date_context` keep their exact values, and
    /// they coexist with `first_message_at` / `last_message_at`.
    #[tokio::test]
    async fn test_persist_message_timestamps_do_not_displace_existing_metadata_keys() {
        let db = test_db().await;

        // Insert origin messages and remember their ids, in chronological order.
        let times = [
            "2026-09-29T17:13:00Z",
            "2026-09-30T08:00:00Z",
            "2026-09-30T17:37:00Z",
        ];
        let mut expected_ids = Vec::new();
        for (i, t) in times.iter().enumerate() {
            let id = insert_message(&db, "user", &format!("Mensaje {}", i), 100, false, t).await;
            expected_ids.push(id);
        }

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();
        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                inactivity_minutes: 0,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        let (memories, _) = MemoryRepo::list(&db, 10, 0).await.unwrap();
        assert_eq!(memories.len(), 1, "exactly one card must be persisted");
        let metadata = memories[0]
            .metadata
            .as_object()
            .expect("metadata must be a JSON object");

        // The pre-existing keys keep their exact values...
        assert_eq!(
            metadata["source"], "episodic_worker",
            "`source` must keep its value"
        );
        let stored_ids: Vec<String> =
            serde_json::from_value(metadata["primary_message_ids"].clone())
                .expect("`primary_message_ids` must remain an array of strings");
        assert_eq!(
            stored_ids, expected_ids,
            "`primary_message_ids` must keep the origin ids, in order"
        );
        assert_eq!(
            metadata["date_context"], "26 de septiembre de 2026 — Configuración de infraestructura",
            "`date_context` must keep the LLM-written date text"
        );

        // ...they are all present, together with the two new keys...
        for key in [
            "source",
            "primary_message_ids",
            "date_context",
            "first_message_at",
            "last_message_at",
        ] {
            assert!(
                metadata.contains_key(key),
                "`{key}` must be present in the metadata"
            );
        }

        // ...and nothing was lost or renamed: the object has exactly these keys.
        assert_eq!(
            metadata.len(),
            5,
            "the metadata must hold exactly the three original keys plus the two new ones"
        );
    }

    // ─── Archivist prompt from settings ──────────────────────────────────

    #[tokio::test]
    async fn test_call_llm_uses_custom_archivist_prompt_and_substitutes_placeholder() {
        let db = test_db().await;
        crate::db::repos::settings::SettingsRepo::set(
            &db,
            "archivist_prompt",
            "CUSTOM ARCHIVIST {{ BLOQUE_DE_MENSAJES }}",
        )
        .await
        .unwrap();

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::call_llm(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "MSG_BLOCK_123",
        )
        .await;

        let calls = chat_calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "LLM should have been called once");
        assert_eq!(
            calls[0].messages[0].content, "CUSTOM ARCHIVIST MSG_BLOCK_123",
            "The custom archivist prompt must be used and the placeholder substituted"
        );
    }

    #[tokio::test]
    async fn test_call_llm_falls_back_when_archivist_prompt_missing() {
        let db = test_db().await;
        // The migration seeds `archivist_prompt`; remove it to force the fallback.
        crate::db::repos::settings::SettingsRepo::delete(&db, "archivist_prompt")
            .await
            .unwrap();

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::call_llm(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "MSG_BLOCK_123",
        )
        .await;

        let calls = chat_calls.lock().unwrap();
        assert!(!calls.is_empty(), "LLM should have been called");
        let expected =
            DEFAULT_ARCHIVIST_PROMPT_FALLBACK.replace("{{ BLOQUE_DE_MENSAJES }}", "MSG_BLOCK_123");
        assert_eq!(
            calls[0].messages[0].content, expected,
            "A minimal fallback must be used, with the placeholder substituted"
        );
    }

    // ─── Additional: parse_memory_card unit tests ────────────────────────

    #[tokio::test]
    async fn test_parse_memory_card_full() {
        let card = EpisodicMemoryWorker::parse_memory_card(SAMPLE_LLM_RESPONSE);
        assert!(card.is_some(), "Should parse valid response");
        let card = card.unwrap();

        assert!(
            !card.date_context.is_empty(),
            "date_context should not be empty"
        );
        assert!(card.date_context.contains("infraestructura"));
        assert!(card.topics.contains("Podman"));
        assert!(card.topics.contains("PostgreSQL"));
        assert!(card.facts.contains("5432"));
        assert!(card.facts.contains("docker-compose.yml"));
        assert!(!card.synthesis.is_empty(), "synthesis should not be empty");
    }

    #[tokio::test]
    async fn test_parse_memory_card_empty() {
        let card = EpisodicMemoryWorker::parse_memory_card("");
        assert!(card.is_none(), "Empty input should return None");
    }

    #[tokio::test]
    async fn test_parse_memory_card_invalid() {
        let card =
            EpisodicMemoryWorker::parse_memory_card("This is just random text without sections.");
        assert!(card.is_none(), "Invalid input should return None");
    }

    // ─── 3.1 / 3.2: cooldown follows the spec: max(poll_interval/2, 30s) ──

    /// Given a batch that yields a valid card whose `persist()` fails,
    /// when `evaluate()` is called again within the cooldown,
    /// then the LLM SHALL NOT be called again.
    #[tokio::test]
    async fn test_persist_failure_does_not_retry_llm_within_cooldown() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Message {}", i), 500, false, &now).await;
        }

        // LLM chat succeeds but embedding (persist) fails.
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let chat_calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let embedding_provider = MockEmbeddingProvider {
            embed_error: true,
            ..MockEmbeddingProvider::new()
        }
        .wrap();

        let last = Arc::new(AtomicI64::new(0));
        let config = EpisodicMemoryConfig {
            poll_interval_minutes: 120, // cooldown = max(120*60/2, 30) = 3600 s
            ..Default::default()
        };

        // First evaluate: LLM called once, then persist fails.
        EpisodicMemoryWorker::evaluate(
            &db,
            provider.clone(),
            embedding_provider.clone(),
            &config,
            &last,
        )
        .await;
        assert_eq!(
            chat_calls.lock().unwrap().len(),
            2,
            "the first evaluate must make the two extractions (episodic + consolidator)"
        );

        // Second evaluate within the cooldown must NOT re-call the LLM.
        EpisodicMemoryWorker::evaluate(&db, provider, embedding_provider, &config, &last).await;
        assert_eq!(
            chat_calls.lock().unwrap().len(),
            2,
            "a persist failure must start the cooldown: no second pass"
        );
    }

    /// Given the `vec_memory` insert fails, when `persist()` runs,
    /// then the `memory` table SHALL NOT contain an orphan row.
    #[tokio::test]
    async fn test_persist_rolls_back_memory_when_vec_memory_fails() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Message {}", i), 500, false, &now).await;
        }

        // Force the vec_memory insert to fail by removing the table.
        sqlx::query("DROP TABLE vec_memory")
            .execute(&db)
            .await
            .expect("dropping vec_memory should succeed");

        let provider = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).wrap();
        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig::default(),
            &no_attempt(),
        )
        .await;

        assert_eq!(
            count_memory(&db).await,
            0,
            "a failed vec_memory insert must not leave an orphan row in memory"
        );
    }

    #[test]
    fn test_cooldown_secs_follows_spec() {
        let mk = |poll| EpisodicMemoryConfig {
            poll_interval_minutes: poll,
            ..Default::default()
        };

        // poll_interval = 120 min → 120*60/2 = 3600 s
        assert_eq!(cooldown_secs(&mk(120)), 3600);
        // poll_interval = 10 min → 10*60/2 = 300 s
        assert_eq!(cooldown_secs(&mk(10)), 300);
        // poll_interval = 30 min → 30*60/2 = 900 s
        assert_eq!(cooldown_secs(&mk(30)), 900);
        // poll_interval = 1 min → 30 s (floor applies)
        assert_eq!(cooldown_secs(&mk(1)), 30, "cooldown must floor at 30 s");
        // poll_interval = 0 min → 30 s (floor applies)
        assert_eq!(cooldown_secs(&mk(0)), 30, "cooldown must floor at 30 s");
    }

    // ─── Bloque 4: consolidación, presupuesto y techo ──────────────────────

    #[tokio::test]
    async fn test_consolidate_within_budget_writes_without_compression() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("consolidation must succeed");

        match outcome {
            Consolidation::Write { payload, .. } => assert_eq!(payload["schema_version"], 1),
            other => panic!("expected Write, got {other:?}"),
        }
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "within budget: only the consolidation call, no compression"
        );
    }

    #[tokio::test]
    async fn test_consolidate_over_budget_compresses_once_and_keeps_compressed() {
        let db = test_db().await;
        let small: serde_json::Value = serde_json::from_str(DEFAULT_STATE_RESPONSE).unwrap();
        let small_tokens = payload_token_count(&small);
        let big: serde_json::Value = serde_json::from_str(&big_state()).unwrap();
        assert!(
            payload_token_count(&big) > small_tokens,
            "precondition: the big state exceeds the budget"
        );
        set_persistent_budget(&db, &small_tokens.to_string()).await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(&big_state())
            .compression(DEFAULT_STATE_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("consolidation must succeed");

        match outcome {
            Consolidation::Write { payload, .. } => assert_eq!(payload, small),
            other => panic!("expected Write, got {other:?}"),
        }
        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "over budget: exactly one compression pass"
        );
    }

    #[tokio::test]
    async fn test_consolidate_above_ceiling_keeps_previous() {
        let db = test_db().await;
        PersistentMemoryRepo::upsert(&db, DEFAULT_STATE_RESPONSE, "2026-09-01T10:00:00Z")
            .await
            .unwrap();
        // Budget 2 ⇒ ceiling 4; the compressed state can't get that small.
        set_persistent_budget(&db, "2").await;
        let small: serde_json::Value = serde_json::from_str(DEFAULT_STATE_RESPONSE).unwrap();
        assert!(
            payload_token_count(&small) > 4,
            "precondition: the compressed state exceeds the ceiling"
        );

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(&big_state())
            .compression(DEFAULT_STATE_RESPONSE);
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("a ceiling rejection is not a failure");

        assert_eq!(outcome, Consolidation::KeepPrevious);

        // The previous state is untouched.
        let stored = PersistentMemoryRepo::get(&db).await.unwrap().unwrap();
        assert_eq!(stored.payload, DEFAULT_STATE_RESPONSE);
        assert_eq!(stored.updated_at, "2026-09-01T10:00:00Z");
    }

    #[tokio::test]
    async fn test_consolidate_between_budget_and_ceiling_stores_with_warning() {
        let db = test_db().await;
        let small: serde_json::Value = serde_json::from_str(DEFAULT_STATE_RESPONSE).unwrap();
        let tokens = payload_token_count(&small);
        assert!(tokens >= 2, "precondition for the ceiling formula");
        // budget = tokens - 1 ⇒ compressed is over budget but under the ceiling.
        set_persistent_budget(&db, &(tokens - 1).to_string()).await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(&big_state())
            .compression(DEFAULT_STATE_RESPONSE);
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("consolidation must succeed");

        match outcome {
            Consolidation::Write { payload, .. } => assert_eq!(payload, small),
            other => panic!("expected Write, got {other:?}"),
        }
    }

    /// M1(a) — a compression failure with an uncompressed state that still fits
    /// under the ceiling: store the uncompressed state (with a warning) and
    /// never abort.
    #[tokio::test]
    async fn test_compression_failure_under_ceiling_stores_uncompressed() {
        let db = test_db().await;
        let state: serde_json::Value = serde_json::from_str(&big_state()).unwrap();
        let tokens = payload_token_count(&state);
        // Budget in (tokens/2, tokens): over budget, but the uncompressed state
        // fits under the ceiling (2× budget).
        let budget = tokens * 3 / 4;
        assert!(tokens > budget, "precondition: over budget");
        assert!(
            tokens <= crate::persistent_memory::absolute_ceiling(budget),
            "precondition: uncompressed fits under the ceiling"
        );
        set_persistent_budget(&db, &budget.to_string()).await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(&big_state())
            .compression("esto no es JSON");
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("size management must never abort the pass");

        match outcome {
            Consolidation::Write { payload, .. } => assert_eq!(payload, state),
            other => panic!("expected the uncompressed state to be stored, got {other:?}"),
        }
    }

    /// M1(b) — a compression failure with an uncompressed state above the
    /// ceiling: keep the previous state (with a warning) and never abort.
    #[tokio::test]
    async fn test_compression_failure_over_ceiling_keeps_previous() {
        let db = test_db().await;
        PersistentMemoryRepo::upsert(&db, DEFAULT_STATE_RESPONSE, "2026-09-01T10:00:00Z")
            .await
            .unwrap();
        let state: serde_json::Value = serde_json::from_str(&big_state()).unwrap();
        let tokens = payload_token_count(&state);
        let budget = tokens / 4; // ceiling = tokens/2, below the uncompressed size
        assert!(
            tokens > crate::persistent_memory::absolute_ceiling(budget),
            "precondition: uncompressed exceeds the ceiling"
        );
        set_persistent_budget(&db, &budget.to_string()).await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(&big_state())
            .compression("esto no es JSON");
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("size management must never abort the pass");

        assert_eq!(outcome, Consolidation::KeepPrevious);
        let stored = PersistentMemoryRepo::get(&db).await.unwrap().unwrap();
        assert_eq!(
            stored.payload, DEFAULT_STATE_RESPONSE,
            "previous state kept"
        );
        assert_eq!(stored.updated_at, "2026-09-01T10:00:00Z");
    }

    #[tokio::test]
    async fn test_consolidate_semantic_failure_is_failure() {
        let db = test_db().await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).failing_semantic();
        let provider = mock.wrap();

        let result = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await;
        assert!(result.is_err(), "a semantic LLM error must fail the pass");
    }

    #[tokio::test]
    async fn test_consolidate_invalid_schema_is_failure() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).semantic(r#"{"schema_version":2}"#);
        let provider = mock.wrap();

        let result = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await;
        assert!(
            result.is_err(),
            "an unsupported schema version must fail the pass"
        );
    }

    /// The initial consolidation is retried once: empty content then valid JSON succeeds.
    #[tokio::test]
    async fn test_consolidate_retries_once_on_empty_then_succeeds() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .state_sequence(vec!["", DEFAULT_STATE_RESPONSE]);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("the single retry must succeed");

        assert!(matches!(outcome, Consolidation::Write { .. }));
        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "empty then valid: exactly two calls"
        );
    }

    /// Two invalid attempts abort and the error carries the content length.
    #[tokio::test]
    async fn test_consolidate_two_invalid_attempts_abort_with_content_len() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).state_sequence(vec!["", ""]);
        let provider = mock.wrap();

        let err = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect_err("two empty attempts must fail");

        let msg = err.to_string();
        assert!(
            msg.contains("content_len"),
            "error must include content_len: {msg}"
        );
        assert!(
            msg.contains("content_len=0"),
            "the empty attempt must report its length: {msg}"
        );
        assert!(
            msg.contains("preview="),
            "the error must include a preview: {msg}"
        );
    }

    /// A non-empty but non-JSON response is retried and its text appears in the preview.
    #[tokio::test]
    async fn test_consolidate_error_preview_includes_content() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .state_sequence(vec!["lo siento, no puedo", "lo siento, no puedo"]);
        let provider = mock.wrap();

        let err = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect_err("a non-JSON response must fail after the retry");

        let msg = err.to_string();
        assert!(
            msg.contains("content_len="),
            "the error must report content_len: {msg}"
        );
        assert!(
            msg.contains("lo siento, no puedo"),
            "the preview must include the raw content: {msg}"
        );
    }

    /// An LLM transport error is not retried: it propagates on the first attempt.
    #[tokio::test]
    async fn test_consolidate_transport_error_is_not_retried() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).failing_semantic();
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let err = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect_err("a transport error must abort the consolidation");

        assert!(
            matches!(err, ConsolidationError::Llm(_)),
            "expected a transport error, got {err}"
        );
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "a transport error must not be retried"
        );
    }

    /// An invalid schema on the first attempt is retried too.
    #[tokio::test]
    async fn test_consolidate_retries_on_invalid_schema_then_succeeds() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).state_sequence(vec![
            r#"{"schema_version":1,"user_profile":"oops"}"#,
            DEFAULT_STATE_RESPONSE,
        ]);
        let provider = mock.wrap();

        let outcome = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("the retry must succeed after an invalid schema");

        assert!(matches!(outcome, Consolidation::Write { .. }));
    }

    /// The compression pass is never retried (exactly one compression call).
    #[tokio::test]
    async fn test_compression_is_not_retried() {
        let db = test_db().await;
        let small_tokens = payload_token_count(
            &serde_json::from_str::<serde_json::Value>(DEFAULT_STATE_RESPONSE).unwrap(),
        );
        set_persistent_budget(&db, &small_tokens.to_string()).await;
        let big = big_state();
        // The initial consolidation is forced through its single retry (empty
        // then oversized valid state) so that reaching the compression pass
        // already depends on the retry path; the compression itself must then
        // run exactly once.
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .state_sequence(vec!["", &big])
            .compression("");
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let _ = EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await;

        let compression_calls = calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                r.messages
                    .first()
                    .map(|m| m.content.contains(COMPRESSION_CALL_MARKER))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(compression_calls, 1, "compression must not be retried");
    }

    // ─── Bloque 5: pasada unificada ────────────────────────────────────────

    #[tokio::test]
    async fn test_unified_pass_writes_card_state_and_marks_in_one_transaction() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Mensaje {i}"), 100, false, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::evaluate(
            &db,
            provider,
            embedding_provider(),
            &EpisodicMemoryConfig {
                inactivity_minutes: 0,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        assert_eq!(count_memory(&db).await, 1, "one Layer B card");
        assert_eq!(count_vec_memory(&db).await, 1, "the card embedding");
        assert_eq!(count_persistent_memory(&db).await, 1, "one Layer C state");
        assert_eq!(count_unindexed(&db).await, 0, "all messages marked");
        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "the pass makes two extractions (B and C)"
        );
    }

    /// M1(c) — a ceiling rejection at pass level keeps the previous state but
    /// still writes the episodic card and marks the batch.
    #[tokio::test]
    async fn test_ceiling_rejection_still_writes_card_and_marks_batch() {
        let db = test_db().await;
        // Budget 1 ⇒ ceiling 2: any consolidated state is rejected, but the
        // pass must continue.
        set_persistent_budget(&db, "1").await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Mensaje {i}"), 100, false, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        EpisodicMemoryWorker::evaluate(
            &db,
            mock.wrap(),
            embedding_provider(),
            &EpisodicMemoryConfig {
                inactivity_minutes: 0,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        assert_eq!(count_memory(&db).await, 1, "the episodic card is written");
        assert_eq!(
            count_persistent_memory(&db).await,
            0,
            "the state is rejected: nothing overwritten"
        );
        assert_eq!(count_unindexed(&db).await, 0, "the batch is marked");
    }

    #[tokio::test]
    async fn test_consolidator_failure_writes_nothing_and_retry_dedupes() {
        let db = test_db().await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Mensaje {i}"), 100, false, &now).await;
        }
        let config = EpisodicMemoryConfig {
            inactivity_minutes: 0,
            ..Default::default()
        };

        // Episodic card succeeds, consolidation fails: nothing is written.
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE).failing_semantic();
        EpisodicMemoryWorker::evaluate(
            &db,
            mock.wrap(),
            embedding_provider(),
            &config,
            &no_attempt(),
        )
        .await;

        assert_eq!(count_memory(&db).await, 0, "no orphan card");
        assert_eq!(count_persistent_memory(&db).await, 0, "no state");
        assert_eq!(count_unindexed(&db).await, 5, "nothing marked");

        // A successful retry produces exactly ONE card (no duplicates).
        let retry = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        EpisodicMemoryWorker::evaluate(
            &db,
            retry.wrap(),
            embedding_provider(),
            &config,
            &no_attempt(),
        )
        .await;

        assert_eq!(count_memory(&db).await, 1, "exactly one card after retry");
        assert_eq!(count_persistent_memory(&db).await, 1, "state written");
        assert_eq!(count_unindexed(&db).await, 0, "messages marked");
    }

    #[tokio::test]
    async fn test_both_extractions_record_stats_with_null_profile() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        let now = chrono::Utc::now().to_rfc3339();
        for i in 0..5 {
            insert_message(&db, "user", &format!("Mensaje {i}"), 100, false, &now).await;
        }

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        EpisodicMemoryWorker::evaluate(
            &db,
            mock.wrap(),
            embedding_provider(),
            &EpisodicMemoryConfig {
                inactivity_minutes: 0,
                ..Default::default()
            },
            &no_attempt(),
        )
        .await;

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(total, 2, "one stats row per extraction");

        let null_profiles: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests WHERE profile_id IS NULL")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            null_profiles, 2,
            "both stats rows must carry a NULL profile"
        );
    }

    // ─── Bloque 5.5: prompt del consolidador ───────────────────────────────

    #[tokio::test]
    async fn test_consolidate_uses_custom_prompt_and_substitutes_placeholders() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        crate::db::repos::settings::SettingsRepo::set(
            &db,
            "consolidator_prompt",
            "Eres el consolidador de memoria persistente. CUSTOM_ESTADO={{ ESTADO_ACTUAL }};MSGS={{ BLOQUE_DE_MENSAJES }}",
        )
        .await
        .unwrap();

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE123",
        )
        .await
        .expect("consolidation must succeed");

        let calls = calls.lock().unwrap();
        let sys = &calls[0].messages[0].content;
        assert!(sys.starts_with("Eres el consolidador de memoria persistente."));
        assert!(sys.contains("CUSTOM_ESTADO="));
        assert!(sys.contains("MSGS=BLOQUE123"));
        assert!(
            !sys.contains("{{ ESTADO_ACTUAL }}"),
            "placeholder substituted"
        );
        assert!(
            !sys.contains("{{ BLOQUE_DE_MENSAJES }}"),
            "placeholder substituted"
        );
        assert!(
            sys.contains("\"schema_version\":1"),
            "the current (empty) state is substituted"
        );
    }

    #[tokio::test]
    async fn test_consolidate_falls_back_when_prompt_missing_or_empty() {
        let db = test_db().await;
        set_persistent_budget(&db, "100000").await;
        crate::db::repos::settings::SettingsRepo::delete(&db, "consolidator_prompt")
            .await
            .unwrap();

        // Missing.
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();
        EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE123",
        )
        .await
        .expect("fallback consolidation must succeed");

        {
            let calls = calls.lock().unwrap();
            let sys = &calls[0].messages[0].content;
            assert!(sys.contains(SEMANTIC_CALL_MARKER), "fallback prompt in use");
            assert!(!sys.contains("{{ ESTADO_ACTUAL }}"));
            assert!(!sys.contains("{{ BLOQUE_DE_MENSAJES }}"));
            assert!(sys.contains("BLOQUE123"));
        }

        // Empty (whitespace only).
        crate::db::repos::settings::SettingsRepo::set(&db, "consolidator_prompt", "   ")
            .await
            .unwrap();
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();
        EpisodicMemoryWorker::consolidate_state(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE456",
        )
        .await
        .expect("fallback consolidation must succeed");

        let calls = calls.lock().unwrap();
        let sys = &calls[0].messages[0].content;
        assert!(sys.contains(SEMANTIC_CALL_MARKER), "fallback prompt in use");
        assert!(sys.contains("BLOQUE456"));
    }

    // ─── Contract tests: parámetros de generación por rol ──────────────────

    /// Run the episodic-card extraction once and return the captured request.
    async fn fichas_request(db: &SqlitePool) -> ChatRequest {
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let _ = EpisodicMemoryWorker::call_llm(
            db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await;

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "the card extraction must call the LLM once");
        calls[0].clone()
    }

    /// Scenario: Las fichas no razonan con los defaults (0.3 / off / 1024).
    #[tokio::test]
    async fn test_fichas_use_generation_defaults() {
        let db = test_db().await;

        let request = fichas_request(&db).await;

        assert_eq!(request.temperature, Some(0.3));
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "default card reasoning must be Off, got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(1024));
    }

    /// Scenario: Las fichas toman sus tres parámetros de settings
    #[tokio::test]
    async fn test_fichas_read_generation_params_from_settings() {
        let db = test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_MEMORY_TEMPERATURE", "0.45")
            .await
            .unwrap();
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_MEMORY_REASONING", "high")
            .await
            .unwrap();
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_MEMORY_MAX_TOKENS", "777")
            .await
            .unwrap();

        let request = fichas_request(&db).await;

        assert_eq!(request.temperature, Some(0.45));
        assert!(
            matches!(
                request.reasoning,
                Some(ReasoningSpec::Effort(ReasoningEffort::High))
            ),
            "expected Effort(High), got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(777));
    }

    /// Run the consolidator once and return the captured semantic request.
    async fn consolidation_request(db: &SqlitePool) -> ChatRequest {
        set_persistent_budget(db, "100000").await;
        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        EpisodicMemoryWorker::consolidate_state(
            db,
            &provider,
            &EpisodicMemoryConfig::default(),
            "BLOQUE",
        )
        .await
        .expect("consolidation must succeed");

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "within budget: only the consolidation call");
        calls[0].clone()
    }

    /// Scenario: El consolidador pide JSON y usa 0.1 / off / 2048 por defecto.
    #[tokio::test]
    async fn test_consolidator_forces_json_and_uses_semantic_defaults() {
        let db = test_db().await;

        let request = consolidation_request(&db).await;

        assert!(
            matches!(request.response_format, Some(ResponseFormat::JsonObject)),
            "the consolidator must always request JSON, got {:?}",
            request.response_format
        );
        assert_eq!(request.temperature, Some(0.1));
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "default consolidator reasoning must be Off, got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(2048));
    }

    /// Scenario: El modo JSON no se puede desactivar desde settings
    #[tokio::test]
    async fn test_consolidator_json_mode_not_configurable() {
        let db = test_db().await;
        crate::db::repos::settings::SettingsRepo::set(
            &db,
            "GENERATION_SEMANTIC_TEMPERATURE",
            "0.9",
        )
        .await
        .unwrap();
        crate::db::repos::settings::SettingsRepo::set(
            &db,
            "GENERATION_SEMANTIC_REASONING",
            "super",
        )
        .await
        .unwrap();
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_SEMANTIC_MAX_TOKENS", "11")
            .await
            .unwrap();

        let request = consolidation_request(&db).await;

        assert!(
            matches!(request.response_format, Some(ResponseFormat::JsonObject)),
            "no settings combination may disable JSON mode"
        );
        assert_eq!(request.temperature, Some(0.9));
        assert_eq!(request.max_tokens, Some(11));
    }

    /// Scenario: La compresión pide JSON y usa GENERATION_SEMANTIC_*.
    #[tokio::test]
    async fn test_compression_forces_json_and_uses_semantic_params() {
        let db = test_db().await;
        let state: serde_json::Value = serde_json::from_str(DEFAULT_STATE_RESPONSE).unwrap();

        let mock = MockEpisodicLLM::new(SAMPLE_LLM_RESPONSE)
            .semantic(DEFAULT_STATE_RESPONSE)
            .compression(DEFAULT_STATE_RESPONSE);
        let calls = mock.chat_calls.clone();
        let provider = mock.wrap();

        let _ = EpisodicMemoryWorker::try_compress(
            &db,
            &provider,
            &EpisodicMemoryConfig::default(),
            &state,
        )
        .await;

        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            1,
            "the compression pass must call the LLM once"
        );
        let request = calls[0].clone();
        assert!(
            matches!(request.response_format, Some(ResponseFormat::JsonObject)),
            "the compression pass must request JSON, got {:?}",
            request.response_format
        );
        assert_eq!(request.temperature, Some(0.1));
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "compression must use Off by default, got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(2048));
    }

    /// Scenario: Una temperatura no parseable cae al default con warning.
    #[tokio::test]
    async fn test_fichas_unparseable_temperature_falls_back_to_default() {
        let db = test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_MEMORY_TEMPERATURE", "alta")
            .await
            .unwrap();

        let request = fichas_request(&db).await;
        assert_eq!(
            request.temperature,
            Some(0.3),
            "an unparseable temperature must fall back to the role default"
        );
    }

    /// Scenario: Un razonamiento desconocido cae a off con warning.
    #[tokio::test]
    async fn test_fichas_unknown_reasoning_falls_back_to_off() {
        let db = test_db().await;
        crate::db::repos::settings::SettingsRepo::set(&db, "GENERATION_MEMORY_REASONING", "super")
            .await
            .unwrap();

        let request = fichas_request(&db).await;
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "an unknown reasoning level must fall back to Off, got {:?}",
            request.reasoning
        );
    }
}
