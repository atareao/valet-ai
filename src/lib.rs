pub mod auth;
pub mod config;
pub mod db;
pub mod embeddings;
pub mod errors;
pub mod generation;
pub mod handlers;
pub mod llm;
pub mod middleware;
pub mod models;
pub mod orchestrator;
pub mod persistent_memory;
pub mod routes;
pub mod services;
pub mod telemetry;
pub mod token_estimate;
pub mod tools;
pub mod workers;

use axum::http::{header, HeaderValue, Method};
use axum::routing::{delete, get};
use axum::{extract::State, Json, Router};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::config::Config;
use crate::orchestrator::agent::Orchestrator;
use crate::orchestrator::context_builder::ContextBuilder;
use crate::orchestrator::guardrails::Guardrails;
use crate::tools::registry::ToolRegistry;
use crate::workers::pool::WorkerPool;

/// Shared application state with an async SQLite connection pool.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub orchestrator: Option<Arc<Orchestrator>>,
    pub guardrails: Option<Arc<Guardrails>>,
    pub tool_registry: Option<Arc<ToolRegistry>>,
    pub auth_config: Option<crate::auth::AuthConfig>,
    pub collapse_tx: Option<mpsc::Sender<String>>,
    pub collapse_threshold_tokens: usize,
    pub memory_tx: Option<mpsc::Sender<()>>,
    pub shutdown_tx: Option<broadcast::Sender<()>>,
    pub last_api_call: Arc<RwLock<Option<crate::models::stats::LastApiCall>>>,
}

/// Build the production tool registry with all 20 built-in tools.
///
/// Exposed so the evaluation harness (`valet-route-eval`) can resolve the same
/// advertised tool set the running application uses.
pub fn build_tool_registry(pool: &SqlitePool) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(crate::tools::weather::WeatherTool::new(
        pool.clone(),
        std::env::var("OPENWEATHER_API_KEY").unwrap_or_default(),
    )));
    registry.register(Box::new(crate::tools::geo::GeocodeTool::new()));
    registry.register(Box::new(crate::tools::geo::ReverseGeocodeTool::new()));
    registry.register(Box::new(
        crate::tools::google_places::SearchPlacesTool::new(pool.clone()),
    ));
    registry.register(Box::new(crate::tools::web_search::WebSearchTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(crate::tools::calendar::CalendarTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(crate::tools::tasks::TasksTool::new(pool.clone())));
    registry.register(Box::new(crate::tools::reminders::RemindersTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(crate::tools::current_time::CurrentTimeTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(
        crate::tools::current_location::CurrentLocationTool::new(pool.clone()),
    ));
    registry.register(Box::new(crate::tools::notes::NotesTool::new(pool.clone())));
    registry.register(Box::new(
        crate::tools::unified_search::UnifiedSearchTool::new(pool.clone()),
    ));
    registry.register(Box::new(crate::tools::widget::RenderWidgetTool::new()));
    registry.register(Box::new(
        crate::tools::strava::StravaRecentActivitiesTool::new(pool.clone()),
    ));
    registry.register(Box::new(
        crate::tools::strava::StravaActivityDetailTool::new(pool.clone()),
    ));
    registry.register(Box::new(
        crate::tools::strava::StravaActivityStreamsTool::new(pool.clone()),
    ));
    registry.register(Box::new(crate::tools::strava::StravaAthleteStatsTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(
        crate::tools::timeline::TimelineGetEventsTool::new(pool.clone()),
    ));
    registry.register(Box::new(crate::tools::timeline::TimelineAddEventTool::new(
        pool.clone(),
    )));
    registry.register(Box::new(
        crate::tools::timeline::TimelineDeleteEventTool::new(pool.clone()),
    ));
    registry
}

impl AppState {
    /// Create a new AppState with an in-memory SQLite database and no seed data.
    /// Used by integration tests that need a clean state.
    pub async fn new_in_memory_empty() -> Self {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory database");
        db::schema::run_migrations(&pool)
            .await
            .expect("Failed to run migrations on in-memory database");
        let _ = db::fts::create_fts_triggers(&pool).await;
        let registry = build_tool_registry(&pool);
        let _ =
            db::repos::tools::ToolsRepo::sync_from_registry(&pool, &registry.definitions()).await;
        Self {
            db: pool,
            orchestrator: None,
            guardrails: None,
            tool_registry: Some(Arc::new(registry)),
            auth_config: None,
            collapse_tx: None,
            collapse_threshold_tokens: 2000,
            memory_tx: None,
            shutdown_tx: None,
            last_api_call: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new AppState with an in-memory SQLite database.
    /// Used by integration tests.
    pub async fn new_in_memory() -> Self {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory database");
        db::schema::run_migrations(&pool)
            .await
            .expect("Failed to run migrations on in-memory database");
        let _ = db::fts::create_fts_triggers(&pool).await;
        let registry = build_tool_registry(&pool);
        let _ =
            db::repos::tools::ToolsRepo::sync_from_registry(&pool, &registry.definitions()).await;
        // Seed test data with known IDs expected by integration tests
        let _ = Self::seed_test_data(&pool).await;
        Self {
            db: pool,
            orchestrator: None,
            guardrails: None,
            tool_registry: Some(Arc::new(registry)),
            auth_config: None,
            collapse_tx: None,
            collapse_threshold_tokens: 2000,
            memory_tx: None,
            shutdown_tx: None,
            last_api_call: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new AppState backed by a file-based database with a fully
    /// initialised orchestrator, tool registry, guardrails, and auth config.
    ///
    /// This is the production entry point used by `main.rs`.
    pub async fn new_with_orchestrator(
        config: &Config,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // 0. Fail-closed auth validation, before any side effect: an incomplete
        //    auth config (e.g. empty JWT_SECRET with auth enabled) must abort
        //    startup instead of silently running with a forgeable session.
        let auth_config = crate::auth::AuthConfig::from_config(config)?;

        // 1. Open database connection pool
        let pool = db::init_db(&config.database_url).await?;

        // 2. Create tool registry and reconcile the tools table with it
        let registry = build_tool_registry(&pool);
        crate::db::repos::tools::ToolsRepo::sync_from_registry(&pool, &registry.definitions())
            .await?;
        let tool_registry = Arc::new(registry);

        // 3. Create guardrails
        let guardrails = Arc::new(Guardrails::new(tool_registry.clone()));

        // 4. Create LLM provider (try OpenRouter first, fall back to Ollama)
        let llm_provider: Arc<dyn crate::llm::provider::LLMProvider> =
            if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
                let config = crate::llm::openrouter::OpenRouterConfig {
                    api_key,
                    model: std::env::var("OPENROUTER_MODEL")
                        .unwrap_or_else(|_| "anthropic/claude-sonnet-20241022".into()),
                    base_url: std::env::var("OPENROUTER_BASE_URL")
                        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),
                    max_retries: 3,
                    timeout_secs: 120,
                };
                Arc::new(crate::llm::openrouter::OpenRouterProvider::new(config))
            } else {
                let config = crate::llm::ollama::OllamaConfig {
                    base_url: std::env::var("OLLAMA_BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:11434".into()),
                    model: std::env::var("LLM_MODEL").unwrap_or_else(|_| "llama3.2:3b".into()),
                    timeout_secs: 120,
                    keep_alive: "5m".into(),
                };
                Arc::new(crate::llm::ollama::OllamaProvider::new(config))
            };

        // 5. Create embedding provider (None disables RAG and the memory worker)
        let embedding_provider: Option<Arc<dyn crate::embeddings::EmbeddingProvider>> =
            crate::embeddings::create_provider(config).map(Arc::from);

        // 5.5 Create worker pool (collapse + memory workers)
        let worker_pool = WorkerPool::start(
            pool.clone(),
            config,
            llm_provider.clone(),
            embedding_provider.clone(),
        );
        let collapse_tx = worker_pool.collapse_tx;
        let memory_tx = worker_pool.memory_tx;
        let shutdown_tx = worker_pool.shutdown_tx;

        // 6. Create orchestrator
        let mut context_builder = ContextBuilder::new();
        context_builder.pool = Some(pool.clone());
        context_builder.provider = embedding_provider;
        context_builder.rag_budget_tokens = std::env::var("RAG_BUDGET_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(800);
        let context_builder = Arc::new(context_builder);
        let model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "anthropic/claude-sonnet-20241022".into());
        let orchestrator_config = crate::orchestrator::agent::OrchestratorConfig {
            model,
            collapse_threshold_tokens: config.collapse_threshold_tokens,
            ..Default::default()
        };

        let last_api_call: Arc<RwLock<Option<crate::models::stats::LastApiCall>>> =
            Arc::new(RwLock::new(None));

        // Decisions classifier for per-turn skill routing. A missing or empty
        // `OPENROUTER_API_KEY` yields `None`, so routing falls open. The client
        // timeout is a hard ceiling; the effective one is imposed per request by
        // `ROUTER_TIMEOUT_MS` from the router. The model is overridden per
        // request by the router, so a default here is enough.
        let decisions: Option<Arc<dyn crate::llm::decisions::DecisionsProvider>> =
            crate::llm::decisions::JevDecisionsConfig::from_env(
                "typesafe/jev-1.13".to_string(),
                10_000,
            )
            .map(|c| {
                Arc::new(crate::llm::decisions::JevDecisionsProvider::new(c))
                    as Arc<dyn crate::llm::decisions::DecisionsProvider>
            });

        let orchestrator = Arc::new(
            Orchestrator::new(
                llm_provider,
                tool_registry.clone(),
                guardrails.clone(),
                context_builder,
                orchestrator_config,
                pool.clone(),
                collapse_tx.clone(),
                memory_tx.clone(),
                last_api_call.clone(),
            )
            .with_decisions(decisions),
        );

        // 7. Auth config was built and validated at step 0 (`auth_config`).
        Ok(Self {
            db: pool,
            orchestrator: Some(orchestrator),
            guardrails: Some(guardrails),
            tool_registry: Some(tool_registry),
            auth_config: Some(auth_config),
            collapse_tx,
            collapse_threshold_tokens: config.collapse_threshold_tokens,
            memory_tx,
            shutdown_tx,
            last_api_call,
        })
    }

    /// Insert records with well-known IDs so integration tests that reference
    /// hard-coded IDs (e.g. `some-id`, `conv-id`, `profile-id`) can pass.
    async fn seed_test_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        // Profile used by memory tests (id = "profile-id")
        sqlx::query(
            "INSERT OR IGNORE INTO profiles (id, name, avatar_url, preferences, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind("profile-id")
        .bind("Test User")
        .bind(Option::<String>::None)
        .bind("{}")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        // Message used by get_message tests
        sqlx::query(
            "INSERT OR IGNORE INTO messages (id, role, content, tokens_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind("msg-id")
        .bind("user")
        .bind("Hello from seeded data")
        .bind(0i64)
        .bind(&now)
        .execute(pool)
        .await?;

        // Memory used by delete_memory tests
        sqlx::query(
            "INSERT OR IGNORE INTO memory (id, content, tokens_count, created_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind("memory-id")
        .bind("Seeded memory")
        .bind(0i64)
        .bind(&now)
        .bind("{}")
        .execute(pool)
        .await?;

        Ok(())
    }
}

/// Health check handler.
///
/// Returns a JSON payload indicating server status, version, and whether the
/// database connection is healthy.
async fn health_handler(State(state): State<AppState>) -> Json<Value> {
    let db_status = match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "db": db_status,
    }))
}

/// Local development origins allowed when OIDC is disabled/unconfigured.
const DEV_CORS_ORIGINS: [&str; 2] = ["http://localhost:5173", "http://localhost:3000"];

/// Build the CORS layer with credentials enabled.
///
/// `allow_credentials(true)` can never coexist with the `*` wildcard (origin,
/// methods or headers) per the Fetch spec, and reflecting arbitrary request
/// origins while allowing credentials is equally unsafe. The allowed origins
/// are therefore always an explicit list: the origin(s) derived from the auth
/// config when enabled, or a fixed localhost list in dev. When no valid origin
/// is available the list is empty, so no `Access-Control-Allow-Origin` is sent.
fn cors_layer(state: &AppState) -> CorsLayer {
    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CACHE_CONTROL,
        ])
        .allow_credentials(true)
        .allow_origin(AllowOrigin::list(allowed_origins(state)))
}

/// Explicit allow-list of CORS origins (`scheme://host[:port]`).
fn allowed_origins(state: &AppState) -> Vec<HeaderValue> {
    match state.auth_config.as_ref() {
        Some(config) if config.enabled => [
            config.redirect_url.as_str(),
            config.post_logout_redirect_url.as_str(),
        ]
        .into_iter()
        .filter_map(origin_header)
        .collect(),
        _ => DEV_CORS_ORIGINS
            .into_iter()
            .filter_map(origin_header)
            .collect(),
    }
}

/// Normalise a URL to its `scheme://host[:port]` origin header, if valid.
fn origin_header(url: &str) -> Option<HeaderValue> {
    url::Url::parse(url)
        .ok()
        .map(|parsed| parsed.origin().ascii_serialization())
        .and_then(|origin| HeaderValue::from_str(&origin).ok())
}

/// Build the Axum [`Router`] with all routes and the given [`AppState`].
///
/// This is the primary entry point for the production server in `main.rs`.
pub fn app_with_state(state: AppState) -> Router {
    // CORS sits outside the session middleware so a preflight `OPTIONS` is
    // never rejected with a 401.
    let cors = cors_layer(&state);

    let api = Router::new()
        // Health
        .route("/api/health", get(health_handler))
        // Export
        .route("/api/export", get(routes::export::export_data))
        // Messages
        .route(
            "/api/messages",
            get(routes::messages::list_messages).post(routes::messages::create_message),
        )
        .route("/api/messages/{msg_id}", get(routes::messages::get_message))
        // Chat
        .route("/api/chat/init", get(routes::chat::chat_init))
        // Profile
        .route(
            "/api/profile",
            get(routes::profile::get_profile).put(routes::profile::update_profile),
        )
        // Memories
        .route(
            "/api/memories",
            get(routes::memories::list_memories).post(routes::memories::create_memory),
        )
        .route(
            "/api/memories/{id}",
            delete(routes::memories::delete_memory),
        )
        // Tools
        .route("/api/tools", get(routes::tools::list_tools))
        // Skills catalog (read-only; same session middleware as `/api/tools`)
        .route("/api/skills", get(handlers::skills::list_skills))
        // Settings
        .route(
            "/api/settings",
            get(routes::settings::get_settings).put(routes::settings::update_settings),
        )
        // Persistent memory (Capa C)
        .route(
            "/api/persistent-memory",
            get(routes::persistent_memory::get_persistent_memory)
                .put(routes::persistent_memory::update_persistent_memory)
                .delete(routes::persistent_memory::delete_persistent_memory),
        )
        // Events
        .merge(routes::events::routes())
        // Tasks
        .merge(routes::tasks::routes())
        // Stats
        .merge(routes::stats::routes())
        // Search
        .route("/api/search", get(handlers::search::search))
        // Streaming + approval
        .merge(routes::stream::routes())
        // Authentication (OIDC login / callback / me / logout)
        .merge(routes::auth::routes())
        // Strava integration (OAuth authorize/callback, status, disconnect)
        .merge(routes::strava::routes())
        // Session middleware: enforces a valid session on `/api/*` when auth
        // is enabled; a no-op when it is disabled (dev mode).
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::session_middleware,
        ));

    api.fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(cors)
        .with_state(state)
}

/// Convenience constructor that creates an ephemeral in-memory database and
/// returns a ready-to-use [`Router`].
///
/// Used by integration tests (e.g. `tests/api/health.rs`) that call
/// `valet::app()` without wiring their own state.
pub async fn app() -> Router {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("Failed to create in-memory database for tests");
    db::schema::run_migrations(&pool)
        .await
        .expect("Failed to run migrations on in-memory database");
    let _ = db::fts::create_fts_triggers(&pool).await;
    let registry = build_tool_registry(&pool);
    let _ = db::repos::tools::ToolsRepo::sync_from_registry(&pool, &registry.definitions()).await;
    let _ = AppState::seed_test_data(&pool).await;
    let state = AppState {
        db: pool,
        orchestrator: None,
        guardrails: None,
        tool_registry: Some(Arc::new(registry)),
        auth_config: None,
        collapse_tx: None,
        collapse_threshold_tokens: 2000,
        memory_tx: None,
        shutdown_tx: None,
        last_api_call: Arc::new(RwLock::new(None)),
    };
    app_with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    /// After `new_with_orchestrator`, the `shutdown_tx` field must be `Some`
    /// so that all worker tasks stay alive. Currently it is `None`, which
    /// causes workers to shut down immediately — this test enforces the fix.
    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn test_app_state_retains_shutdown_tx() {
        env::set_var("OPENROUTER_API_KEY", "sk-test-key-for-unit-test");
        env::set_var("OPENROUTER_MODEL", "test/model");

        let tmp_dir = env::temp_dir();
        let db_filename = "valet_test_shutdown_tx.db";
        let db_path = tmp_dir.join(db_filename);
        let _ = std::fs::remove_file(&db_path);

        use crate::config::Config;
        let mut test_config = Config::from_env();
        test_config.database_url = db_path
            .to_str()
            .expect("temp path must be valid UTF-8")
            .to_string();

        let state = AppState::new_with_orchestrator(&test_config)
            .await
            .expect("new_with_orchestrator should succeed with minimal env vars");

        // RED: This assertion will FAIL because shutdown_tx is None.
        // After the GREEN fix (moving sender from WorkerPool), it will pass.
        assert!(
            state.shutdown_tx.is_some(),
            "shutdown_tx is None — WorkerPool sender was dropped at function return"
        );

        // Teardown
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_shutdown_tx.db-wal"));
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_shutdown_tx.db-shm"));

        env::remove_var("OPENROUTER_API_KEY");
        env::remove_var("OPENROUTER_MODEL");
    }

    /// With `EMBEDDING_PROVIDER`/`EMBEDDING_MODEL` configured, the production
    /// wiring must hand a pool **and** a provider to the orchestrator's
    /// `ContextBuilder`, otherwise RAG silently falls back to no memories.
    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn test_new_with_orchestrator_wires_context_builder_with_embeddings() {
        env::set_var("EMBEDDING_PROVIDER", "ollama");
        env::set_var("EMBEDDING_MODEL", "all-minilm");

        let tmp_dir = env::temp_dir();
        let db_filename = "valet_test_wire_rag.db";
        let db_path = tmp_dir.join(db_filename);
        let _ = std::fs::remove_file(&db_path);

        use crate::config::Config;
        let mut test_config = Config::from_env();
        test_config.database_url = db_path
            .to_str()
            .expect("temp path must be valid UTF-8")
            .to_string();

        let state = AppState::new_with_orchestrator(&test_config)
            .await
            .expect("new_with_orchestrator should succeed with embeddings configured");

        let orchestrator = state
            .orchestrator
            .as_ref()
            .expect("orchestrator should be present");
        let context_builder = &orchestrator.context_builder;

        assert!(
            context_builder.pool.is_some(),
            "ContextBuilder.pool should be wired"
        );
        assert!(
            context_builder.provider.is_some(),
            "ContextBuilder.provider should be wired when embeddings are configured"
        );

        // Teardown
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_wire_rag.db-wal"));
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_wire_rag.db-shm"));

        env::remove_var("EMBEDDING_PROVIDER");
        env::remove_var("EMBEDDING_MODEL");
    }

    // -----------------------------------------------------------------------
    // `render_widget` must be registered in the production registry.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_build_tool_registry_includes_render_widget() {
        use crate::tools::permission::Permission;
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

        let registry = build_tool_registry(&pool);

        assert!(
            registry.get("render_widget").is_some(),
            "build_tool_registry must register `render_widget`"
        );

        let defs = registry.definitions();
        let render = defs
            .iter()
            .find(|d| d.name == "render_widget")
            .expect("`render_widget` must be advertised in definitions()");
        assert_eq!(
            render.parameters["properties"]["widget_name"]["type"], "string",
            "widget_name must be declared as a string"
        );
        assert_eq!(
            render.parameters["properties"]["data"]["type"], "object",
            "data must be declared as an object"
        );
        assert_eq!(
            registry.permission("render_widget", &serde_json::json!({})),
            Some(Permission::NoConfirm),
            "render_widget must not require confirmation"
        );
    }

    /// Fail-closed: with auth enabled but an empty `JWT_SECRET`, startup must
    /// abort with an error instead of running with a forgeable session cookie.
    #[tokio::test]
    #[serial]
    async fn test_new_with_orchestrator_fails_closed_when_auth_incomplete() {
        let mut config = Config::from_env();
        config.auth_enabled = true;
        config.auth_issuer_url = "https://issuer.example".into();
        config.auth_client_id = "client".into();
        config.auth_client_secret = "secret".into();
        config.auth_redirect_url = "http://localhost:3000/api/auth/callback".into();
        config.jwt_secret = String::new();

        let result = AppState::new_with_orchestrator(&config).await;

        assert!(
            result.is_err(),
            "an empty JWT_SECRET with auth enabled must abort startup"
        );
    }

    /// With a complete auth configuration the production entry point must
    /// still start successfully.
    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn test_new_with_orchestrator_ok_with_complete_auth_config() {
        env::set_var("OPENROUTER_API_KEY", "sk-test-key-for-unit-test");
        env::set_var("OPENROUTER_MODEL", "test/model");
        env::set_var("AUTH_ENABLED", "true");
        env::set_var("AUTH_ISSUER_URL", "https://issuer.example");
        env::set_var("AUTH_CLIENT_ID", "test-client");
        env::set_var("AUTH_CLIENT_SECRET", "test-secret");
        env::set_var(
            "AUTH_REDIRECT_URL",
            "https://app.example.com/api/auth/callback",
        );
        env::set_var("JWT_SECRET", "test-jwt-secret");

        let tmp_dir = env::temp_dir();
        let db_path = tmp_dir.join("valet_test_auth_complete.db");
        let _ = std::fs::remove_file(&db_path);

        let mut test_config = Config::from_env();
        test_config.database_url = db_path.to_str().unwrap().to_string();

        let state = AppState::new_with_orchestrator(&test_config)
            .await
            .expect("a complete auth config must start");

        let auth = state
            .auth_config
            .as_ref()
            .expect("auth_config must be wired");
        assert!(auth.enabled);
        assert_eq!(auth.client_id, "test-client");
        assert_eq!(auth.jwt_secret, "test-jwt-secret");

        // Teardown
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_auth_complete.db-wal"));
        let _ = std::fs::remove_file(tmp_dir.join("valet_test_auth_complete.db-shm"));

        for var in [
            "OPENROUTER_API_KEY",
            "OPENROUTER_MODEL",
            "AUTH_ENABLED",
            "AUTH_ISSUER_URL",
            "AUTH_CLIENT_ID",
            "AUTH_CLIENT_SECRET",
            "AUTH_REDIRECT_URL",
            "JWT_SECRET",
        ] {
            env::remove_var(var);
        }
    }
}
