pub mod ollama;
pub mod openrouter;
pub mod provider;
pub mod reindex;

pub use provider::EmbeddingProvider;

use crate::config::Config;

/// Configuration for embedding providers, derived from [`Config`].
///
/// Embeddings are opt-in: there is deliberately no default model.
#[derive(Clone)]
pub struct EmbeddingConfig {
    /// Embedding backend: `"ollama"` or `"openrouter"`.
    pub provider: String,
    /// Model identifier used for embeddings (required, no default).
    pub model: String,
    /// Base URL of the Ollama server.
    pub ollama_url: String,
    /// API key for OpenRouter (empty when using Ollama).
    pub openrouter_api_key: String,
}

impl std::fmt::Debug for EmbeddingConfig {
    /// Redacts the OpenRouter API key so it never leaks into logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("ollama_url", &self.ollama_url)
            .field("openrouter_api_key", &"***")
            .finish()
    }
}

impl EmbeddingConfig {
    /// Build an [`EmbeddingConfig`] from the application [`Config`].
    ///
    /// Returns `None` when `EMBEDDING_PROVIDER` or `EMBEDDING_MODEL` are not
    /// defined, or when the provider is `"openrouter"` without an API key. In
    /// each case a specific warning is logged so the operator can tell the two
    /// situations apart.
    pub fn from_config(cfg: &Config) -> Option<Self> {
        let provider = match cfg.embedding_provider.clone() {
            Some(provider) => provider,
            None => {
                tracing::warn!("RAG disabled: EMBEDDING_PROVIDER/EMBEDDING_MODEL not configured");
                return None;
            }
        };
        let model = match cfg.embedding_model.clone() {
            Some(model) => model,
            None => {
                tracing::warn!("RAG disabled: EMBEDDING_PROVIDER/EMBEDDING_MODEL not configured");
                return None;
            }
        };

        let openrouter_api_key = cfg.openrouter_api_key.clone().unwrap_or_default();
        if provider == "openrouter" && openrouter_api_key.is_empty() {
            tracing::warn!(
                "RAG disabled: EMBEDDING_PROVIDER=openrouter requires OPENROUTER_API_KEY"
            );
            return None;
        }

        Some(Self {
            provider,
            model,
            ollama_url: cfg.ollama_base_url.clone(),
            openrouter_api_key,
        })
    }
}

/// Build the embedding provider from the application [`Config`].
///
/// Returns `None` when embeddings are not configured. The reason is logged by
/// [`EmbeddingConfig::from_config`], so `None` already means "RAG disabled".
pub fn create_provider(cfg: &Config) -> Option<Box<dyn EmbeddingProvider>> {
    let ec = EmbeddingConfig::from_config(cfg)?;

    match ec.provider.as_str() {
        "openrouter" => Some(Box::new(openrouter::OpenRouterProvider::new(
            ec.openrouter_api_key,
            ec.model,
        ))),
        "ollama" => Some(Box::new(ollama::OllamaProvider::new(
            Some(ec.ollama_url),
            ec.model,
        ))),
        other => {
            tracing::warn!("RAG disabled: unknown EMBEDDING_PROVIDER '{other}'");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// A fully-populated [`Config`] with embeddings disabled.
    fn base_config() -> Config {
        Config {
            host: "0.0.0.0".into(),
            port: 3000,
            database_url: "valet.db".into(),
            log_level: "info".into(),
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
            rag_budget_tokens: 2000,
            embedding_provider: None,
            embedding_model: None,
            embedding_dimension: None,
        }
    }

    #[test]
    fn from_config_returns_none_without_provider_or_model() {
        let cfg = base_config();
        assert!(EmbeddingConfig::from_config(&cfg).is_none());
    }

    #[test]
    fn from_config_ollama_with_provider_and_model() {
        let mut cfg = base_config();
        cfg.embedding_provider = Some("ollama".into());
        cfg.embedding_model = Some("all-minilm".into());

        let ec = EmbeddingConfig::from_config(&cfg).expect("should be Some");
        assert_eq!(ec.provider, "ollama");
        assert_eq!(ec.model, "all-minilm");
        assert_eq!(ec.ollama_url, "http://localhost:11434");
    }

    #[test]
    fn from_config_openrouter_with_key() {
        let mut cfg = base_config();
        cfg.embedding_provider = Some("openrouter".into());
        cfg.embedding_model = Some("openai/text-embedding-3-small".into());
        cfg.openrouter_api_key = Some("sk-test".into());

        let ec = EmbeddingConfig::from_config(&cfg).expect("should be Some");
        assert_eq!(ec.provider, "openrouter");
        assert_eq!(ec.model, "openai/text-embedding-3-small");
        assert_eq!(ec.openrouter_api_key, "sk-test");
    }

    #[test]
    fn from_config_openrouter_without_key_is_none() {
        let mut cfg = base_config();
        cfg.embedding_provider = Some("openrouter".into());
        cfg.embedding_model = Some("openai/text-embedding-3-small".into());
        cfg.openrouter_api_key = None;

        assert!(EmbeddingConfig::from_config(&cfg).is_none());
    }

    #[test]
    fn create_provider_returns_none_without_config() {
        let cfg = base_config();
        assert!(create_provider(&cfg).is_none());
    }

    #[test]
    fn create_provider_ollama_configured() {
        let mut cfg = base_config();
        cfg.embedding_provider = Some("ollama".into());
        cfg.embedding_model = Some("all-minilm".into());

        assert!(create_provider(&cfg).is_some());
    }

    /// `Debug` for [`EmbeddingConfig`] must never leak the OpenRouter API key.
    #[test]
    fn debug_redacts_openrouter_api_key() {
        let mut cfg = base_config();
        cfg.embedding_provider = Some("openrouter".into());
        cfg.embedding_model = Some("openai/text-embedding-3-small".into());
        cfg.openrouter_api_key = Some("sk-super-secret-key".into());

        let ec = EmbeddingConfig::from_config(&cfg).expect("should be Some");
        let rendered = format!("{ec:?}");

        assert!(
            !rendered.contains("sk-super-secret-key"),
            "Debug leaked the API key: {rendered}"
        );
        assert!(
            rendered.contains("openrouter_api_key: \"***\""),
            "Debug did not redact the API key: {rendered}"
        );
    }
}
