use crate::db::repos::memory::MemoryRepo;
use crate::embeddings::EmbeddingProvider;
use crate::llm::provider::ChatMessage;
use crate::models::Memory;
use crate::orchestrator::context_classifier::ContextStrategy;
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct BuiltContext {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub token_estimate: usize,
    pub rag_memories: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Profile not found")]
    ProfileNotFound,
    #[error("Search error: {0}")]
    SearchError(String),
    #[error("Window error: {0}")]
    WindowError(String),
}

pub struct ContextBuilder {
    pub pool: Option<SqlitePool>,
    pub provider: Option<Arc<dyn EmbeddingProvider>>,
    pub rag_budget_tokens: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self {
            pool: None,
            provider: None,
            rag_budget_tokens: 800,
        }
    }

    pub async fn build(
        &self,
        strategy: ContextStrategy,
        _profile_id: &str,
        user_message: &str,
    ) -> Result<BuiltContext, ContextError> {
        // Episodic memory is retrieved for **every** message, independently of
        // the context strategy (spec «La estrategia de contexto SHALL NOT
        // gobernar la memoria episódica»). The strategy only shapes the system
        // prompt and the token estimate now; it no longer enables or disables
        // retrieval.
        let rag_memories = self.build_rag_memories(user_message).await;
        let memory_len: usize = rag_memories.iter().map(|m| m.len()).sum();

        match strategy {
            ContextStrategy::SlidingWindow => Ok(BuiltContext {
                system_prompt: "You are Valet, a helpful AI assistant.".into(),
                messages: vec![],
                token_estimate: 500 + memory_len,
                rag_memories,
            }),
            ContextStrategy::Historical => Ok(BuiltContext {
                system_prompt: "You are Valet, analyzing historical data.".into(),
                messages: vec![],
                token_estimate: 5000 + memory_len,
                rag_memories,
            }),
        }
    }

    /// Perform a real vector-similarity search and assemble the memory block.
    ///
    /// Retrieval (KNN, similarity threshold, temporal decay, ordering) is done
    /// by [`MemoryRepo::search_by_vector`]. This method owns only the **prompt
    /// budget**: it reads `RAG_BUDGET_TOKENS` from `settings` on every call
    /// (falling back to the configured `rag_budget_tokens`) and accumulates
    /// `tokens_count` until the next card would overflow the budget, at which
    /// point it stops with `break` — a card that does not fit must not let a
    /// later, less relevant card slip in for being smaller (design D6).
    ///
    /// Returns `Vec::new()` (never placeholder memories) when the pool or the
    /// embedding provider is missing, or when the embedding/search fails.
    async fn build_rag_memories(&self, user_message: &str) -> Vec<String> {
        let (Some(pool), Some(provider)) = (&self.pool, &self.provider) else {
            tracing::warn!("RAG: pool or embedding provider not configured; returning no memories");
            return Vec::new();
        };

        let embedding = match provider.embed(user_message).await {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!("RAG: embedding generation failed: {e}");
                return Vec::new();
            }
        };

        let memories = match MemoryRepo::search_by_vector(pool, &embedding).await {
            Ok(memories) => memories,
            Err(e) => {
                tracing::warn!("RAG: vector search failed: {e}");
                return Vec::new();
            }
        };

        let budget_tokens = read_rag_budget_tokens(pool)
            .await
            .unwrap_or(self.rag_budget_tokens);

        let mut results = Vec::new();
        let mut running_tokens: usize = 0;
        for memory in memories {
            if running_tokens + memory.tokens_count > budget_tokens {
                // Step 7 (D6): stop, do NOT keep scanning for a smaller card.
                break;
            }
            running_tokens += memory.tokens_count;
            results.push(format_memory(&memory));
        }
        results
    }
}

/// Read `RAG_BUDGET_TOKENS` from `settings`, so it can be changed in the UI
/// without a restart. Returns `None` when it is absent or unparseable, so the
/// caller can fall back to the statically configured value.
async fn read_rag_budget_tokens(pool: &SqlitePool) -> Option<usize> {
    crate::db::repos::settings::SettingsRepo::get(pool, "RAG_BUDGET_TOKENS")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<usize>().ok())
}

/// Format a `Memory` card for the injected block.
///
/// Returns the card's content **as is**, with no date in front (design D3): the
/// chronology is already given by the `FECHA/CONTEXTO` line the LLM writes into
/// the content, which is meant to be read; a machine date is for machine
/// decisions (the ranking), not for the prompt, and lies after a
/// reconstruction. The old `[{tags}]` prefix is also gone: `EpisodicMemoryWorker`
/// never writes a `tags` key into `metadata` (production: 7 cards, 0 with
/// `tags`), so that prefix always rendered as a constant empty `[]` that cost
/// tokens without informing anything. See the spec requirements «El formato de
/// ficha SHALL NOT depender de metadata ausente» and the `orchestrator/agent`
/// requirement «Orden por decaimiento de cada ficha».
fn format_memory(m: &Memory) -> String {
    m.content.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use crate::embeddings::provider::EmbeddingError;
    use async_trait::async_trait;
    use sqlx::sqlite::SqlitePoolOptions;

    /// In-memory pool with all migrations applied.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create in-memory pool");
        run_migrations(&pool).await.expect("migrations failed");
        pool
    }

    // ─── Mock embedding provider ─────────────────────────────────────────

    /// Pad a leading slice to the 1024 dimensions the `vec0` table declares.
    fn v1024(leading: &[f32]) -> Vec<f32> {
        let mut v = leading.to_vec();
        v.resize(1024, 0.0);
        v
    }

    struct MockEmbedProvider;

    #[async_trait]
    impl EmbeddingProvider for MockEmbedProvider {
        async fn embed(&self, _input: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(v1024(&[0.1, 0.2, 0.3]))
        }
    }

    // ─── Existing tests (must stay GREEN) ─────────────────────────────────

    #[tokio::test]
    async fn test_sliding_window_context() -> Result<(), Box<dyn std::error::Error>> {
        let builder = ContextBuilder::new();
        let ctx = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "hello")
            .await?;
        assert!(ctx.system_prompt.contains("Valet"));
        assert!(ctx.token_estimate <= 2000);
        Ok(())
    }

    #[tokio::test]
    async fn test_historical_context() -> Result<(), Box<dyn std::error::Error>> {
        let builder = ContextBuilder::new();
        let ctx = builder
            .build(ContextStrategy::Historical, "profile-1", "history")
            .await?;
        assert!(ctx.token_estimate >= 1000);
        Ok(())
    }

    // The `!doc`/`RAG` strategy no longer exists (block 9.1). These tests used
    // to invoke `build()` through `ContextStrategy::RAG`; that vehicle is gone,
    // so they now drive the same assertions through `SlidingWindow`. This is a
    // mechanical substitution of a removed enum variant, not an assertion
    // change: what each test pins is unchanged.

    #[tokio::test]
    async fn test_memory_context_without_pool_is_empty() -> Result<(), Box<dyn std::error::Error>> {
        let builder = ContextBuilder::new();
        let ctx = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "search")
            .await?;
        // No pool/provider → no placeholder memories, just an empty vec.
        assert!(ctx.rag_memories.is_empty());
        Ok(())
    }

    // ─── Memory retrieval tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_memory_without_pool() {
        let builder = ContextBuilder::new();
        let ctx = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "any")
            .await
            .expect("build should succeed even without pool");
        // Without pool → no placeholder memories.
        assert!(
            ctx.rag_memories.is_empty(),
            "expected empty rag_memories when pool is None"
        );
    }

    #[tokio::test]
    async fn test_memory_with_pool_no_provider() {
        let pool = setup_pool().await;
        let builder = ContextBuilder {
            pool: Some(pool),
            provider: None,
            rag_budget_tokens: 2000,
        };
        let ctx = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "any")
            .await
            .expect("build should succeed with pool but no provider");
        // No provider → no placeholder memories.
        assert!(
            ctx.rag_memories.is_empty(),
            "expected empty rag_memories when provider is None"
        );
    }

    #[tokio::test]
    async fn test_memory_empty_db() {
        let pool = setup_pool().await;
        let provider = Arc::new(MockEmbedProvider);
        let builder = ContextBuilder {
            pool: Some(pool),
            provider: Some(provider),
            rag_budget_tokens: 2000,
        };
        let ctx = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "search")
            .await
            .expect("build should succeed with empty vec_memory");
        // No memories stored → search_by_vector returns empty vec
        assert!(
            ctx.rag_memories.is_empty(),
            "expected empty rag_memories when vec_memory is empty"
        );
    }

    /// Insert an embedding row directly into `vec_memory` for tests.
    ///
    /// CHANGED ON PURPOSE (invariant exception 2): stored through
    /// `vec_f32(?)` with the declared 1024 dimensions, instead of raw JSON text
    /// in the old regular table.
    async fn insert_embedding(pool: &SqlitePool, id: &str, embedding: &[f32]) {
        let json = serde_json::to_string(embedding).expect("failed to serialize embedding");
        sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES (?1, vec_f32(?2))")
            .bind(id)
            .bind(&json)
            .execute(pool)
            .await
            .expect("failed to insert vec_memory row");
    }

    /// 8.1 — a card with no `tags` in `metadata` must not render empty `[]`
    /// brackets, and the injected text is the card content, with no date.
    ///
    /// UPDATED (exception 1): this test used to assert that the card carried its
    /// temporal anchor (`created_at`); `episodic-memory-occurred-at` removes the
    /// date from the prompt (D3), so that assertion moved to the new behaviour.
    /// The `[tags]` assertions are unchanged.
    #[test]
    fn format_memory_omits_tags_and_returns_content() {
        let m = Memory {
            id: "m1".into(),
            content: "User likes Rust".into(),
            tokens_count: 10,
            created_at: "2026-09-29T10:00:00+00:00".into(),
            metadata: serde_json::json!({
                "source": "episodic_worker",
                "primary_message_ids": ["a", "b"],
            }),
        };

        let formatted = format_memory(&m);

        assert!(
            !formatted.contains("[]"),
            "a card without `tags` must not produce empty `[]` brackets, got {formatted:?}"
        );
        assert!(
            !formatted.contains("[tags]"),
            "the `[tags]` prefix must be gone, got {formatted:?}"
        );
        assert_eq!(
            formatted, "User likes Rust",
            "the injected text is the card content, with no date, got {formatted:?}"
        );
    }

    /// A card with a `created_at` and a `metadata.last_message_at` is formatted
    /// **without any date**: the injected text is its content, as is.
    #[test]
    fn format_memory_returns_content_without_any_date() {
        let m = Memory {
            id: "m-no-date".into(),
            content: "Contenido sin fecha".into(),
            tokens_count: 5,
            created_at: "2026-09-29T10:00:00Z".into(),
            metadata: serde_json::json!({
                "source": "episodic_worker",
                "primary_message_ids": ["a"],
                "last_message_at": "2026-09-30T17:37:00Z",
            }),
        };

        let formatted = format_memory(&m);

        assert_eq!(
            formatted, "Contenido sin fecha",
            "the injected text must be the card content, with no date"
        );
        assert!(
            !formatted.contains("2026-09-29T10:00:00Z"),
            "no date derived from `created_at` may appear, got {formatted:?}"
        );
        assert!(
            !formatted.contains("2026-09-30T17:37:00Z"),
            "no date derived from `metadata` may appear, got {formatted:?}"
        );
    }

    // ─── Characterization: `format_memory` returns the content as is ────────
    //
    // `episodic-memory-occurred-at` removes, on purpose, the date from the
    // injected card (D3): `format_memory` now returns the content `as is`. The
    // block-1 version of this test pinned the OLD format `[{created_at}] {content}`;
    // the assertion has moved here on purpose and is **exception 1 of 2** to this
    // change's invariant (exception 2 is measuring the decay against
    // `metadata.last_message_at`, in `db::repos::memory`). The test is renamed to
    // match the new behaviour. Any OTHER test that breaks while implementing the
    // change is a regression, not an update.
    //
    /// CHARACTERIZATION: `format_memory` returns the card's content with no date
    /// in front, whether or not the metadata carries a `last_message_at`.
    #[test]
    fn characterization_format_memory_returns_content_without_date() {
        let m = Memory {
            id: "m-char".into(),
            content: "Contenido de la ficha".into(),
            tokens_count: 5,
            created_at: "2026-09-29T10:00:00Z".into(),
            metadata: serde_json::json!({
                "source": "episodic_worker",
                "primary_message_ids": ["a"],
                "last_message_at": "2026-09-30T17:37:00Z",
            }),
        };

        let formatted = format_memory(&m);

        assert_eq!(
            formatted, "Contenido de la ficha",
            "NEW behaviour: the injected text is the card content, with no date"
        );
    }

    /// 8.2 — EXCEPTION 1 of this change's invariant: the formatting assertion
    /// that changes on purpose. It used to pin `[{tags}] {content}`, then
    /// `[{created_at}] {content}`; `episodic-memory-occurred-at` removes the date
    /// from the prompt (D3), so it now pins the bare content.
    #[tokio::test]
    async fn test_memory_with_pool_and_provider_returns_formatted_memories() {
        let pool = setup_pool().await;

        // A real memory card. It deliberately still carries `tags` in metadata
        // to prove the format ignores them now.
        let metadata = serde_json::json!({"tags": ["rust", "backend"]});
        let mem = MemoryRepo::create(&pool, "User likes Rust", 10, &metadata)
            .await
            .expect("create memory should succeed");

        // Embedding matches the mock provider's output dimension (3).
        insert_embedding(&pool, &mem.id, &v1024(&[0.1, 0.2, 0.3])).await;

        let builder = ContextBuilder {
            pool: Some(pool),
            provider: Some(Arc::new(MockEmbedProvider)),
            rag_budget_tokens: 2000,
        };
        let ctx = builder
            .build(
                ContextStrategy::SlidingWindow,
                "profile-1",
                "what does the user like",
            )
            .await
            .expect("build should succeed with pool + provider");

        assert_eq!(
            ctx.rag_memories.len(),
            1,
            "exactly one stored card is retrieved"
        );
        assert_eq!(
            ctx.rag_memories[0], "User likes Rust",
            "rag_memories should contain the card content, with no date"
        );
    }

    // ─── Retrieval is orthogonal to the context strategy (task 8.6) ─────────
    //
    // Block 8 deliberately broke the old premise that `ContextStrategy` decided
    // whether episodic memory was retrieved at all: both `SlidingWindow` and
    // `Historical` now inject memory identically (spec «La estrategia de
    // contexto SHALL NOT gobernar la memoria episódica»). Block 9.1 then removed
    // the `RAG` variant and `!doc`, so these tests can only exercise the two
    // surviving strategies; the assertions are the block-8 ones.
    //
    // ─── The three and only exceptions to the invariant (task 1.3) ──────────
    //
    // These are the ONLY test assertions allowed to change on purpose:
    //   1. `test_memory_with_pool_and_provider_returns_formatted_memories`,
    //      which pinned the `[{tags}] {content}` format (task 8.2).
    //   2. Any test (here and in `db::repos::memory`) that assumes
    //      `vec_memory.embedding` stores JSON text.
    //   3. `test_doc_override` and `test_classify_with_doc` in
    //      `context_classifier`, which disappear along with `!doc` (task 9.2).
    // Any OTHER test that breaks when implementing the change is a regression,
    // not an update.

    /// Build a `ContextBuilder` wired with a pool + provider and seed exactly
    /// one memory card (with tags) plus its `vec_memory` embedding row. The
    /// same builder + state is then reused across the surviving strategies.
    async fn builder_with_one_stored_memory() -> ContextBuilder {
        let pool = setup_pool().await;

        let metadata = serde_json::json!({"tags": ["rust", "backend"]});
        let mem = MemoryRepo::create(&pool, "User likes Rust", 10, &metadata)
            .await
            .expect("create memory should succeed");

        // Embedding matches the mock provider's output dimension (3).
        insert_embedding(&pool, &mem.id, &v1024(&[0.1, 0.2, 0.3])).await;

        ContextBuilder {
            pool: Some(pool),
            provider: Some(Arc::new(MockEmbedProvider)),
            rag_budget_tokens: 2000,
        }
    }

    /// 8.6 / spec «Misma memoria en `SlidingWindow` y `Historical`» — both
    /// surviving strategies retrieve the same episodic memory now that retrieval
    /// is orthogonal to the strategy. `Historical` here takes the place the
    /// removed `RAG` variant used to play.
    #[tokio::test]
    async fn sliding_window_and_historical_retrieve_the_same_memory() {
        let builder = builder_with_one_stored_memory().await;
        let sliding = builder
            .build(
                ContextStrategy::SlidingWindow,
                "profile-1",
                "what does the user like",
            )
            .await
            .expect("build should succeed");
        let historical = builder
            .build(
                ContextStrategy::Historical,
                "profile-1",
                "what does the user like",
            )
            .await
            .expect("build should succeed");

        assert!(
            !sliding.rag_memories.is_empty(),
            "SlidingWindow must retrieve the stored memory"
        );
        assert_eq!(
            sliding.rag_memories, historical.rag_memories,
            "SlidingWindow and Historical must retrieve the same memory"
        );
    }

    /// 8.6 — `Historical` retrieves the stored card. Kept separate from the
    /// equality check above so each strategy is pinned on its own.
    #[tokio::test]
    async fn historical_retrieves_the_stored_memory() {
        let builder = builder_with_one_stored_memory().await;
        let historical = builder
            .build(
                ContextStrategy::Historical,
                "profile-1",
                "what does the user like",
            )
            .await
            .expect("build should succeed");

        assert!(
            !historical.rag_memories.is_empty(),
            "Historical must retrieve the stored memory"
        );
        assert!(
            historical.rag_memories[0].ends_with("User likes Rust"),
            "Historical must return the stored card, got {:?}",
            historical.rag_memories
        );
    }

    /// 8.7 — a plain message classified as `Override::None` / `SlidingWindow`
    /// receives memory when a card clears the threshold. This ties the
    /// classifier to the orthogonal retrieval: "no override" no longer means
    /// "no memory".
    #[tokio::test]
    async fn none_override_sliding_window_message_receives_memory() {
        use crate::orchestrator::context_classifier::{ContextClassifier, Override};

        let builder = builder_with_one_stored_memory().await;
        let classifier = ContextClassifier::new();
        let classification = classifier.classify("Añade leche a la compra");
        assert_eq!(classification.override_cmd, Override::None);
        assert_eq!(classification.strategy, ContextStrategy::SlidingWindow);

        let ctx = builder
            .build(
                classification.strategy,
                "profile-1",
                "Añade leche a la compra",
            )
            .await
            .expect("build should succeed");

        assert!(
            !ctx.rag_memories.is_empty(),
            "a None/SlidingWindow message must receive memory when a card clears the threshold"
        );
    }

    /// UPDATED (was `characterization_only_rag_returns_the_stored_memory`):
    /// the old assertion pinned the `[{tags}] {content}` format (now obsolete,
    /// task 8.2) and the "only RAG retrieves" premise (now obsolete, task 8.6).
    /// It is kept to prove the surviving retrieval path returns the card in the
    /// new `[{created_at}] {content}` format; `RAG` was removed in block 9.1, so
    /// it is driven through `SlidingWindow`.
    #[tokio::test]
    async fn sliding_window_retrieves_the_stored_memory() {
        let builder = builder_with_one_stored_memory().await;
        let ctx = builder
            .build(
                ContextStrategy::SlidingWindow,
                "profile-1",
                "what does the user like",
            )
            .await
            .expect("build should succeed");
        assert_eq!(ctx.rag_memories.len(), 1);
        assert!(
            ctx.rag_memories[0].ends_with("User likes Rust"),
            "SlidingWindow must return the stored memory, got {:?}",
            ctx.rag_memories
        );
    }

    /// `BuiltContext::system_prompt` is **vestigial** in production: `agent.rs`
    /// ignores it and reads the real prompt from `settings.system_prompt`. This
    /// test only records the dead values so their removal is visible; it does
    /// not assert anything about what the LLM receives.
    ///
    /// Still valid after block 10.1: that block removed the
    /// `BuiltContext::session_summary` field (always `None`), not
    /// `system_prompt`. Block 9.1 removed the `RAG` branch, so its dead value
    /// ("You are Valet, using RAG context.") disappeared with it and only the
    /// two surviving strategies are recorded here.
    #[tokio::test]
    async fn characterization_built_context_system_prompt_is_vestigial() {
        let builder = builder_with_one_stored_memory().await;

        let sliding = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "hi")
            .await
            .expect("build should succeed");
        let historical = builder
            .build(ContextStrategy::Historical, "profile-1", "hi")
            .await
            .expect("build should succeed");

        // NOTE: `agent.rs` never sends these strings to the LLM — it reads
        // `settings.system_prompt` instead. Kept only to document the field.
        assert_eq!(
            sliding.system_prompt,
            "You are Valet, a helpful AI assistant."
        );
        assert_eq!(
            historical.system_prompt,
            "You are Valet, analyzing historical data."
        );
    }

    // ─── 6.4 / block 5.4: the prompt budget cuts with `break` ────────────────

    /// Provider that embeds every input to the x-axis, so a stored vector's
    /// cosine similarity to the query is exactly its first component.
    struct AxisEmbedProvider;

    #[async_trait]
    impl EmbeddingProvider for AxisEmbedProvider {
        async fn embed(&self, _input: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(v1024(&[1.0, 0.0, 0.0]))
        }
    }

    /// A unit vector at cosine similarity `s` to `(1, 0, 0)`, padded to 1024.
    fn cosine_vector(s: f32) -> Vec<f32> {
        v1024(&[s, (1.0 - s * s).max(0.0).sqrt(), 0.0])
    }

    async fn set_setting(pool: &SqlitePool, key: &str, value: &str) {
        crate::db::repos::settings::SettingsRepo::set(pool, key, value)
            .await
            .expect("failed to set setting");
    }

    /// Seed a card with its own vector and token count.
    async fn seed_card(pool: &SqlitePool, content: &str, tokens: usize, similarity: f32) -> String {
        let mem = MemoryRepo::create(pool, content, tokens, &serde_json::json!({}))
            .await
            .expect("create memory should succeed");
        insert_embedding(pool, &mem.id, &cosine_vector(similarity)).await;
        mem.id
    }

    fn axis_builder(pool: SqlitePool) -> ContextBuilder {
        ContextBuilder {
            pool: Some(pool),
            provider: Some(Arc::new(AxisEmbedProvider)),
            // Reserve value; the live budget is read from `settings`.
            rag_budget_tokens: 800,
        }
    }

    /// `RAG_BUDGET_TOKENS = 300` and two 237-token cards: the first fits, the
    /// second does not, and the accumulation stops there.
    #[tokio::test]
    async fn test_memory_budget_includes_first_card_not_second() {
        let pool = setup_pool().await;
        set_setting(&pool, "RAG_BUDGET_TOKENS", "300").await;

        seed_card(&pool, "First", 237, 0.99).await;
        seed_card(&pool, "Second", 237, 0.98).await;

        let ctx = axis_builder(pool)
            .build(ContextStrategy::SlidingWindow, "profile-1", "query")
            .await
            .expect("build should succeed");

        assert_eq!(
            ctx.rag_memories.len(),
            1,
            "only the first 237-token card fits in a 300-token budget"
        );
        assert!(
            ctx.rag_memories[0].contains("First"),
            "the fitting card must be the first one, got {:?}",
            ctx.rag_memories
        );
    }

    /// The spec scenario: budget 800 with cards of 347/307/248/245/210/164/138
    /// tokens → exactly 2 cards (347 + 307). The 138-token card must NOT slip
    /// in by skipping the 248-token one — that would be the `continue` bug.
    #[tokio::test]
    async fn test_memory_budget_stops_at_first_card_that_does_not_fit() {
        let pool = setup_pool().await;
        set_setting(&pool, "RAG_BUDGET_TOKENS", "800").await;

        // Decreasing similarity ⇒ deterministic relevance order.
        for (content, tokens, similarity) in [
            ("a", 347usize, 0.99f32),
            ("b", 307, 0.98),
            ("c", 248, 0.97),
            ("d", 245, 0.96),
            ("e", 210, 0.95),
            ("f", 164, 0.94),
            ("g", 138, 0.93),
        ] {
            seed_card(&pool, content, tokens, similarity).await;
        }

        let ctx = axis_builder(pool)
            .build(ContextStrategy::SlidingWindow, "profile-1", "query")
            .await
            .expect("build should succeed");

        assert_eq!(
            ctx.rag_memories.len(),
            2,
            "347 + 307 fit; the next card does not and nothing later may slip in, got {:?}",
            ctx.rag_memories
        );
        assert!(
            ctx.rag_memories[0].contains('a') && ctx.rag_memories[1].contains('b'),
            "the two fitting cards must be the 347 and 307 ones, got {:?}",
            ctx.rag_memories
        );
        assert!(
            !ctx.rag_memories.iter().any(|m| m.contains('g')),
            "the small 138-token card must NOT be included"
        );
    }

    /// The budget is read from `settings` on every call, so changing it takes
    /// effect without a restart and is not governed by the reserve field.
    #[tokio::test]
    async fn test_memory_budget_read_from_settings_takes_effect_without_restart() {
        let pool = setup_pool().await;

        seed_card(&pool, "One", 300, 0.99).await;
        seed_card(&pool, "Two", 300, 0.98).await;

        let builder = axis_builder(pool.clone());

        // Reserve field says 800, but settings says 300 → only one card.
        set_setting(&pool, "RAG_BUDGET_TOKENS", "300").await;
        let tight = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "query")
            .await
            .expect("build should succeed");
        assert_eq!(tight.rag_memories.len(), 1);

        // Raising the setting, same builder, no restart → both cards.
        set_setting(&pool, "RAG_BUDGET_TOKENS", "800").await;
        let wide = builder
            .build(ContextStrategy::SlidingWindow, "profile-1", "query")
            .await
            .expect("build should succeed");
        assert_eq!(wide.rag_memories.len(), 2);
    }
}
