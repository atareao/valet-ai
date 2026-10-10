use crate::config::Config;
use crate::db::DbPool;
use crate::embeddings::EmbeddingProvider;
use crate::workers::episodic_memory::{EpisodicMemoryConfig, EpisodicMemoryWorker};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::llm::provider::LLMProvider;

/// Minimal collapse prompt used only when `settings.collapse_prompt` is
/// missing or empty. The real prompt is seeded by migration
/// `20260929000001_prompts.sql` and can be customised from the UI.
const DEFAULT_COLLAPSE_PROMPT_FALLBACK: &str = "Resume el siguiente texto de forma concisa.";

/// Container for all background worker tasks.
///
/// Each field holds an optional [`JoinHandle`] for the corresponding worker.
/// When `shutdown()` is called, a signal is broadcast to all workers and
/// all handles are aborted.
pub struct WorkerPool {
    pub collapse: Option<JoinHandle<()>>,
    pub collapse_tx: Option<mpsc::Sender<String>>,
    pub memory: Option<JoinHandle<()>>,
    pub memory_tx: Option<mpsc::Sender<()>>,
    pub shutdown_tx: Option<broadcast::Sender<()>>,
}

impl WorkerPool {
    /// Start the background workers: the channel-driven CollapseWorker and
    /// the EpisodicMemoryWorker (which manages its own loop).
    ///
    /// A broadcast channel is created so all workers can be gracefully
    /// stopped.
    pub fn start(
        db: DbPool,
        _config: &Config,
        llm_provider: Arc<dyn LLMProvider>,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // ── Collapse worker ─────────────────────────────────────
        let (collapse_tx, collapse_rx) = mpsc::channel::<String>(256);
        let collapse = {
            let db = db.clone();
            let llm_provider = llm_provider.clone();
            let collapse_model = _config.collapse_model.clone();

            let collapse_prompt = {
                // Read collapse_prompt from settings synchronously for now.
                // The migration seeds the real value; use a minimal fallback
                // if it is missing, empty, or unreadable.
                let db_for_prompt = db.clone();
                let prompt_future = async move {
                    match crate::db::repos::settings::SettingsRepo::get(
                        &db_for_prompt,
                        "collapse_prompt",
                    )
                    .await
                    {
                        Ok(Some(p)) if !p.trim().is_empty() => p,
                        Ok(_) => {
                            tracing::warn!(
                                "settings.collapse_prompt missing or empty; using minimal fallback"
                            );
                            DEFAULT_COLLAPSE_PROMPT_FALLBACK.to_string()
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "failed to read settings.collapse_prompt; using minimal fallback"
                            );
                            DEFAULT_COLLAPSE_PROMPT_FALLBACK.to_string()
                        }
                    }
                };

                // We need to block on this because `start()` is not async.
                // Use tokio::runtime::Handle to run the future on the current runtime.
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    tokio::task::block_in_place(|| handle.block_on(prompt_future))
                } else {
                    DEFAULT_COLLAPSE_PROMPT_FALLBACK.to_string()
                }
            };

            let mut shutdown_rx = shutdown_tx.subscribe();
            tokio::spawn(async move {
                let handle = crate::workers::collapse::CollapseWorker::start(
                    db,
                    llm_provider,
                    collapse_rx,
                    collapse_prompt,
                    collapse_model,
                );
                // Esperar shutdown o que el worker termine
                let _ = shutdown_rx.recv().await;
                handle.abort();
            })
        };

        // ── Episodic memory worker (only when embeddings are configured) ──
        let (memory, memory_tx) = match embedding_provider {
            Some(embedding_provider) => {
                let (memory_tx, memory_rx) = mpsc::channel::<()>(256);
                let shutdown_rx = shutdown_tx.subscribe();
                let db = db.clone();
                let llm_provider = llm_provider.clone();
                let memory_config = EpisodicMemoryConfig {
                    batch_tokens: _config.memory_batch_tokens,
                    inactivity_minutes: _config.memory_inactivity_minutes as i64,
                    overlap: _config.memory_overlap,
                    poll_interval_minutes: _config.memory_poll_interval_minutes,
                    model: _config.memory_model.clone(),
                    semantic_model: _config.semantic_model.clone(),
                    timeline_model: _config.timeline_model.clone(),
                };
                let handle = EpisodicMemoryWorker::start(
                    db,
                    llm_provider,
                    embedding_provider,
                    memory_rx,
                    shutdown_rx,
                    memory_config,
                );
                (Some(handle), Some(memory_tx))
            }
            None => {
                tracing::warn!("EpisodicMemoryWorker not started: embeddings not configured");
                (None, None)
            }
        };

        Self {
            collapse: Some(collapse),
            collapse_tx: Some(collapse_tx),
            memory,
            memory_tx,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Gracefully shut down all workers.
    ///
    /// Sends a shutdown signal through the broadcast channel and aborts all
    /// task handles.
    pub async fn shutdown(&mut self) {
        // Send shutdown signal to all workers
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Abort each handle
        if let Some(handle) = self.collapse.take() {
            handle.abort();
        }
        self.collapse_tx.take();
        if let Some(handle) = self.memory.take() {
            handle.abort();
        }
        self.memory_tx.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db::repos::messages::MessagesRepo;
    use crate::db::schema::run_migrations;
    use crate::embeddings::provider::EmbeddingError;
    use crate::llm::provider::{ChatMessage, ChatRequest, ChatResponse, LLMError, TokenUsage};
    use async_trait::async_trait;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::{Arc, Mutex};

    /// Create a minimal [`DbPool`] with an in-memory database for tests.
    async fn test_db() -> DbPool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory database for test");
        run_migrations(&pool)
            .await
            .expect("Failed to run migrations");
        pool
    }

    /// A mock LLM provider that records chat requests and returns canned responses.
    struct MockPoolLLM {
        pub calls: Arc<Mutex<Vec<ChatRequest>>>,
    }

    #[async_trait]
    impl crate::llm::provider::LLMProvider for MockPoolLLM {
        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
            self.calls.lock().unwrap().push(request);
            Ok(ChatResponse {
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "Resumen del mensaje.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
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

    fn test_llm_provider() -> Arc<dyn crate::llm::provider::LLMProvider> {
        Arc::new(MockPoolLLM {
            calls: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// A no-op embedding provider for pool tests.
    struct MockPoolEmbeddings;

    #[async_trait]
    impl EmbeddingProvider for MockPoolEmbeddings {
        async fn embed(&self, _input: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    fn test_embedding_provider() -> Option<Arc<dyn EmbeddingProvider>> {
        Some(Arc::new(MockPoolEmbeddings))
    }

    /// Create a [`Config`] with default values for testing.
    fn test_config() -> Config {
        Config {
            host: "0.0.0.0".into(),
            port: 3000,
            database_url: ":memory:".into(),
            log_level: "debug".into(),
            openrouter_api_key: None,
            openrouter_model: "anthropic/claude-sonnet-20241022".into(),
            openrouter_base_url: "https://openrouter.ai/api/v1".into(),
            ollama_base_url: "http://localhost:11434".into(),
            ollama_model: "llama3.2:3b".into(),
            auth_enabled: false,
            auth_issuer_url: "http://localhost:8080".into(),
            auth_client_id: String::new(),
            auth_client_secret: String::new(),
            auth_redirect_url: "http://localhost:3000/auth/callback".into(),
            auth_post_logout_redirect_url: "http://localhost:3000".into(),
            jwt_secret: String::new(),
            openweather_api_key: None,
            google_places_api_key: None,
            brave_search_api_key: None,
            collapse_threshold_tokens: 2000,
            collapse_model: "mistralai/mistral-small".into(),
            memory_batch_tokens: 2000,
            memory_inactivity_minutes: 30,
            memory_overlap: 2,
            memory_poll_interval_minutes: 30,
            memory_model: "mistralai/mistral-small".into(),
            semantic_model: "mistralai/mistral-small".into(),
            timeline_model: "mistralai/mistral-small".into(),
            rag_budget_tokens: 2000,
            embedding_provider: None,
            embedding_model: None,
            embedding_dimension: None,
        }
    }

    /// Given a WorkerPool started with a Config that specifies a collapse_model,
    /// when a long message's ID is sent through the collapse channel,
    /// then the real CollapseWorker must process it and set collapsed_content in the DB.
    ///
    /// This test uses real sqllite and sqlx async pool. It requires `SQLX_OFFLINE=true`
    /// or a running database to build queries; the in-memory pool avoids needing a server.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_uses_real_collapse_worker() {
        // Create an in-memory DB with migrations and a long message
        let pool = test_db().await;

        let long_content = "x".repeat(8000);
        let msg = MessagesRepo::create(
            &pool,
            "user",
            &long_content,
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await
        .unwrap();
        let msg_id = msg.id.clone();

        let config = test_config();
        let mut pool_workers = WorkerPool::start(
            pool.clone(),
            &config,
            test_llm_provider(),
            test_embedding_provider(),
        );

        // Send the message ID through the collapse channel
        if let Some(tx) = &pool_workers.collapse_tx {
            tx.send(msg_id.clone()).await.unwrap();
        } else {
            panic!("collape_tx should be Some");
        }

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify the message was processed: collapsed_content should be set
        let processed = MessagesRepo::find_by_id(&pool, &msg_id)
            .await
            .unwrap()
            .expect("Message should exist");

        assert!(
            processed.collapsed_content.is_some(),
            "The real CollapseWorker should have set collapsed_content, \
             but the placeholder only logs — this test will FAIL (RED)"
        );

        pool_workers.shutdown().await;
    }

    /// Given a DB without `collapse_prompt`, when the pool starts and a long
    /// message is sent, the collapse worker must use a minimal fallback prompt.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_falls_back_when_collapse_prompt_missing() {
        let pool = test_db().await;
        crate::db::repos::settings::SettingsRepo::delete(&pool, "collapse_prompt")
            .await
            .unwrap();

        let long_content = "x".repeat(8000);
        let msg = MessagesRepo::create(
            &pool,
            "user",
            &long_content,
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await
        .unwrap();
        let msg_id = msg.id.clone();

        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let provider: Arc<dyn crate::llm::provider::LLMProvider> = Arc::new(MockPoolLLM {
            calls: calls.clone(),
        });

        let config = test_config();
        let mut pool_workers =
            WorkerPool::start(pool.clone(), &config, provider, test_embedding_provider());

        if let Some(tx) = &pool_workers.collapse_tx {
            tx.send(msg_id.clone()).await.unwrap();
        } else {
            panic!("collapse_tx should be Some");
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        {
            let captured = calls.lock().unwrap();
            assert!(
                !captured.is_empty(),
                "Collapse worker should have called the LLM"
            );
            assert_eq!(
                captured[0].messages[0].content, DEFAULT_COLLAPSE_PROMPT_FALLBACK,
                "A minimal fallback must be used when collapse_prompt is missing"
            );
        }

        pool_workers.shutdown().await;
    }

    /// Sanity check: `shutdown_tx` is publicly accessible and is `Some`
    /// immediately after `WorkerPool::start()`.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_worker_pool_shutdown_tx_accessible() {
        let db = test_db().await;
        let config = test_config();
        let mut pool =
            WorkerPool::start(db, &config, test_llm_provider(), test_embedding_provider());

        assert!(
            pool.shutdown_tx.is_some(),
            "shutdown_tx should be Some after WorkerPool::start()"
        );

        pool.shutdown().await;
    }

    /// Given `embedding_provider = None`, the episodic memory worker must not
    /// be started and no `memory_tx` channel should exist.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_pool_without_embeddings_skips_memory_worker() {
        let db = test_db().await;
        let config = test_config();
        let mut pool = WorkerPool::start(db, &config, test_llm_provider(), None);

        assert!(
            pool.memory.is_none(),
            "episodic memory worker must not start without an embedding provider"
        );
        assert!(
            pool.memory_tx.is_none(),
            "no memory channel should be created without an embedding provider"
        );
        // The collapse worker still starts.
        assert!(
            pool.collapse.is_some(),
            "collapse worker should still start"
        );

        pool.shutdown().await;
    }
}
