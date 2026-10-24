//! SSE streaming endpoint and approval resolution.
//!
//! These handlers connect the orchestrator's streaming output to the frontend
//! via Server-Sent Events.
//!
//! ## Routes
//!
//! * `GET  /api/chat/events`        — SSE stream of chat events, so a message
//!   written in one device shows up in the others
//! * `POST /api/chat/stream`        — SSE stream of [`SSEEvent`]s
//! * `POST /api/approval/{request_id}` — Resolve a pending approval

use axum::extract::{Path, Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::Value;
use std::convert::Infallible;
use std::pin::Pin;
use tokio::sync::broadcast;

use crate::chat_events::ChatEvent;
use crate::db::repos::messages::{MessagesCursor, MessagesRepo};
use crate::orchestrator::agent::BrowserContext;
use crate::orchestrator::agent::SSEEvent;
use crate::AppState;

/// Query payload for the streaming message endpoint.
#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub content: String,
    pub browser_context: Option<BrowserContext>,
    #[serde(rename = "override")]
    pub override_cmd: Option<String>,
    /// Identifier proposed by the client for its own user message (design D5).
    /// Honoured only when it is a non-empty string of at most 64 characters that
    /// no persisted message owns; otherwise the server generates one and the
    /// turn proceeds normally.
    pub client_message_id: Option<String>,
}

/// Query payload for the chat event stream: the only accepted parameter is the
/// opaque cursor of the last message the client saw.
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub since: Option<String>,
}

/// Body payload for the approval resolution endpoint.
#[derive(Debug, Deserialize)]
pub struct ApprovalBody {
    pub approved: bool,
}

/// Messages fetched per replay page. The replay loops until it catches up with
/// the live flow, so a backlog larger than one page is never truncated.
const CHAT_EVENTS_REPLAY_PAGE: i64 = 100;

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Wrap an SSE stream into a response that tells intermediaries (proxies,
/// nginx) neither to transform nor to buffer the event stream.
fn sse_response(
    stream: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>,
) -> axum::response::Response {
    let mut response = Sse::new(stream).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
}

/// `POST /api/chat/stream`
///
/// Sends a message to the orchestrator and returns the response as an SSE
/// stream of [`SSEEvent`] values.
///
/// The response declares `cache-control: no-cache, no-transform` and
/// `x-accel-buffering: no` so intermediaries (proxies, nginx) neither
/// transform nor buffer the event stream.
///
/// When the orchestrator is not available (e.g. in tests), falls back to a
/// single static chunk to preserve backward compatibility.
pub async fn stream_message(
    State(state): State<AppState>,
    Json(query): Json<MessageQuery>,
) -> axum::response::Response {
    tracing::info!(
        content_len = %query.content.len(),
        "📥 SSE stream request received"
    );

    // One turn at a time (D6/D10). A busy chat answers a plain HTTP 409 —never
    // an SSE stream— before persisting anything and before starting any work.
    // Acquiring the turn publishes its start; the guard releases the lock
    // (publishing its end) when it is dropped, so neither an error nor a panic
    // inside the turn can leave the chat locked forever.
    let turn_guard = match state.turn_lock.try_acquire() {
        Some(guard) => guard,
        None => {
            tracing::info!("🚫 A turn is already running; rejecting with 409");
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "El chat ya está procesando un turno. Espera a que termine para enviar otro mensaje.",
                    "busy": true,
                })),
            )
                .into_response();
        }
    };

    // If orchestrator is not configured, fall back to stub response.
    let Some(orchestrator) = state.orchestrator.clone() else {
        tracing::warn!("Orchestrator is None, using fallback stub response");
        let event = SSEEvent::Chunk {
            content: "Hello from Valet!".to_string(),
        };
        // No child task is launched on this path, so the turn is just this
        // response: the guard is held by the stream and released —freeing the
        // chat and publishing `turn_finished`— as soon as the body is done.
        let stream = async_stream::stream! {
            let _turn_guard = turn_guard;
            yield Ok::<Event, Infallible>(Event::default().data(event.to_json_string()));
        };
        return sse_response(Box::pin(stream));
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<SSEEvent>(32);

    // Resolve profile_id from the database instead of hardcoding "profile-1".
    let profile = match crate::db::repos::profiles::ProfilesRepo::get_or_create(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to resolve profile");
            let _ = tx
                .send(SSEEvent::Error {
                    message: format!("Failed to resolve profile: {}", e),
                })
                .await;
            // The turn never starts, so no task is launched either; the guard is
            // released with this (empty) body rather than leaked.
            let stream = async_stream::stream! {
                let _turn_guard = turn_guard;
                yield Ok::<Event, Infallible>(Event::default().data(""));
            };
            return sse_response(Box::pin(stream));
        }
    };

    let content = query.content.clone();
    let browser_context = query.browser_context.clone();
    let client_message_id = query.client_message_id.clone();

    // Persist browser_context into settings (inline — reverse_geocode has internal cache)
    if let Some(ref ctx) = query.browser_context {
        use crate::db::repos::settings::SettingsRepo;
        let _ = SettingsRepo::set(&state.db, "timezone", &ctx.timezone).await;
        if let Some(lat) = ctx.latitude {
            let _ = SettingsRepo::set(&state.db, "latitude", &lat.to_string()).await;
        }
        if let Some(lon) = ctx.longitude {
            let _ = SettingsRepo::set(&state.db, "longitude", &lon.to_string()).await;
        }
        if let Some(ref loc) = ctx.location_name {
            let _ = SettingsRepo::set(&state.db, "location_name", loc).await;
        } else if let (Some(lat), Some(lon)) = (ctx.latitude, ctx.longitude) {
            if let Some(address) = crate::tools::geo_utils::reverse_geocode(lat, lon).await {
                let _ = SettingsRepo::set(&state.db, "location_name", &address).await;
            }
        }
    }

    tokio::spawn(async move {
        // The turn lock lives here, not in the response stream: the lock must be
        // released when the *turn* ends, and the turn is this task. Tying it to
        // the response body instead would free the chat the moment a client
        // disconnects mid-turn, letting a second client start a concurrent turn
        // while this one is still running — which breaks "one turn at a time".
        // The guard is a local of the task body, so it is dropped before the
        // task's captures —that is, before `tx` closes the response stream. The
        // chat is therefore already free by the time the client sees the stream
        // end, and `turn_finished` is on the bus before that.
        let _turn_guard = turn_guard;
        if let Err(e) = orchestrator
            .process_message_stream(
                &profile.id,
                &content,
                browser_context,
                client_message_id,
                tx.clone(),
            )
            .await
        {
            tracing::error!(error = %e, "❌ Orchestrator error, sending error to client");
            let _ = tx
                .send(SSEEvent::Error {
                    message: format!("Error: {}", e),
                })
                .await
                .ok();
        }
    });

    let stream = async_stream::stream! {
        while let Some(event) = rx.recv().await {
            yield Ok::<Event, Infallible>(Event::default().data(event.to_json_string()));
        }
    };

    sse_response(Box::pin(stream))
}

/// `GET /api/chat/events`
///
/// Long-lived SSE stream with the chat's live events, so every connected device
/// sees the messages persisted by any other one without reloading.
///
/// The stream opens with `{"type":"snapshot","busy":<bool>}`, then —when a
/// `since` cursor is informed— replays the messages strictly after it, each as
/// `{"type":"message_created","message":<row>,"cursor":"<opaque>"}`, and from
/// there on follows the live flow. A subscriber that falls behind the bus queue
/// is told to `resync` and the stream closes: events are never dropped in
/// silence. An idle connection is kept alive with an SSE comment.
///
/// An unreadable `since` is rejected with `400` without opening any stream, with
/// the same criterion `GET /api/messages` uses for its cursor.
pub async fn chat_events(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>,
) -> axum::response::Response {
    let cursor = match params
        .since
        .as_deref()
        .map(MessagesCursor::decode)
        .transpose()
    {
        Ok(cursor) => cursor,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    // Subscribe *before* reading the snapshot and the replay, so everything
    // published from here on is queued and nothing falls between the replay and
    // the live flow. A message that lands in both is harmless: the client
    // upserts by id (D2).
    let mut receiver = state.chat_events.subscribe();
    let busy = state.turn_lock.is_busy();

    let mut replayed: Vec<crate::models::Message> = Vec::new();
    if cursor.is_some() {
        let mut page_cursor = cursor;
        loop {
            match MessagesRepo::list_after(&state.db, page_cursor.as_ref(), CHAT_EVENTS_REPLAY_PAGE)
                .await
            {
                Ok((page, next)) => {
                    if page.is_empty() {
                        // Nothing strictly after the cursor: keep the cursor the
                        // caller brought and stop.
                        break;
                    }
                    replayed.extend(page);
                    match next {
                        Some(next) => page_cursor = Some(next),
                        None => break,
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "failed to replay the chat events");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": format!("failed to read the chat history: {error}"),
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    let heartbeat_period = state.chat_events_heartbeat;
    let bus_capacity = state.chat_events_capacity;
    let snapshot = serde_json::json!({ "type": "snapshot", "busy": busy }).to_string();

    let stream = async_stream::stream! {
        // 1. The state of the turn at the very moment the stream opened.
        yield Ok::<Event, Infallible>(Event::default().data(snapshot));

        // 2. Whatever the client missed, in ascending order.
        for message in replayed {
            yield Ok::<Event, Infallible>(
                Event::default().data(ChatEvent::message_created(&message).to_json_string()),
            );
        }

        // 3. The live flow, kept alive by a heartbeat comment (D7).
        let mut heartbeat = tokio::time::interval(heartbeat_period);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // An interval's first tick resolves immediately; consume it so the first
        // heartbeat lands one period after the stream opened.
        heartbeat.tick().await;

        loop {
            tokio::select! {
                received = receiver.recv() => match received {
                    Ok(event) => {
                        yield Ok::<Event, Infallible>(
                            Event::default().data(event.to_json_string()),
                        );
                    }
                    // The subscriber fell behind and the bus dropped events.
                    // Never silence: ask it to reconnect with its last cursor and
                    // close the stream.
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            bus_capacity,
                            "chat event subscriber lagged; asking it to resync"
                        );
                        yield Ok::<Event, Infallible>(
                            Event::default().data("{\"type\":\"resync\"}"),
                        );
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = heartbeat.tick() => {
                    // A comment is not an event: the client ignores it, and it
                    // only keeps an idle connection alive across proxies. As it
                    // is not a `data:` line, it never looks like a chat event.
                    yield Ok::<Event, Infallible>(
                        Event::default().comment("keep-alive"),
                    );
                }
            }
        }
    };

    sse_response(Box::pin(stream))
}

/// `POST /api/approval/{request_id}`
///
/// Resolves a pending human-in-the-loop approval request by delegating
/// to the guardrails component.
///
/// When guardrails are not available (e.g. in tests), falls back to a
/// hard-coded success response to preserve backward compatibility.
pub async fn resolve_approval(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
    Json(body): Json<ApprovalBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let guardrails = match state.guardrails {
        Some(ref g) => g.clone(),
        // Fall back to stub for backward compat with tests
        None => {
            return Ok(Json(serde_json::json!({
                "status": "resolved",
                "approved": body.approved,
            })));
        }
    };

    match guardrails.resolve_approval(&request_id, body.approved) {
        Ok(()) => Ok(Json(serde_json::json!({
            "status": "resolved",
            "approved": body.approved,
        }))),
        Err(e) => {
            let (status, msg) = match e {
                crate::orchestrator::guardrails::GuardrailError::RequestNotFound(_) => {
                    (StatusCode::NOT_FOUND, "Approval request not found")
                }
                crate::orchestrator::guardrails::GuardrailError::AlreadyResolved => {
                    (StatusCode::CONFLICT, "Approval request already resolved")
                }
                _ => (StatusCode::BAD_REQUEST, "Invalid approval request"),
            };
            Err((status, Json(serde_json::json!({"error": msg}))))
        }
    }
}

/// Assemble the event stream, message stream and approval routes into a
/// sub-router.
///
/// `GET /api/chat/events` is mounted here, so it ends up under the same
/// authenticated session as the rest of `/api/*` (see `app_with_state`).
pub fn routes() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/chat/events", get(chat_events))
        .route("/api/chat/stream", post(stream_message))
        .route("/api/approval/{request_id}", post(resolve_approval))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use std::sync::{Arc, Mutex, RwLock};
    use tower::ServiceExt;

    // ------------------------------------------------------------------
    // Approval endpoint
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_approval_endpoint_returns_ok() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/approval/test-request")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"approved":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_approval_endpoint_structure() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/approval/req-1")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"approved":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "resolved");
        // The stub (no guardrails configured) must reflect the requested value.
        assert_eq!(json["approved"], false);
    }

    #[tokio::test]
    async fn test_approval_missing_body_returns_error() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/approval/req-1")
                    .header("content-type", "application/json")
                    .body(Body::from(r"{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_client_error());
    }

    // ------------------------------------------------------------------
    // Stream endpoint
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_stream_endpoint_returns_sse() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content":"Hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("text/event-stream"),
            "Expected SSE content-type, got: {}",
            content_type
        );
    }

    /// The stream response must tell intermediaries not to transform the body.
    #[tokio::test]
    async fn test_stream_response_declares_no_transform() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content":"Hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let cache_control = response
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            cache_control.contains("no-cache") && cache_control.contains("no-transform"),
            "Expected cache-control to declare no-cache, no-transform, got: {cache_control}"
        );
    }

    /// The stream response must tell nginx not to buffer the SSE body.
    #[tokio::test]
    async fn test_stream_response_declares_no_buffering() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content":"Hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let x_accel_buffering = response
            .headers()
            .get("x-accel-buffering")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(
            x_accel_buffering, "no",
            "Expected x-accel-buffering: no, got: {x_accel_buffering}"
        );
    }

    #[tokio::test]
    async fn test_stream_endpoint_rejects_missing_content() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(r"{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn test_stream_endpoint_route_not_found_for_get() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/chat/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // ------------------------------------------------------------------
    // Mock types for profile_id capture test
    // ------------------------------------------------------------------

    /// Spy tool that captures the profile_id received during execution.
    struct ProfileIdCaptureTool {
        captured_profile_id: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait]
    impl crate::tools::r#trait::Tool for ProfileIdCaptureTool {
        fn name(&self) -> &'static str {
            "capture_tool"
        }

        fn description(&self) -> &'static str {
            "Tool that captures profile_id"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &serde_json::Value) -> crate::tools::permission::Permission {
            crate::tools::permission::Permission::NoConfirm
        }

        async fn execute(
            &self,
            args: serde_json::Value,
        ) -> Result<crate::tools::r#trait::ToolResult, crate::tools::r#trait::ToolError> {
            let pid = args
                .get("profile_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            *self.captured_profile_id.lock().unwrap() = pid;
            Ok(crate::tools::r#trait::ToolResult {
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
    impl crate::llm::provider::LLMProvider for MockLLMWithToolCallNoProfile {
        async fn chat(
            &self,
            _request: crate::llm::provider::ChatRequest,
        ) -> Result<crate::llm::provider::ChatResponse, crate::llm::provider::LLMError> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count == 1 {
                Ok(crate::llm::provider::ChatResponse {
                    message: crate::llm::provider::ChatMessage {
                        role: "assistant".into(),
                        content: "Let me process that.".into(),
                        tool_calls: Some(vec![crate::llm::provider::ToolCall {
                            id: "call-1".into(),
                            name: "capture_tool".into(),
                            arguments: serde_json::json!({"some_arg": "value"}),
                        }]),
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                    finish_reason: None,
                })
            } else {
                Ok(crate::llm::provider::ChatResponse {
                    message: crate::llm::provider::ChatMessage {
                        role: "assistant".into(),
                        content: "Done.".into(),
                        tool_calls: None,
                        tool_result: None,
                        tool_call_id: None,
                    },
                    usage: None,
                    finish_reason: None,
                })
            }
        }

        async fn chat_stream(
            &self,
            request: crate::llm::provider::ChatRequest,
        ) -> Result<
            Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<
                                crate::llm::provider::StreamEvent,
                                crate::llm::provider::LLMError,
                            >,
                        > + Send,
                >,
            >,
            crate::llm::provider::LLMError,
        > {
            let result = self.chat(request).await?;
            let tool_calls = result.message.tool_calls.clone();
            let mut events: Vec<
                Result<crate::llm::provider::StreamEvent, crate::llm::provider::LLMError>,
            > = Vec::new();
            if let Some(tcs) = tool_calls {
                for tc in tcs {
                    events.push(Ok(crate::llm::provider::StreamEvent::ToolCall(tc)));
                }
            }
            events.push(Ok(crate::llm::provider::StreamEvent::Done(result)));
            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test]
    async fn test_stream_profile_id_resolved_from_db() -> Result<(), Box<dyn std::error::Error>> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        // 1. Create in-memory DB and run migrations
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

        // 2. INSERT a profile with known id that is NOT "profile-1"
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO profiles (id, name, avatar_url, preferences, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind("test-profile-real")
        .bind("Test User")
        .bind(Option::<String>::None)
        .bind("{}")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await?;

        // 3. Create spy tool that captures profile_id
        let captured_profile_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let mut registry = crate::tools::registry::ToolRegistry::new();
        registry.register(Box::new(ProfileIdCaptureTool {
            captured_profile_id: captured_profile_id.clone(),
        }));
        let registry = Arc::new(registry);

        // 4. Create mock LLM and supporting components
        let call_count = Arc::new(Mutex::new(0));
        let llm = Arc::new(MockLLMWithToolCallNoProfile {
            call_count: call_count.clone(),
        });
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let context_builder = Arc::new(crate::orchestrator::context_builder::ContextBuilder::new());
        let config = crate::orchestrator::agent::OrchestratorConfig::default();

        let orchestrator = Arc::new(crate::orchestrator::agent::Orchestrator::new(
            llm,
            registry.clone(),
            guardrails.clone(),
            context_builder,
            config,
            pool.clone(),
            None,
            None,
            Arc::new(RwLock::new(None)),
        ));

        // 5. Build AppState
        let (chat_events, _chat_events_rx) =
            tokio::sync::broadcast::channel(crate::chat_events::CHAT_EVENT_BUS_CAPACITY);
        let turn_lock = Arc::new(crate::chat_events::TurnLock::new(chat_events.clone()));
        let state = crate::AppState {
            db: pool,
            orchestrator: Some(orchestrator),
            guardrails: Some(guardrails),
            tool_registry: Some(registry),
            auth_config: None,
            collapse_tx: None,
            collapse_threshold_tokens: 2000,
            memory_tx: None,
            shutdown_tx: None,
            last_api_call: Arc::new(RwLock::new(None)),
            chat_events,
            turn_lock,
            chat_events_heartbeat: std::time::Duration::from_secs(
                crate::chat_events::DEFAULT_CHAT_EVENTS_HEARTBEAT_SECS,
            ),
            chat_events_capacity: crate::chat_events::CHAT_EVENT_BUS_CAPACITY,
        };

        // 6. Build axum Router
        let app = crate::app_with_state(state);

        // 7. Send POST request to /api/chat/stream
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content":"create an event for me"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 8. Drain the SSE response body (this blocks until the stream ends)
        let body = response.into_body();
        let _bytes = axum::body::to_bytes(body, usize::MAX).await?;

        // 9. Assert that the spy tool received "test-profile-real" as profile_id
        //    — NOT "profile-1" (the hardcoded value).
        let captured = captured_profile_id.lock().unwrap().clone();
        assert_eq!(
            captured.as_deref(),
            Some("test-profile-real"),
            "Profile_id should be 'test-profile-real' (from DB), not 'profile-1'. Got: {:?}",
            captured
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_endpoint_accepts_browser_context() {
        let app = crate::app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/chat/stream")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"content":"Hello","browser_context":{"timestamp":"2026-09-24T08:00:00Z","timezone":"Europe/Madrid","latitude":39.36,"longitude":-0.41,"location_name":"Silla, Valencia, España"}}"#
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("text/event-stream"),
            "Expected SSE content-type, got: {}",
            content_type
        );
    }

    // ------------------------------------------------------------------
    // GET /api/chat/events
    // ------------------------------------------------------------------

    use crate::chat_events::ChatEvent;
    use crate::db::repos::messages::{MessagesCursor, MessagesRepo};
    use std::time::Duration;

    /// Percent-encode a value for the query string so the request URI stays
    /// valid; axum percent-decodes it back before the handler sees it.
    fn encode_query_value(value: &str) -> String {
        let mut out = String::new();
        for byte in value.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char);
                }
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        out
    }

    /// Insert a message row with an explicit `created_at`, so several rows can
    /// tie on the same second.
    async fn insert_message(pool: &sqlx::SqlitePool, id: &str, created_at: &str) {
        sqlx::query(
            "INSERT INTO messages (id, role, content, tokens_count, created_at) \
             VALUES (?1, 'user', ?2, 0, ?3)",
        )
        .bind(id)
        .bind(format!("Message {id}"))
        .bind(created_at)
        .execute(pool)
        .await
        .expect("the seed row must be inserted");
    }

    /// Read the body of an SSE response until `stop` is satisfied against the
    /// accumulated text, the stream ends or `timeout` elapses. It never panics
    /// on a timeout: the caller asserts on whatever it managed to read.
    async fn read_sse_until(
        response: axum::response::Response,
        timeout: Duration,
        stop: impl Fn(&str) -> bool,
    ) -> String {
        use futures::StreamExt;
        let mut body = response.into_body().into_data_stream();
        let mut acc = String::new();
        let deadline = tokio::time::Instant::now() + timeout;
        while !stop(&acc) {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, body.next()).await {
                Ok(Some(Ok(chunk))) => acc.push_str(&String::from_utf8_lossy(&chunk)),
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_elapsed) => break,
            }
        }
        acc
    }

    /// The `data:` lines accumulated so far.
    fn count_data_lines(text: &str) -> usize {
        text.lines()
            .filter(|line| line.starts_with("data:"))
            .count()
    }

    /// The SSE comment (heartbeat) lines accumulated so far.
    fn count_comment_lines(text: &str) -> usize {
        text.lines().filter(|line| line.starts_with(':')).count()
    }

    /// The JSON payload of every `data:` line, in order.
    fn sse_payloads(text: &str) -> Vec<Value> {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
            .collect()
    }

    /// Scenario: el primer evento es `snapshot` con el estado real del cerrojo.
    #[tokio::test]
    async fn test_chat_events_first_event_is_the_snapshot_of_the_real_lock_state() {
        // No turn in progress: the chat is free.
        let state = crate::AppState::new_in_memory_empty().await;
        let app = crate::app_with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/chat/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let text = read_sse_until(response, Duration::from_secs(5), |acc| {
            count_data_lines(acc) >= 1
        })
        .await;
        let events = sse_payloads(&text);
        assert_eq!(
            events.first().map(|e| &e["type"]),
            Some(&Value::from("snapshot"))
        );
        assert_eq!(events[0]["busy"], Value::Bool(false));

        // A turn in progress: the first event must say so.
        let state = crate::AppState::new_in_memory_empty().await;
        let turn_lock = state.turn_lock.clone();
        let app = crate::app_with_state(state);
        let _guard = turn_lock.try_acquire().expect("the lock starts free");
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/chat/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let text = read_sse_until(response, Duration::from_secs(5), |acc| {
            count_data_lines(acc) >= 1
        })
        .await;
        let events = sse_payloads(&text);
        assert_eq!(
            events.first().map(|e| &e["type"]),
            Some(&Value::from("snapshot"))
        );
        assert_eq!(events[0]["busy"], Value::Bool(true));
    }

    /// Scenario: el replay recupera lo perdido y solo lo perdido.
    #[tokio::test]
    async fn test_chat_events_replays_only_what_follows_the_cursor() {
        let state = crate::AppState::new_in_memory_empty().await;
        insert_message(&state.db, "e1", "2026-10-24 10:00:01").await;
        insert_message(&state.db, "e2", "2026-10-24 10:00:02").await;
        insert_message(&state.db, "e3", "2026-10-24 10:00:03").await;
        let app = crate::app_with_state(state);

        let cursor = MessagesCursor::new("2026-10-24 10:00:01", "e1").encode();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/chat/events?since={}",
                        encode_query_value(&cursor)
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // snapshot + the two messages after the cursor
        let text = read_sse_until(response, Duration::from_secs(5), |acc| {
            count_data_lines(acc) >= 3
        })
        .await;
        let events = sse_payloads(&text);
        assert_eq!(
            events.len(),
            3,
            "expected the snapshot plus exactly two messages, got: {events:?}"
        );
        assert_eq!(events[0]["type"], "snapshot");
        assert_eq!(events[1]["type"], "message_created");
        assert_eq!(events[1]["message"]["id"], "e2");
        assert_eq!(events[2]["message"]["id"], "e3");
        // Ascending order and an opaque, non-empty cursor on every message.
        assert_eq!(events[1]["cursor"], "2026-10-24 10:00:02|e2");
        assert_eq!(events[2]["cursor"], "2026-10-24 10:00:03|e3");
        // The row keeps the shape `GET /api/messages` returns.
        assert_eq!(events[1]["message"]["role"], "user");
        assert_eq!(events[1]["message"]["content"], "Message e2");
        assert!(events[1]["message"]["created_at"].is_string());
    }

    /// Scenario: sin mensajes posteriores el replay está vacío.
    #[tokio::test]
    async fn test_chat_events_with_nothing_after_the_cursor_replays_nothing() {
        let mut state = crate::AppState::new_in_memory_empty().await;
        // Short heartbeats: the heartbeat is the barrier that proves the replay
        // (emitted right after the snapshot) was empty.
        state.chat_events_heartbeat = Duration::from_millis(40);
        insert_message(&state.db, "e1", "2026-10-24 10:00:01").await;
        let app = crate::app_with_state(state);

        // `app.clone()` keeps the router —and with it the event bus— alive while
        // the body is read, exactly as `axum::serve` does for the whole life of
        // the server. A consumed router would drop the bus and close the stream.
        let cursor = MessagesCursor::new("2026-10-24 10:00:01", "e1").encode();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/chat/events?since={}",
                        encode_query_value(&cursor)
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let text = read_sse_until(response, Duration::from_secs(5), |acc| {
            count_comment_lines(acc) >= 2
        })
        .await;
        assert!(
            count_comment_lines(&text) >= 2,
            "the heartbeat must keep the stream alive, got: {text:?}"
        );
        let events = sse_payloads(&text);
        assert_eq!(
            events.len(),
            1,
            "no message follows the cursor, so only the snapshot arrives: {events:?}"
        );
        assert_eq!(events[0]["type"], "snapshot");
    }

    /// Scenario: un cursor ilegible se rechaza sin abrir el stream.
    #[tokio::test]
    async fn test_chat_events_rejects_an_unreadable_cursor_without_opening_a_stream() {
        let state = crate::AppState::new_in_memory_empty().await;
        insert_message(&state.db, "e1", "2026-10-24 10:00:01").await;
        let app = crate::app_with_state(state);

        for since in ["no-es-un-cursor", "2026-10-24 10:00:01|", "|e1"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!(
                            "/api/chat/events?since={}",
                            encode_query_value(since)
                        ))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "an unreadable cursor ({since:?}) must be rejected"
            );
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            assert!(
                !content_type.contains("text/event-stream"),
                "no event stream may be opened for an unreadable cursor ({since:?})"
            );

            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: Value = serde_json::from_slice(&body).unwrap();
            assert!(
                json["error"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("cursor"),
                "the error must be readable and mention the cursor, got: {json}"
            );
        }
    }

    /// Scenario: el stream sobrevive a los proxies intermedios.
    #[tokio::test]
    async fn test_chat_events_declares_the_proxy_friendly_headers() {
        let state = crate::AppState::new_in_memory_empty().await;
        let app = crate::app_with_state(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/chat/events")
                    .header("accept-encoding", "gzip, deflate, br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string()
        };

        assert!(
            header("content-type").contains("text/event-stream"),
            "expected text/event-stream, got: {}",
            header("content-type")
        );
        let cache_control = header("cache-control");
        assert!(
            cache_control.contains("no-cache") && cache_control.contains("no-transform"),
            "expected cache-control to declare no-cache, no-transform, got: {cache_control}"
        );
        assert_eq!(header("x-accel-buffering"), "no");
        assert!(
            headers.get("content-encoding").is_none(),
            "the event stream must not be compressed or transformed"
        );
    }

    /// Scenario: una conexión ociosa no se cae y sigue viva.
    #[tokio::test]
    async fn test_an_idle_event_stream_is_kept_alive_by_heartbeats() {
        let mut state = crate::AppState::new_in_memory_empty().await;
        // Configurable heartbeat: 15 s in production, milliseconds here, so the
        // test never waits a real period.
        state.chat_events_heartbeat = Duration::from_millis(30);
        let app = crate::app_with_state(state);

        // Keep the router (and therefore the bus) alive while the body is read.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/chat/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let text = read_sse_until(response, Duration::from_secs(5), |acc| {
            count_comment_lines(acc) >= 2
        })
        .await;
        assert!(
            count_comment_lines(&text) >= 2,
            "an idle connection must be kept alive by heartbeats, got: {text:?}"
        );
        let events = sse_payloads(&text);
        assert!(
            events.iter().all(|event| event["type"] == "snapshot"),
            "a heartbeat must not be read as a chat event: {events:?}"
        );
    }

    // ------------------------------------------------------------------
    // POST /api/chat/stream — one turn at a time, client_message_id
    // ------------------------------------------------------------------

    /// Mock LLM that answers with plain text immediately (no tool calls).
    struct PlainMockLLM;

    #[async_trait::async_trait]
    impl crate::llm::provider::LLMProvider for PlainMockLLM {
        async fn chat(
            &self,
            _request: crate::llm::provider::ChatRequest,
        ) -> Result<crate::llm::provider::ChatResponse, crate::llm::provider::LLMError> {
            Ok(crate::llm::provider::ChatResponse {
                message: crate::llm::provider::ChatMessage {
                    role: "assistant".into(),
                    content: "Vale.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
                finish_reason: None,
            })
        }

        async fn chat_stream(
            &self,
            request: crate::llm::provider::ChatRequest,
        ) -> Result<
            Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<
                                crate::llm::provider::StreamEvent,
                                crate::llm::provider::LLMError,
                            >,
                        > + Send,
                >,
            >,
            crate::llm::provider::LLMError,
        > {
            let result = self.chat(request).await?;
            let events: Vec<
                Result<crate::llm::provider::StreamEvent, crate::llm::provider::LLMError>,
            > = vec![
                Ok(crate::llm::provider::StreamEvent::Chunk(
                    result.message.content.clone(),
                )),
                Ok(crate::llm::provider::StreamEvent::Done(result)),
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// Mock LLM whose stream always fails.
    struct FailingMockLLM;

    #[async_trait::async_trait]
    impl crate::llm::provider::LLMProvider for FailingMockLLM {
        async fn chat(
            &self,
            _request: crate::llm::provider::ChatRequest,
        ) -> Result<crate::llm::provider::ChatResponse, crate::llm::provider::LLMError> {
            Err(crate::llm::provider::LLMError::HttpError("boom".into()))
        }

        async fn chat_stream(
            &self,
            _request: crate::llm::provider::ChatRequest,
        ) -> Result<
            Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<
                                crate::llm::provider::StreamEvent,
                                crate::llm::provider::LLMError,
                            >,
                        > + Send,
                >,
            >,
            crate::llm::provider::LLMError,
        > {
            Err(crate::llm::provider::LLMError::HttpError("boom".into()))
        }
    }

    /// An `AppState` whose orchestrator answers with `llm` — no network at all.
    async fn state_with_llm(
        llm: Arc<dyn crate::llm::provider::LLMProvider>,
    ) -> (crate::AppState, sqlx::SqlitePool) {
        let mut state = crate::AppState::new_in_memory_empty().await;
        let pool = state.db.clone();
        let registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        let guardrails = Arc::new(crate::orchestrator::guardrails::Guardrails::new(
            registry.clone(),
        ));
        let context_builder = Arc::new(crate::orchestrator::context_builder::ContextBuilder::new());
        let orchestrator = Arc::new(
            crate::orchestrator::agent::Orchestrator::new(
                llm,
                registry.clone(),
                guardrails.clone(),
                context_builder,
                crate::orchestrator::agent::OrchestratorConfig::default(),
                pool.clone(),
                None,
                None,
                Arc::new(RwLock::new(None)),
            )
            .with_event_bus(Some(state.chat_events.clone())),
        );
        state.orchestrator = Some(orchestrator);
        state.tool_registry = Some(registry);
        state.guardrails = Some(guardrails);
        (state, pool)
    }

    /// A `POST /api/chat/stream` request with the given JSON body.
    fn stream_request(body: Value) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/api/chat/stream")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }

    /// Scenario: el segundo envío simultáneo recibe un aviso claro.
    #[tokio::test]
    async fn test_a_second_simultaneous_message_is_rejected_with_409_and_persists_nothing() {
        let state = crate::AppState::new_in_memory_empty().await;
        let turn_lock = state.turn_lock.clone();
        let db = state.db.clone();
        let app = crate::app_with_state(state);

        // A turn is already in progress.
        let guard = turn_lock.try_acquire().expect("the lock starts free");

        let response = app
            .oneshot(stream_request(serde_json::json!({"content": "Hola"})))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["busy"], Value::Bool(true));
        assert!(
            !json["error"].as_str().unwrap_or_default().is_empty(),
            "the rejection must carry a readable message, got: {json}"
        );

        let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            messages, 0,
            "a rejected turn must not persist a user message"
        );
        assert!(
            turn_lock.is_busy(),
            "the first turn must not be interrupted by the rejected request"
        );
        drop(guard);
    }

    /// Scenario: el resto de clientes se entera de que hay un turno en curso.
    #[tokio::test]
    async fn test_an_admitted_turn_publishes_its_start_and_its_finish() {
        let state = crate::AppState::new_in_memory_empty().await;
        let mut bus_rx = state.chat_events.subscribe();
        let app = crate::app_with_state(state);

        let response = app
            .oneshot(stream_request(serde_json::json!({"content": "Hola"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // Run the stream to its end so the turn finishes.
        let text = read_sse_until(response, Duration::from_secs(5), |_| false).await;
        assert!(
            text.contains("data:"),
            "the admitted turn must still stream something, got: {text:?}"
        );

        let mut seen = Vec::new();
        while let Ok(event) = bus_rx.try_recv() {
            seen.push(event);
        }
        assert!(
            matches!(seen.first(), Some(ChatEvent::TurnStarted)),
            "the admitted turn must publish its start first: {seen:?}"
        );
        assert!(
            matches!(seen.last(), Some(ChatEvent::TurnFinished)),
            "the finished turn must publish its end last: {seen:?}"
        );
    }

    /// Scenario: un turno que falla libera el cerrojo.
    #[tokio::test]
    async fn test_a_failed_turn_releases_the_lock_for_the_next_one() {
        let (state, _pool) = state_with_llm(Arc::new(FailingMockLLM)).await;
        let turn_lock = state.turn_lock.clone();
        let app = crate::app_with_state(state);

        let first = app
            .clone()
            .oneshot(stream_request(serde_json::json!({"content": "Hola"})))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let text = read_sse_until(first, Duration::from_secs(5), |_| false).await;
        assert!(
            text.contains(r#""type":"error""#),
            "the failing provider must surface an error, got: {text:?}"
        );
        assert!(
            !turn_lock.is_busy(),
            "a turn that ends with an error must release the lock"
        );

        let second = app
            .oneshot(stream_request(
                serde_json::json!({"content": "Hola otra vez"}),
            ))
            .await
            .unwrap();
        assert_ne!(
            second.status(),
            StatusCode::CONFLICT,
            "the next turn must be admitted after a failed one"
        );
    }

    /// Scenario: resolver una aprobación no queda bloqueado por el cerrojo.
    #[tokio::test]
    async fn test_resolving_an_approval_does_not_need_the_turn_lock() {
        let state = crate::AppState::new_in_memory_empty().await;
        let turn_lock = state.turn_lock.clone();
        let app = crate::app_with_state(state);

        let _guard = turn_lock.try_acquire().expect("the lock starts free");
        assert!(turn_lock.is_busy(), "a turn must be in progress");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/approval/req-while-busy")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"approved":true}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "resolving an approval of the running turn must not be blocked"
        );
    }

    /// Scenario: el id propuesto se respeta.
    #[tokio::test]
    async fn test_a_valid_client_message_id_is_used_as_the_user_message_id() {
        let (state, pool) = state_with_llm(Arc::new(PlainMockLLM)).await;
        let app = crate::app_with_state(state);

        let response = app
            .oneshot(stream_request(serde_json::json!({
                "content": "Hola",
                "client_message_id": "cli-abc",
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let text = read_sse_until(response, Duration::from_secs(5), |_| false).await;

        let done = sse_payloads(&text)
            .into_iter()
            .find(|event| event["type"] == "done")
            .unwrap_or_else(|| panic!("the turn must finish with a done event, got: {text:?}"));
        assert_eq!(
            done["user_message_id"], "cli-abc",
            "the done event must report the proposed identifier"
        );

        let stored = MessagesRepo::find_by_id(&pool, "cli-abc")
            .await
            .unwrap()
            .expect("the proposed identifier must be the persisted one");
        assert_eq!(stored.role, "user");
        assert_eq!(stored.content, "Hola");
    }

    /// Scenario: un id ya usado y un id inválido no rompen el turno.
    #[tokio::test]
    async fn test_an_unusable_client_message_id_never_breaks_the_turn() {
        let (state, pool) = state_with_llm(Arc::new(PlainMockLLM)).await;
        // A row that already owns the identifier a client is about to propose.
        MessagesRepo::create_with_id(
            &pool,
            "already-used",
            "assistant",
            "previo",
            None,
            None,
            None,
            None,
            2000,
            None,
        )
        .await
        .unwrap();
        let app = crate::app_with_state(state);

        let cases: [(Option<String>, &str); 4] = [
            (None, "ausente"),
            (Some(String::new()), "vacío"),
            (Some("x".repeat(65)), "demasiado largo"),
            (Some("already-used".to_string()), "ya usado"),
        ];

        for (index, (requested, label)) in cases.iter().enumerate() {
            let content = format!("Turno {index}");
            let mut body = serde_json::json!({ "content": content });
            if let Some(id) = requested {
                body["client_message_id"] = serde_json::json!(id);
            }

            let response = app.clone().oneshot(stream_request(body)).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "case {label}: the turn must proceed normally"
            );
            let text = read_sse_until(response, Duration::from_secs(5), |_| false).await;

            let done = sse_payloads(&text)
                .into_iter()
                .find(|event| event["type"] == "done")
                .unwrap_or_else(|| {
                    panic!("case {label}: the turn must finish with a done event, got: {text:?}")
                });
            let persisted_id = done["user_message_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            assert!(
                !persisted_id.is_empty(),
                "case {label}: the server must persist a generated identifier"
            );
            if let Some(requested) = requested {
                assert_ne!(
                    &persisted_id, requested,
                    "case {label}: the server must generate its own identifier"
                );
            }

            let stored = MessagesRepo::find_by_id(&pool, &persisted_id)
                .await
                .unwrap()
                .unwrap_or_else(|| {
                    panic!("case {label}: the reported identifier must be persisted")
                });
            assert_eq!(stored.role, "user");
            assert_eq!(stored.content, content);
        }

        // The row that already owned the identifier is untouched.
        let untouched = MessagesRepo::find_by_id(&pool, "already-used")
            .await
            .unwrap()
            .expect("the pre-existing row must still exist");
        assert_eq!(untouched.content, "previo");
    }

    // ------------------------------------------------------------------
    // POST /api/chat/stream — the lock accompanies the turn, not the
    // client connection
    // ------------------------------------------------------------------

    /// Mock LLM that parks the turn inside `chat_stream` until it is released.
    ///
    /// While parked a turn is genuinely in progress: the lock is taken and the
    /// orchestrator has not finished. Nothing here touches the network.
    struct ParkingMockLLM {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl crate::llm::provider::LLMProvider for ParkingMockLLM {
        async fn chat(
            &self,
            _request: crate::llm::provider::ChatRequest,
        ) -> Result<crate::llm::provider::ChatResponse, crate::llm::provider::LLMError> {
            Ok(crate::llm::provider::ChatResponse {
                message: crate::llm::provider::ChatMessage {
                    role: "assistant".into(),
                    content: "Vale.".into(),
                    tool_calls: None,
                    tool_result: None,
                    tool_call_id: None,
                },
                usage: None,
                finish_reason: None,
            })
        }

        async fn chat_stream(
            &self,
            request: crate::llm::provider::ChatRequest,
        ) -> Result<
            Pin<
                Box<
                    dyn tokio_stream::Stream<
                            Item = Result<
                                crate::llm::provider::StreamEvent,
                                crate::llm::provider::LLMError,
                            >,
                        > + Send,
                >,
            >,
            crate::llm::provider::LLMError,
        > {
            // The turn is running now, and stays here until the test releases it.
            self.entered.notify_one();
            self.release.notified().await;

            let result = self.chat(request).await?;
            let events: Vec<
                Result<crate::llm::provider::StreamEvent, crate::llm::provider::LLMError>,
            > = vec![
                Ok(crate::llm::provider::StreamEvent::Chunk(
                    result.message.content.clone(),
                )),
                Ok(crate::llm::provider::StreamEvent::Done(result)),
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    /// Scenario (single turn): el cerrojo se libera al terminar el turno, no
    /// cuando el cliente se va.
    ///
    /// Un cliente que desconecta a mitad de turno NO libera el chat: la guarda
    /// vive en la tarea del turno, así que sigue tomada hasta que el turno
    /// termina de verdad. Con la guarda en el cuerpo de la respuesta —el sitio
    /// equivocado— soltar el cuerpo liberaría el cerrojo con el turno aún
    /// corriendo.
    #[tokio::test]
    async fn test_the_turn_lock_accompanies_the_turn_not_the_client_connection() {
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let (state, _pool) = state_with_llm(Arc::new(ParkingMockLLM {
            entered: entered.clone(),
            release: release.clone(),
        }))
        .await;
        let turn_lock = state.turn_lock.clone();
        let mut bus_rx = state.chat_events.subscribe();
        let app = crate::app_with_state(state);

        let response = app
            .oneshot(stream_request(serde_json::json!({"content": "Hola"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            turn_lock.is_busy(),
            "an admitted turn must hold the lock from the start"
        );

        // Wait until the turn is parked inside the provider: from here on it is
        // certainly still running.
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .expect("the turn must reach the provider");

        // The client goes away mid-turn: the body is dropped without draining it.
        drop(response);

        assert!(
            turn_lock.is_busy(),
            "the client leaving must not free the chat while the turn is running"
        );

        // Let the turn finish: only now is the chat free again.
        release.notify_one();
        let finished = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match bus_rx.recv().await {
                    Ok(ChatEvent::TurnFinished) => return true,
                    Ok(_) => continue,
                    Err(_) => return false,
                }
            }
        })
        .await
        .expect("the turn must finish once the provider is released");
        assert!(
            finished,
            "the finished turn must publish its end on the bus"
        );
        assert!(
            !turn_lock.is_busy(),
            "the lock must be free as soon as the turn ends"
        );
    }

    // ------------------------------------------------------------------
    // GET /api/chat/events — resync of a lagging subscriber
    // ------------------------------------------------------------------

    /// Reads chunks from `body` into `acc` until `stop(acc)` holds, the body
    /// ends or `timeout` elapses. Returns whether `stop` was satisfied; it never
    /// panics, so the caller asserts on whatever it managed to read.
    async fn read_into(
        body: &mut axum::body::BodyDataStream,
        acc: &mut String,
        timeout: Duration,
        stop: impl Fn(&str) -> bool,
    ) -> bool {
        use futures::StreamExt;
        let deadline = tokio::time::Instant::now() + timeout;
        while !stop(acc) {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            match tokio::time::timeout(remaining, body.next()).await {
                Ok(Some(Ok(chunk))) => acc.push_str(&String::from_utf8_lossy(&chunk)),
                Ok(Some(Err(_))) | Ok(None) => return false,
                Err(_) => return false,
            }
        }
        true
    }

    /// A `message_created` event for a row with the given id.
    fn chat_event_message(id: &str) -> ChatEvent {
        ChatEvent::message_created(&crate::models::Message {
            id: id.to_string(),
            role: "user".to_string(),
            content: format!("Message {id}"),
            tool_calls: None,
            tool_results: None,
            tokens_count: 0,
            collapsed_content: None,
            collapsed_tokens_count: 0,
            is_indexed: false,
            summary_ref: None,
            location: None,
            tools_used: None,
            widgets: None,
            created_at: "2026-10-24 10:00:01".to_string(),
        })
    }

    /// Opens `GET /api/chat/events` against `app` and returns the response body
    /// stream. `app` is consumed; the caller must keep a `chat_events` sender
    /// clone alive so the bus —and with it the stream— survives.
    async fn open_event_stream(app: axum::Router) -> axum::body::BodyDataStream {
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/chat/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.into_body().into_data_stream()
    }

    /// Scenario (resync): un suscriptor lento recibe el aviso y el stream se
    /// cierra.
    ///
    /// El bus se construye con una cola de un solo evento para desbordarla sin
    /// publicar 256 (design D1).
    #[tokio::test]
    async fn test_a_lagged_subscriber_is_told_to_resync_and_its_stream_closes() {
        let state = crate::AppState::new_in_memory_empty_with_event_capacity(1).await;
        let events = state.chat_events.clone();
        let app = crate::app_with_state(state);

        use futures::StreamExt;

        let mut body = open_event_stream(app).await;

        // The subscriber never consumes: the queue overflows while the stream is
        // not polled.
        for index in 0..5 {
            events
                .send(chat_event_message(&format!("lagged-{index}")))
                .expect("the bus accepts events while the stream is open");
        }

        let mut acc = String::new();
        assert!(
            read_into(&mut body, &mut acc, Duration::from_secs(5), |acc| acc
                .contains("\"resync\""))
            .await,
            "the lagging subscriber must be told to resync, got: {acc:?}"
        );

        let seen = sse_payloads(&acc);
        assert_eq!(
            seen.first().map(|event| &event["type"]),
            Some(&Value::from("snapshot")),
            "the stream still opens with the snapshot, got: {seen:?}"
        );
        assert_eq!(
            seen.iter()
                .filter(|event| event["type"] == "resync")
                .count(),
            1,
            "exactly one resync, got: {seen:?}"
        );
        assert_eq!(
            seen.last().map(|event| &event["type"]),
            Some(&Value::from("resync")),
            "the resync must be the last event, got: {seen:?}"
        );

        // The stream is closed: a further event never reaches the client and the
        // body ends by itself.
        events
            .send(ChatEvent::TurnStarted)
            .expect("the bus is still alive");
        match tokio::time::timeout(Duration::from_millis(500), body.next()).await {
            Ok(None) => {}
            Ok(Some(Ok(chunk))) => panic!(
                "the stream must close after the resync, but it delivered {:?}",
                String::from_utf8_lossy(&chunk)
            ),
            Ok(Some(Err(error))) => panic!("the stream failed instead of closing: {error}"),
            Err(_) => panic!("the stream did not close after the resync"),
        }
    }

    /// Scenario (resync): un cliente que va al día nunca ve el aviso.
    ///
    /// El bus es el mismo diminuto del caso anterior: el `resync` depende del
    /// consumidor, no de la capacidad.
    #[tokio::test]
    async fn test_a_client_that_keeps_up_receives_its_messages_and_never_a_resync() {
        let state = crate::AppState::new_in_memory_empty_with_event_capacity(1).await;
        let events = state.chat_events.clone();
        let app = crate::app_with_state(state);

        let mut body = open_event_stream(app).await;
        let mut acc = String::new();

        // The snapshot opens the stream.
        assert!(
            read_into(&mut body, &mut acc, Duration::from_secs(5), |acc| {
                count_data_lines(acc) >= 1
            })
            .await,
            "the stream must open with the snapshot, got: {acc:?}"
        );

        // One message published and consumed before the next one: the subscriber
        // never falls behind.
        for index in 0..3 {
            let id = format!("kept-up-{index}");
            events
                .send(chat_event_message(&id))
                .expect("the bus accepts events while the stream is open");
            let expected = index + 2; // the snapshot plus the messages so far
            assert!(
                read_into(&mut body, &mut acc, Duration::from_secs(5), |acc| {
                    count_data_lines(acc) >= expected
                })
                .await,
                "the subscriber must receive the event of {id}, got: {acc:?}"
            );
        }

        let seen = sse_payloads(&acc);
        assert_eq!(seen[0]["type"], "snapshot");
        let ids: Vec<&str> = seen
            .iter()
            .filter(|event| event["type"] == "message_created")
            .filter_map(|event| event["message"]["id"].as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["kept-up-0", "kept-up-1", "kept-up-2"],
            "the client must receive its messages in order, got: {seen:?}"
        );
        assert!(
            seen.iter().all(|event| event["type"] != "resync"),
            "a client that keeps up must never be told to resync, got: {seen:?}"
        );
    }

    // ------------------------------------------------------------------
    // End-to-end: a real turn, through the production construction path,
    // reaches every subscriber over `GET /api/chat/events`
    // ------------------------------------------------------------------

    /// The `message_created` events of an accumulated SSE text, as
    /// `(role, cursor)` pairs.
    fn message_created_roles(text: &str) -> Vec<(String, String)> {
        sse_payloads(text)
            .into_iter()
            .filter(|event| event["type"] == "message_created")
            .map(|event| {
                (
                    event["message"]["role"].as_str().unwrap_or("").to_string(),
                    event["cursor"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    }

    /// Scenario: un cliente recibe el mensaje escrito en otro (y el resto de
    /// clientes se entera del turno).
    ///
    /// Es el escenario insignia del change, por el camino **de producción**: el
    /// estado se construye con [`crate::AppState::new_with_orchestrator_with_llm`],
    /// la misma ruta que recorre `main.rs` —solo que con un LLM de mentira, sin
    /// red—, de modo que se ejercita el cableado real del bus en el orquestador
    /// (`.with_event_bus`). Si esa línea desaparece de `src/lib.rs`, estos dos
    /// suscriptores dejan de recibir los `message_created` y el test falla.
    ///
    /// `new_in_memory_*` y `app()` no sirven aquí: construyen `orchestrator:
    /// None`, así que jamás tocarían el cableado del bus.
    #[tokio::test(flavor = "multi_thread")]
    #[serial_test::serial]
    async fn test_both_subscribers_receive_a_real_turn_end_to_end(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::config::Config;

        // A production-shaped state over a file database (as `main.rs` does),
        // with a deterministic mock LLM in place of the network provider.
        let tmp_dir = std::env::temp_dir();
        let db_path = tmp_dir.join("valet_test_chat_sync_end_to_end.db");
        let _ = std::fs::remove_file(&db_path);

        let mut config = Config::from_env();
        config.database_url = db_path
            .to_str()
            .expect("temp path must be valid UTF-8")
            .to_string();

        let state =
            crate::AppState::new_with_orchestrator_with_llm(&config, Arc::new(PlainMockLLM))
                .await?;

        // The production wiring must have attached the chat bus to the
        // orchestrator; without it, nothing below can reach the subscribers.
        assert!(
            state
                .orchestrator
                .as_ref()
                .expect("the production path wires an orchestrator")
                .events
                .is_some(),
            "the production construction path must wire the chat event bus into the orchestrator"
        );

        // Keep the turn off the network: even if `OPENROUTER_API_KEY` is present
        // in the environment, a disabled router never calls the classifier.
        crate::db::repos::settings::SettingsRepo::set(&state.db, "ROUTER_ENABLED", "false").await?;

        let app = crate::app_with_state(state);

        // Two clients subscribe before the turn starts.
        let mut first = open_event_stream(app.clone()).await;
        let mut second = open_event_stream(app.clone()).await;

        // A real `POST /api/chat/stream`, drained to the end so the turn
        // completes and both messages are persisted and published.
        let response = app
            .clone()
            .oneshot(stream_request(
                serde_json::json!({"content": "Hola desde el test end-to-end"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let turn_text = read_sse_until(response, Duration::from_secs(10), |_| false).await;
        assert!(
            turn_text.contains("\"type\":\"done\""),
            "the turn must complete with a done event, got: {turn_text:?}"
        );

        // Both subscribers —the one that originated the turn is not special—
        // must have received the two persisted messages and the turn marks.
        let mut first_text = String::new();
        let mut second_text = String::new();
        for (label, body, acc) in [
            ("first", &mut first, &mut first_text),
            ("second", &mut second, &mut second_text),
        ] {
            // Wait until *every* event the test asserts on has arrived: both
            // persisted messages *and* the turn marks. Stopping at the two
            // `message_created` events would race `turn_finished`, which is
            // published after them.
            let read = read_into(body, acc, Duration::from_secs(10), |acc| {
                let events = sse_payloads(acc);
                message_created_roles(acc).len() >= 2
                    && events.iter().any(|event| event["type"] == "turn_started")
                    && events.iter().any(|event| event["type"] == "turn_finished")
            })
            .await;
            assert!(
                read,
                "subscriber {label} must receive the two messages, got: {acc:?}"
            );
        }

        for (label, acc) in [("first", &first_text), ("second", &second_text)] {
            let events = sse_payloads(acc);
            let roles: Vec<String> = message_created_roles(acc)
                .into_iter()
                .map(|(role, _)| role)
                .collect();
            assert!(
                roles.iter().any(|role| role == "user"),
                "subscriber {label} must receive the user `message_created`, got: {events:?}"
            );
            assert!(
                roles.iter().any(|role| role == "assistant"),
                "subscriber {label} must receive the assistant `message_created`, got: {events:?}"
            );
            assert!(
                events.iter().any(|event| event["type"] == "turn_started"),
                "subscriber {label} must be told the turn started, got: {events:?}"
            );
            assert!(
                events.iter().any(|event| event["type"] == "turn_finished"),
                "subscriber {label} must be told the turn finished, got: {events:?}"
            );
        }

        // The teardown mirrors the other production-path tests.
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_chat_sync_end_to_end.db-wal"));
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_chat_sync_end_to_end.db-shm"));

        Ok(())
    }
}
