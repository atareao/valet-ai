use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing;

use crate::db::repos::messages::MessagesRepo;
use crate::db::repos::stats::StatsRepo;
use crate::db::DbPool;
use crate::llm::provider::{ChatMessage, ChatRequest, LLMProvider};
use crate::models::message::estimate_markdown_tokens_heuristic;
use crate::models::stats::CallKind;
use std::time::Instant;

/// Background worker that collapses long messages by sending them to an LLM
/// for summarisation.
///
/// Listens on an `mpsc` channel for message IDs whose estimated token count
/// exceeds the collapse threshold. When a message ID is received, the worker
/// fetches the message's full content from the database, sends it to the LLM
/// with the configured collapse prompt, and stores the resulting summary back
/// in the `collapsed_content` column.
pub struct CollapseWorker;

/// Build a synchronous callback that forwards collapse message IDs to `tx`.
///
/// The callback must not silently discard IDs when the channel is full: it
/// applies backpressure by spawning a task that awaits capacity, falling back
/// to a non-blocking send (with a warning) when no async runtime is available.
pub fn collapse_forwarder(tx: mpsc::Sender<String>) -> Box<dyn Fn(String) + Send> {
    Box::new(move |msg_id: String| {
        let tx = tx.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // Backpressure: wait for capacity instead of dropping the ID.
                //
                // Tradeoff: a `spawn` is issued per ID, so when the channel is
                // saturated the tasks queue up and their delivery order is not
                // guaranteed. This is acceptable because the `CollapseWorker`
                // processes each message ID independently (no cross-message
                // ordering requirement).
                handle.spawn(async move {
                    if let Err(e) = tx.send(msg_id).await {
                        tracing::warn!(
                            error = %e,
                            "CollapseWorker: collapse channel closed; message id not forwarded"
                        );
                    }
                });
            }
            Err(_) => {
                // No async runtime available: fall back to a non-blocking send
                // but never drop the ID without at least a warning.
                if let Err(e) = tx.try_send(msg_id) {
                    tracing::warn!(
                        error = %e,
                        "CollapseWorker: collapse channel full or closed; message id dropped"
                    );
                }
            }
        }
    })
}

impl CollapseWorker {
    /// Start the collapse worker in a new tokio task.
    ///
    /// The worker loops on `rx` forever (or until the channel closes). For
    /// each received message ID it:
    /// 1. Reads the message from the database.
    /// 2. Builds a [`ChatRequest`] with the collapse prompt + message content.
    /// 3. Calls `llm_provider.chat()`.
    /// 4. Writes the LLM response into `collapsed_content`.
    ///
    /// Returns a [`JoinHandle`] that can be awaited or aborted.
    pub fn start(
        db: DbPool,
        llm_provider: Arc<dyn LLMProvider>,
        mut rx: mpsc::Receiver<String>,
        collapse_prompt: String,
        model: String,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(message_id) = rx.recv().await {
                tracing::info!(message_id = %message_id, "CollapseWorker processing message");

                // 1. Read message from DB
                let msg = match MessagesRepo::find_by_id(&db, &message_id).await {
                    Ok(Some(msg)) => Some(msg),
                    _ => None,
                };

                if let Some(msg) = msg {
                    // 2. Build LLM request
                    let generation = crate::generation::read_generation_params(
                        &db,
                        crate::generation::GenerationRole::Collapse,
                    )
                    .await;
                    let request = ChatRequest {
                        model: model.clone(),
                        messages: vec![
                            ChatMessage {
                                role: "system".to_string(),
                                content: collapse_prompt.clone(),
                                tool_calls: None,
                                tool_result: None,
                                tool_call_id: None,
                            },
                            ChatMessage {
                                role: "user".to_string(),
                                content: msg.content.clone(),
                                tool_calls: None,
                                tool_result: None,
                                tool_call_id: None,
                            },
                        ],
                        tools: None,
                        temperature: Some(generation.temperature),
                        max_tokens: Some(generation.max_tokens),
                        stream: false,
                        reasoning: generation.reasoning,
                        response_format: None,
                    };

                    // 3. Call LLM
                    let start = Instant::now();
                    match llm_provider.chat(request).await {
                        Ok(response) => {
                            let duration_ms = start.elapsed().as_millis() as i64;
                            let collapsed = response.message.content;
                            let collapsed_tokens = estimate_markdown_tokens_heuristic(&collapsed);

                            // Record stats
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
                                &db,
                                CallKind::Collapse,
                                &uuid::Uuid::new_v4().to_string(),
                                &model,
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

                            // 4. Update DB
                            let update_result = sqlx::query(
                                "UPDATE messages SET collapsed_content = ?1, collapsed_tokens_count = ?2 WHERE id = ?3",
                            )
                             .bind(&collapsed)
                             .bind(collapsed_tokens as i64)
                             .bind(&message_id)
                             .execute(&db)
                             .await;

                            if let Err(e) = update_result {
                                tracing::error!(message_id = %message_id, error = %e, "CollapseWorker failed to update DB");
                            } else {
                                tracing::info!(message_id = %message_id, collapsed_tokens = %collapsed_tokens, "CollapseWorker completed");
                            }
                        }
                        Err(e) => {
                            let duration_ms = start.elapsed().as_millis() as i64;
                            let _ = StatsRepo::record_request(
                                &db,
                                CallKind::Collapse,
                                &uuid::Uuid::new_v4().to_string(),
                                &model,
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
                            tracing::error!(message_id = %message_id, error = %e, "CollapseWorker LLM call failed");
                        }
                    }
                } else {
                    tracing::warn!(message_id = %message_id, "CollapseWorker: message not found");
                }
            }
            tracing::info!("CollapseWorker shutting down");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::messages::MessagesRepo;
    use crate::db::schema::run_migrations;
    use crate::llm::provider::{
        ChatMessage, ChatRequest, ChatResponse, LLMError, ReasoningSpec, TokenUsage,
    };
    use async_trait::async_trait;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::sync::{Arc, Mutex};

    /// A mock LLM provider that records every [`ChatRequest`] it receives and
    /// returns a canned response.
    struct MockLLMProvider {
        pub calls: Arc<Mutex<Vec<ChatRequest>>>,
    }

    #[async_trait]
    impl LLMProvider for MockLLMProvider {
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

    /// Helper: create an in-memory DbPool with migrations.
    async fn test_db() -> Result<DbPool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await?;
        run_migrations(&pool).await.unwrap();
        Ok(pool)
    }

    /// Helper: create a long message (> 2000 tokens) and return its id.
    async fn create_long_message(pool: &DbPool) -> Result<String, sqlx::Error> {
        let long_content = "x".repeat(8000);
        let msg = MessagesRepo::create(
            pool,
            "user",
            &long_content,
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await?;
        Ok(msg.id)
    }

    /// Given a running CollapseWorker with a mock LLM provider,
    /// when a message ID is sent through the channel,
    /// then the LLM provider's `chat()` must be called with the correct request.
    #[tokio::test]
    async fn test_worker_receives_message_id_and_calls_llm(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        let (tx, rx) = mpsc::channel::<String>(16);
        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(MockLLMProvider {
            calls: calls.clone(),
        });

        let prompt = "Resume el siguiente mensaje en español.".to_string();
        let _handle = CollapseWorker::start(
            pool.clone(),
            mock,
            rx,
            prompt,
            "mistralai/mistral-small".to_string(),
        );

        // Send message ID through channel
        tx.send(msg_id.clone()).await.unwrap();

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // The LLM must have been called
        let call_count = calls.lock().unwrap().len();
        assert!(
            call_count > 0,
            "LLM provider was not called – worker did not process the message"
        );

        // The request must contain the collapse prompt as system message
        let request = calls.lock().unwrap()[0].clone();
        assert!(
            request.messages.iter().any(|m| m.role == "system"),
            "Request should contain a system message with the collapse prompt"
        );
        Ok(())
    }

    /// Given a running CollapseWorker,
    /// when a message is collapsed,
    /// then the database must have its `collapsed_content` updated.
    #[tokio::test]
    async fn test_worker_updates_collapsed_content_in_db() -> Result<(), Box<dyn std::error::Error>>
    {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        let (tx, rx) = mpsc::channel::<String>(16);
        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(MockLLMProvider { calls });

        let prompt = "Resume el siguiente mensaje.".to_string();
        let _handle = CollapseWorker::start(
            pool.clone(),
            mock,
            rx,
            prompt,
            "mistralai/mistral-small".to_string(),
        );

        // Send message ID
        tx.send(msg_id.clone()).await.unwrap();

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Verify the DB was updated
        let msg = MessagesRepo::find_by_id(&pool, &msg_id)
            .await?
            .expect("Message should exist");

        assert!(
            msg.collapsed_content.is_some(),
            "collapsed_content should be set after worker processes the message"
        );
        assert_eq!(
            msg.collapsed_content.as_deref(),
            Some("Resumen del mensaje."),
            "collapsed_content should match the mock LLM response"
        );
        assert!(
            msg.collapsed_tokens_count > 0,
            "collapsed_tokens_count should be > 0"
        );
        Ok(())
    }

    /// Given a custom collapse prompt,
    /// when the worker processes a message,
    /// then the prompt sent to the LLM must match the configured one.
    #[tokio::test]
    async fn test_worker_uses_custom_prompt() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        let (tx, rx) = mpsc::channel::<String>(16);
        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(MockLLMProvider {
            calls: calls.clone(),
        });

        let custom_prompt = "CUSTOM: Summarize this in one sentence.".to_string();
        let _handle = CollapseWorker::start(
            pool.clone(),
            mock,
            rx,
            custom_prompt.clone(),
            "mistralai/mistral-small".to_string(),
        );

        // Send message ID
        tx.send(msg_id.clone()).await.unwrap();

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Verify the system message contains our custom prompt
        let request = calls.lock().unwrap()[0].clone();
        let system_msg = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .expect("Should have a system message");

        assert_eq!(
            system_msg.content, custom_prompt,
            "System message should contain the custom collapse prompt"
        );
        Ok(())
    }

    /// Given a CollapseWorker started with a specific model,
    /// when it processes a message,
    /// then the ChatRequest.model must match the configured model.
    #[tokio::test]
    async fn test_worker_uses_configured_model() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        let (tx, rx) = mpsc::channel::<String>(16);
        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(MockLLMProvider {
            calls: calls.clone(),
        });

        let prompt = "Resume el siguiente mensaje.".to_string();
        let model = "google/gemini-2.0-flash-lite".to_string();
        let _handle = CollapseWorker::start(pool.clone(), mock, rx, prompt, model.clone());

        // Send message ID
        tx.send(msg_id.clone()).await.unwrap();

        // Give the worker time to process
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let request = calls.lock().unwrap()[0].clone();
        assert_eq!(
            request.model, "google/gemini-2.0-flash-lite",
            "ChatRequest.model should use the configured model, not the hardcoded default"
        );
        Ok(())
    }

    /// Given a full collapse channel (capacity 1), when a second ID is
    /// forwarded, then it must not be lost silently: applying backpressure it
    /// must be delivered once capacity frees up.
    #[tokio::test]
    async fn test_collapse_forwarder_applies_backpressure_on_full_channel() {
        let (tx, mut rx) = mpsc::channel::<String>(1);
        tx.send("first".to_string())
            .await
            .expect("send first should succeed");

        let forward = collapse_forwarder(tx);
        forward("second".to_string());

        // Drain the first ID, freeing capacity.
        let first = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await
            .expect("first recv timed out")
            .expect("first ID should be present");
        assert_eq!(first, "first");

        // The second must eventually arrive (backpressure), not be dropped.
        let second = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await
            .expect("second ID was lost: forwarder dropped it when the channel was full")
            .expect("second ID should be present");
        assert_eq!(second, "second");
    }

    // -----------------------------------------------------------------------
    // Contract tests — CollapseWorker reads its generation params from `settings`
    // -----------------------------------------------------------------------

    /// Run the worker once for `msg_id` and return the first captured request.
    async fn collapse_once(
        pool: &DbPool,
        msg_id: String,
    ) -> Result<ChatRequest, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel::<String>(16);
        let calls: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mock = Arc::new(MockLLMProvider {
            calls: calls.clone(),
        });
        let _handle = CollapseWorker::start(
            pool.clone(),
            mock,
            rx,
            "Resume el mensaje.".to_string(),
            "mistralai/mistral-small".to_string(),
        );

        tx.send(msg_id).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let captured = calls.lock().unwrap();
        assert!(!captured.is_empty(), "LLM provider was not called");
        Ok(captured[0].clone())
    }

    /// Scenario: El colapso no razona con los defaults (0.2 / off / 1024).
    #[tokio::test]
    async fn test_collapse_uses_generation_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        let request = collapse_once(&pool, msg_id).await?;

        assert_eq!(
            request.temperature,
            Some(0.2),
            "default collapse temperature must be 0.2"
        );
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "default collapse reasoning must be Off, got {:?}",
            request.reasoning
        );
        assert_eq!(
            request.max_tokens,
            Some(1024),
            "default collapse max_tokens must be 1024"
        );

        Ok(())
    }

    /// Scenario: El colapso toma sus tres parámetros de settings
    #[tokio::test]
    async fn test_collapse_reads_generation_params_from_settings(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "GENERATION_COLLAPSE_TEMPERATURE",
            "0.5",
        )
        .await?;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "GENERATION_COLLAPSE_REASONING",
            "off",
        )
        .await?;
        crate::db::repos::settings::SettingsRepo::set(
            &pool,
            "GENERATION_COLLAPSE_MAX_TOKENS",
            "512",
        )
        .await?;

        let request = collapse_once(&pool, msg_id).await?;

        assert_eq!(request.temperature, Some(0.5));
        assert!(
            matches!(request.reasoning, Some(ReasoningSpec::Off)),
            "expected Off, got {:?}",
            request.reasoning
        );
        assert_eq!(request.max_tokens, Some(512));

        Ok(())
    }

    /// Scenario: Una clave ausente cae al default del rol
    #[tokio::test]
    async fn test_collapse_missing_max_tokens_falls_back_to_default(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = test_db().await?;
        let msg_id = create_long_message(&pool).await?;

        crate::db::repos::settings::SettingsRepo::delete(&pool, "GENERATION_COLLAPSE_MAX_TOKENS")
            .await?;

        let request = collapse_once(&pool, msg_id).await?;
        assert_eq!(
            request.max_tokens,
            Some(1024),
            "a missing key must fall back to the role default"
        );

        Ok(())
    }
}
