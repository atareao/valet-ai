//! SSE streaming endpoint and approval resolution.
//!
//! These handlers connect the orchestrator's streaming output to the frontend
//! via Server-Sent Events.
//!
//! ## Routes
//!
//! * `POST /api/chat/stream`        — SSE stream of [`SSEEvent`]s
//! * `POST /api/approval/{request_id}` — Resolve a pending approval

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::stream::Stream;
use serde::Deserialize;
use serde_json::Value;
use std::convert::Infallible;
use std::pin::Pin;

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
}

/// Body payload for the approval resolution endpoint.
#[derive(Debug, Deserialize)]
pub struct ApprovalBody {
    pub approved: bool,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/chat/stream`
///
/// Sends a message to the orchestrator and returns the response as an SSE
/// stream of [`SSEEvent`] values.
///
/// When the orchestrator is not available (e.g. in tests), falls back to a
/// single static chunk to preserve backward compatibility.
pub async fn stream_message(
    State(state): State<AppState>,
    Json(query): Json<MessageQuery>,
) -> Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>> {
    tracing::info!(
        content_len = %query.content.len(),
        "📥 SSE stream request received"
    );

    // If orchestrator is not configured, fall back to stub response.
    if state.orchestrator.is_none() {
        tracing::warn!("Orchestrator is None, using fallback stub response");
        let event = SSEEvent::Chunk {
            content: "Hello from Valet!".to_string(),
        };
        let stream =
            futures::stream::once(async move { Ok(Event::default().data(event.to_json_string())) });
        return Sse::new(Box::pin(stream));
    }

    let orchestrator = state.orchestrator.unwrap();
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
            let stream =
                futures::stream::once(
                    async move { Ok::<_, Infallible>(Event::default().data("")) },
                );
            return Sse::new(Box::pin(stream));
        }
    };

    let content = query.content.clone();
    let browser_context = query.browser_context.clone();

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
        if let Err(e) = orchestrator
            .process_message_stream(&profile.id, &content, browser_context, tx.clone())
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
            yield Ok(Event::default().data(event.to_json_string()));
        }
    };

    Sse::new(Box::pin(stream))
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

/// Assemble the stream and approval routes into a sub-router.
pub fn routes() -> axum::Router<AppState> {
    use axum::routing::post;
    axum::Router::new()
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
}
