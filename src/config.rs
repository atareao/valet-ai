use std::env;

/// Centralized configuration for Valet.
///
/// All environment variables are read once via [`Config::from_env`] and
/// exposed as typed fields with sensible defaults.
#[derive(Debug, Clone)]
pub struct Config {
    // Server
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub log_level: String,

    // LLM
    pub openrouter_api_key: Option<String>,
    pub openrouter_model: String,
    pub openrouter_base_url: String,
    pub ollama_base_url: String,
    pub ollama_model: String,

    // Auth
    pub auth_enabled: bool,
    pub auth_issuer_url: String,
    pub auth_client_id: String,
    pub auth_client_secret: String,
    pub auth_redirect_url: String,
    pub auth_post_logout_redirect_url: String,
    pub jwt_secret: String,

    // Weather
    pub openweather_api_key: Option<String>,

    // Google Places
    pub google_places_api_key: Option<String>,

    // Brave Search
    pub brave_search_api_key: Option<String>,

    // Message collapse threshold
    pub collapse_threshold_tokens: usize,
    // Model used for collapse/summary
    pub collapse_model: String,

    // Episodic Memory
    pub memory_batch_tokens: usize,
    pub memory_inactivity_minutes: u64,
    pub memory_overlap: i64,
    pub memory_poll_interval_minutes: u64,
    pub memory_model: String,
    /// Model used by the persistent-memory consolidator. Reads `SEMANTIC_MODEL`
    /// and falls back to `MEMORY_MODEL` (and, in turn, to its default).
    pub semantic_model: String,
    pub rag_budget_tokens: usize,

    // Embeddings (RAG)
    pub embedding_provider: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimension: Option<usize>,
}

impl Config {
    /// Build a [`Config`] from environment variables, applying sensible
    /// defaults whenever a variable is not set or cannot be parsed.
    pub fn from_env() -> Self {
        // `SEMANTIC_MODEL` is the consolidator's model; when it is not set it
        // falls back to `MEMORY_MODEL` (which in turn has its own default).
        let memory_model = env::var("MEMORY_MODEL")
            .unwrap_or_else(|_| "mistralai/mistral-small-24b-instruct-2501".into());
        let semantic_model = env::var("SEMANTIC_MODEL").unwrap_or_else(|_| memory_model.clone());

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "valet.db".into()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),

            openrouter_api_key: env::var("OPENROUTER_API_KEY").ok(),
            openrouter_model: env::var("OPENROUTER_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-20241022".into()),
            openrouter_base_url: env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".into()),
            ollama_model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".into()),

            auth_enabled: env::var("AUTH_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            auth_issuer_url: env::var("AUTH_ISSUER_URL").unwrap_or_default(),
            auth_client_id: env::var("AUTH_CLIENT_ID").unwrap_or_default(),
            auth_client_secret: env::var("AUTH_CLIENT_SECRET").unwrap_or_default(),
            // Fail-closed: no non-empty default. With `AUTH_ENABLED=true` and
            // these unset, `AuthConfig::validate()` aborts startup rather than
            // pointing the callback at an inexistent route or injecting a stray
            // origin into the credentialed CORS allow-list.
            auth_redirect_url: env::var("AUTH_REDIRECT_URL").unwrap_or_default(),
            auth_post_logout_redirect_url: env::var("AUTH_POST_LOGOUT_REDIRECT_URL")
                .unwrap_or_default(),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_default(),

            openweather_api_key: env::var("OPENWEATHER_API_KEY").ok(),

            google_places_api_key: env::var("GOOGLE_PLACES_API_KEY").ok(),
            brave_search_api_key: env::var("BRAVE_SEARCH_API_KEY").ok(),

            collapse_threshold_tokens: env::var("COLLAPSE_THRESHOLD_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
            collapse_model: env::var("COLLAPSE_MODEL")
                .unwrap_or_else(|_| "mistralai/mistral-small-24b-instruct-2501".into()),

            memory_batch_tokens: env::var("MEMORY_BATCH_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
            memory_inactivity_minutes: env::var("MEMORY_INACTIVITY_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            memory_overlap: env::var("MEMORY_OVERLAP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            memory_poll_interval_minutes: env::var("MEMORY_POLL_INTERVAL_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            memory_model,
            semantic_model,
            rag_budget_tokens: env::var("RAG_BUDGET_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(800),

            // Embeddings are opt-in: no model is defaulted.
            embedding_provider: env::var("EMBEDDING_PROVIDER").ok(),
            embedding_model: env::var("EMBEDDING_MODEL").ok(),
            embedding_dimension: match env::var("EMBEDDING_DIMENSION") {
                Ok(raw) => match raw.parse::<usize>() {
                    Ok(dimension) => Some(dimension),
                    Err(_) => {
                        tracing::warn!(
                            "EMBEDDING_DIMENSION='{raw}' is not a valid usize; ignoring it"
                        );
                        None
                    }
                },
                Err(_) => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// When no environment variables are set, [`Config::from_env`] must
    /// return the documented default values.
    #[test]
    #[serial]
    fn test_config_defaults() {
        // Unset any variables that may be set in the test environment
        // so we always get defaults.
        for var in [
            "HOST",
            "PORT",
            "DATABASE_URL",
            "LOG_LEVEL",
            "OPENROUTER_API_KEY",
            "OPENROUTER_MODEL",
            "OPENROUTER_BASE_URL",
            "OLLAMA_BASE_URL",
            "OLLAMA_MODEL",
            "AUTH_ENABLED",
            "AUTH_ISSUER_URL",
            "AUTH_CLIENT_ID",
            "AUTH_CLIENT_SECRET",
            "AUTH_REDIRECT_URL",
            "AUTH_POST_LOGOUT_REDIRECT_URL",
            "JWT_SECRET",
            "OPENWEATHER_API_KEY",
            "GOOGLE_PLACES_API_KEY",
            "BRAVE_SEARCH_API_KEY",
            "COLLAPSE_THRESHOLD_TOKENS",
            "COLLAPSE_MODEL",
            "MEMORY_BATCH_TOKENS",
            "MEMORY_INACTIVITY_MINUTES",
            "MEMORY_OVERLAP",
            "MEMORY_POLL_INTERVAL_MINUTES",
            "MEMORY_MODEL",
            "SEMANTIC_MODEL",
            "RAG_BUDGET_TOKENS",
            "EMBEDDING_PROVIDER",
            "EMBEDDING_MODEL",
            "EMBEDDING_DIMENSION",
        ] {
            env::remove_var(var);
        }

        let cfg = Config::from_env();

        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.database_url, "valet.db");
        assert_eq!(cfg.log_level, "info");

        assert!(cfg.openrouter_api_key.is_none());
        assert_eq!(cfg.openrouter_model, "anthropic/claude-sonnet-20241022");
        assert_eq!(cfg.openrouter_base_url, "https://openrouter.ai/api/v1");
        assert_eq!(cfg.ollama_base_url, "http://localhost:11434");
        assert_eq!(cfg.ollama_model, "llama3.2:3b");

        assert!(!cfg.auth_enabled);
        assert_eq!(cfg.auth_issuer_url, "");
        assert_eq!(cfg.auth_client_id, "");
        assert_eq!(cfg.auth_client_secret, "");
        // Fail-closed: an unset redirect URL must default to empty so that
        // enabling auth without configuring it aborts startup.
        assert_eq!(cfg.auth_redirect_url, "");
        assert_eq!(cfg.auth_post_logout_redirect_url, "");
        assert_eq!(cfg.jwt_secret, "");

        assert!(cfg.openweather_api_key.is_none());
        assert!(cfg.google_places_api_key.is_none());
        assert!(cfg.brave_search_api_key.is_none());

        assert_eq!(cfg.collapse_threshold_tokens, 2000);
        assert_eq!(
            cfg.collapse_model,
            "mistralai/mistral-small-24b-instruct-2501"
        );

        assert_eq!(cfg.memory_batch_tokens, 2000);
        assert_eq!(cfg.memory_inactivity_minutes, 30);
        assert_eq!(cfg.memory_overlap, 2);
        assert_eq!(cfg.memory_poll_interval_minutes, 30);
        assert_eq!(
            cfg.memory_model,
            "mistralai/mistral-small-24b-instruct-2501"
        );
        assert_eq!(cfg.rag_budget_tokens, 800);
        assert_eq!(
            cfg.semantic_model, "mistralai/mistral-small-24b-instruct-2501",
            "without SEMANTIC_MODEL, semantic_model falls back to the MEMORY_MODEL default"
        );

        assert!(cfg.embedding_provider.is_none());
        assert!(cfg.embedding_model.is_none());
        assert!(cfg.embedding_dimension.is_none());
    }

    /// When environment variables are set, [`Config::from_env`] must pick
    /// them up correctly, including type parsing and optional handling.
    #[test]
    #[serial]
    fn test_config_custom_values() {
        // The test asserts the SEMANTIC_MODEL fallback, so it must start from a
        // clean slate: an ambient SEMANTIC_MODEL would otherwise override it.
        env::remove_var("SEMANTIC_MODEL");

        // Set custom values
        env::set_var("HOST", "127.0.0.1");
        env::set_var("PORT", "9090");
        env::set_var("DATABASE_URL", "custom.db");
        env::set_var("LOG_LEVEL", "debug");
        env::set_var("OPENROUTER_API_KEY", "sk-or-v1-test-key");
        env::set_var("OPENROUTER_MODEL", "anthropic/claude-opus-20240229");
        env::set_var("OPENROUTER_BASE_URL", "https://custom.openrouter.ai/v1");
        env::set_var("OLLAMA_BASE_URL", "http://ollama.local:11434");
        env::set_var("OLLAMA_MODEL", "mistral:7b");
        env::set_var("AUTH_ENABLED", "true");
        env::set_var("AUTH_ISSUER_URL", "https://auth.example.com");
        env::set_var("AUTH_CLIENT_ID", "my-client");
        env::set_var("AUTH_CLIENT_SECRET", "my-secret");
        env::set_var("AUTH_REDIRECT_URL", "https://app.example.com/callback");
        env::set_var("JWT_SECRET", "super-secret-key");
        env::set_var("OPENWEATHER_API_KEY", "weather-key-123");
        env::set_var("GOOGLE_PLACES_API_KEY", "google-places-key-456");
        env::set_var("BRAVE_SEARCH_API_KEY", "brave-search-key-789");
        env::set_var("COLLAPSE_THRESHOLD_TOKENS", "500");
        env::set_var("COLLAPSE_MODEL", "google/gemini-2.0-flash-lite");
        env::set_var("MEMORY_BATCH_TOKENS", "5000");
        env::set_var("MEMORY_INACTIVITY_MINUTES", "15");
        env::set_var("MEMORY_OVERLAP", "3");
        env::set_var("MEMORY_POLL_INTERVAL_MINUTES", "10");
        env::set_var("MEMORY_MODEL", "google/gemini-2.0-flash-lite");
        env::set_var("RAG_BUDGET_TOKENS", "4000");
        env::set_var("EMBEDDING_PROVIDER", "openrouter");
        env::set_var("EMBEDDING_MODEL", "openai/text-embedding-3-small");
        env::set_var("EMBEDDING_DIMENSION", "1536");

        let cfg = Config::from_env();

        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 9090);
        assert_eq!(cfg.database_url, "custom.db");
        assert_eq!(cfg.log_level, "debug");

        assert_eq!(cfg.openrouter_api_key.as_deref(), Some("sk-or-v1-test-key"));
        assert_eq!(cfg.openrouter_model, "anthropic/claude-opus-20240229");
        assert_eq!(cfg.openrouter_base_url, "https://custom.openrouter.ai/v1");
        assert_eq!(cfg.ollama_base_url, "http://ollama.local:11434");
        assert_eq!(cfg.ollama_model, "mistral:7b");

        assert!(cfg.auth_enabled);
        assert_eq!(cfg.auth_issuer_url, "https://auth.example.com");
        assert_eq!(cfg.auth_client_id, "my-client");
        assert_eq!(cfg.auth_client_secret, "my-secret");
        assert_eq!(cfg.auth_redirect_url, "https://app.example.com/callback");
        assert_eq!(cfg.jwt_secret, "super-secret-key");

        assert_eq!(cfg.openweather_api_key.as_deref(), Some("weather-key-123"));
        assert_eq!(
            cfg.google_places_api_key.as_deref(),
            Some("google-places-key-456")
        );
        assert_eq!(
            cfg.brave_search_api_key.as_deref(),
            Some("brave-search-key-789")
        );

        assert_eq!(cfg.collapse_threshold_tokens, 500);
        assert_eq!(cfg.collapse_model, "google/gemini-2.0-flash-lite");
        assert_eq!(cfg.memory_batch_tokens, 5000);
        assert_eq!(cfg.memory_inactivity_minutes, 15);
        assert_eq!(cfg.memory_overlap, 3);
        assert_eq!(cfg.memory_poll_interval_minutes, 10);
        assert_eq!(cfg.memory_model, "google/gemini-2.0-flash-lite");
        assert_eq!(cfg.rag_budget_tokens, 4000);
        assert_eq!(
            cfg.semantic_model, "google/gemini-2.0-flash-lite",
            "without SEMANTIC_MODEL, semantic_model falls back to MEMORY_MODEL"
        );

        assert_eq!(cfg.embedding_provider.as_deref(), Some("openrouter"));
        assert_eq!(
            cfg.embedding_model.as_deref(),
            Some("openai/text-embedding-3-small")
        );
        assert_eq!(cfg.embedding_dimension, Some(1536));

        // Clean up to avoid polluting other tests
        for var in [
            "HOST",
            "PORT",
            "DATABASE_URL",
            "LOG_LEVEL",
            "OPENROUTER_API_KEY",
            "OPENROUTER_MODEL",
            "OPENROUTER_BASE_URL",
            "OLLAMA_BASE_URL",
            "OLLAMA_MODEL",
            "AUTH_ENABLED",
            "AUTH_ISSUER_URL",
            "AUTH_CLIENT_ID",
            "AUTH_CLIENT_SECRET",
            "AUTH_REDIRECT_URL",
            "AUTH_POST_LOGOUT_REDIRECT_URL",
            "JWT_SECRET",
            "OPENWEATHER_API_KEY",
            "GOOGLE_PLACES_API_KEY",
            "BRAVE_SEARCH_API_KEY",
            "COLLAPSE_THRESHOLD_TOKENS",
            "COLLAPSE_MODEL",
            "MEMORY_BATCH_TOKENS",
            "MEMORY_INACTIVITY_MINUTES",
            "MEMORY_OVERLAP",
            "MEMORY_POLL_INTERVAL_MINUTES",
            "MEMORY_MODEL",
            "SEMANTIC_MODEL",
            "RAG_BUDGET_TOKENS",
            "EMBEDDING_PROVIDER",
            "EMBEDDING_MODEL",
            "EMBEDDING_DIMENSION",
        ] {
            env::remove_var(var);
        }
    }

    /// When COLLAPSE_THRESHOLD_TOKENS is not set, the default must be 2000.
    #[test]
    #[serial]
    fn test_config_collapse_threshold_default() {
        for var in [
            "HOST",
            "PORT",
            "DATABASE_URL",
            "LOG_LEVEL",
            "OPENROUTER_API_KEY",
            "OPENROUTER_MODEL",
            "OPENROUTER_BASE_URL",
            "OLLAMA_BASE_URL",
            "OLLAMA_MODEL",
            "AUTH_ENABLED",
            "AUTH_ISSUER_URL",
            "AUTH_CLIENT_ID",
            "AUTH_CLIENT_SECRET",
            "AUTH_REDIRECT_URL",
            "AUTH_POST_LOGOUT_REDIRECT_URL",
            "JWT_SECRET",
            "OPENWEATHER_API_KEY",
            "GOOGLE_PLACES_API_KEY",
            "BRAVE_SEARCH_API_KEY",
            "COLLAPSE_THRESHOLD_TOKENS",
            "COLLAPSE_MODEL",
            "MEMORY_BATCH_TOKENS",
            "MEMORY_INACTIVITY_MINUTES",
            "MEMORY_OVERLAP",
            "MEMORY_POLL_INTERVAL_MINUTES",
            "MEMORY_MODEL",
            "SEMANTIC_MODEL",
            "RAG_BUDGET_TOKENS",
            "EMBEDDING_PROVIDER",
            "EMBEDDING_MODEL",
            "EMBEDDING_DIMENSION",
        ] {
            env::remove_var(var);
        }

        let cfg = Config::from_env();
        assert_eq!(cfg.collapse_threshold_tokens, 2000);
    }

    /// When COLLAPSE_MODEL is not set, the default must be "mistralai/mistral-small-24b-instruct-2501".
    #[test]
    #[serial]
    fn test_config_collapse_model_default() {
        for var in [
            "HOST",
            "PORT",
            "DATABASE_URL",
            "LOG_LEVEL",
            "OPENROUTER_API_KEY",
            "OPENROUTER_MODEL",
            "OPENROUTER_BASE_URL",
            "OLLAMA_BASE_URL",
            "OLLAMA_MODEL",
            "AUTH_ENABLED",
            "AUTH_ISSUER_URL",
            "AUTH_CLIENT_ID",
            "AUTH_CLIENT_SECRET",
            "AUTH_REDIRECT_URL",
            "AUTH_POST_LOGOUT_REDIRECT_URL",
            "JWT_SECRET",
            "OPENWEATHER_API_KEY",
            "GOOGLE_PLACES_API_KEY",
            "BRAVE_SEARCH_API_KEY",
            "COLLAPSE_THRESHOLD_TOKENS",
            "COLLAPSE_MODEL",
            "MEMORY_BATCH_TOKENS",
            "MEMORY_INACTIVITY_MINUTES",
            "MEMORY_OVERLAP",
            "MEMORY_POLL_INTERVAL_MINUTES",
            "MEMORY_MODEL",
            "SEMANTIC_MODEL",
            "RAG_BUDGET_TOKENS",
            "EMBEDDING_PROVIDER",
            "EMBEDDING_MODEL",
            "EMBEDDING_DIMENSION",
        ] {
            env::remove_var(var);
        }

        let cfg = Config::from_env();
        assert_eq!(
            cfg.collapse_model,
            "mistralai/mistral-small-24b-instruct-2501"
        );
    }

    /// An unparseable `EMBEDDING_DIMENSION` must be ignored (left as `None`)
    /// instead of silently discarding the parse failure.
    #[test]
    #[serial]
    fn test_config_embedding_dimension_invalid_is_none() {
        env::set_var("EMBEDDING_DIMENSION", "not-a-number");

        let cfg = Config::from_env();
        assert!(cfg.embedding_dimension.is_none());

        env::remove_var("EMBEDDING_DIMENSION");
    }

    /// `SEMANTIC_MODEL` overrides `MEMORY_MODEL` when both are set.
    #[test]
    #[serial]
    fn test_config_semantic_model_overrides_memory_model() {
        env::set_var("MEMORY_MODEL", "memory/model");
        env::set_var("SEMANTIC_MODEL", "semantic/model");

        let cfg = Config::from_env();
        assert_eq!(cfg.semantic_model, "semantic/model");
        assert_eq!(cfg.memory_model, "memory/model");

        env::remove_var("MEMORY_MODEL");
        env::remove_var("SEMANTIC_MODEL");
    }

    /// Without `SEMANTIC_MODEL`, `semantic_model` falls back to `MEMORY_MODEL`.
    #[test]
    #[serial]
    fn test_config_semantic_model_falls_back_to_memory_model() {
        env::remove_var("SEMANTIC_MODEL");
        env::set_var("MEMORY_MODEL", "memory/only");

        let cfg = Config::from_env();
        assert_eq!(cfg.semantic_model, "memory/only");

        env::remove_var("MEMORY_MODEL");
    }

    /// With neither set, `semantic_model` uses the shared default.
    #[test]
    #[serial]
    fn test_config_semantic_model_default() {
        env::remove_var("MEMORY_MODEL");
        env::remove_var("SEMANTIC_MODEL");

        let cfg = Config::from_env();
        assert_eq!(
            cfg.semantic_model,
            "mistralai/mistral-small-24b-instruct-2501"
        );
    }
}
