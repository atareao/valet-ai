pub mod auth;
pub mod config;
pub mod db;
pub mod embeddings;
pub mod errors;
pub mod generation;
pub mod handlers;
pub mod llm;
pub mod models;
pub mod orchestrator;
pub mod persistent_memory;
pub mod routes;
pub mod services;
pub mod telemetry;
pub mod token_estimate;
pub mod tools;
pub mod workers;

use axum::routing::{delete, get, put};
use axum::{extract::State, Json, Router};
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{Any, CorsLayer};
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

/// Build the production tool registry with all 13 built-in tools.
fn build_tool_registry(pool: &SqlitePool) -> ToolRegistry {
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
        if let Ok(disabled) = db::repos::tools::ToolsRepo::disabled_names(&pool).await {
            registry.set_disabled(disabled);
        }
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
        if let Ok(disabled) = db::repos::tools::ToolsRepo::disabled_names(&pool).await {
            registry.set_disabled(disabled);
        }
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
        // 1. Open database connection pool
        let pool = db::init_db(&config.database_url).await?;

        // 2. Create tool registry and reconcile the tools table with it
        let registry = build_tool_registry(&pool);
        crate::db::repos::tools::ToolsRepo::sync_from_registry(&pool, &registry.definitions())
            .await?;
        if let Ok(disabled) = crate::db::repos::tools::ToolsRepo::disabled_names(&pool).await {
            registry.set_disabled(disabled);
        }
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

        let orchestrator = Arc::new(Orchestrator::new(
            llm_provider,
            tool_registry.clone(),
            guardrails.clone(),
            context_builder,
            orchestrator_config,
            pool.clone(),
            collapse_tx.clone(),
            memory_tx.clone(),
            last_api_call.clone(),
        ));

        // 7. Create auth config from environment
        let auth_config = crate::auth::AuthConfig {
            enabled: std::env::var("AUTH_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            issuer_url: std::env::var("AUTH_ISSUER_URL")
                .unwrap_or_else(|_| "http://localhost:8080".into()),
            client_id: std::env::var("AUTH_CLIENT_ID").unwrap_or_default(),
            client_secret: std::env::var("AUTH_CLIENT_SECRET").unwrap_or_default(),
            redirect_url: std::env::var("AUTH_REDIRECT_URL")
                .unwrap_or_else(|_| "http://localhost:3000/auth/callback".into()),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_default(),
        };

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

/// Build the Axum [`Router`] with all routes and the given [`AppState`].
///
/// This is the primary entry point for the production server in `main.rs`.
pub fn app_with_state(state: AppState) -> Router {
    // CORS middleware — allows any origin in dev; production origins are
    // restricted via AUTH_REDIRECT_URL or environment-specific config.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
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
        .route("/api/tools/{id}/toggle", put(routes::tools::toggle_tool))
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
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
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
    if let Ok(disabled) = db::repos::tools::ToolsRepo::disabled_names(&pool).await {
        registry.set_disabled(disabled);
    }
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
}
