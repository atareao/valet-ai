use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;

use super::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider, StreamEvent};

/// A provider that chains multiple [`LLMProvider`]s in priority order.
///
/// If the primary provider fails, the next provider in the list is tried,
/// and so on. If all providers fail, the last error is returned.
pub struct FallbackProvider {
    providers: Vec<Box<dyn LLMProvider>>,
}

impl FallbackProvider {
    pub fn new(providers: Vec<Box<dyn LLMProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl LLMProvider for FallbackProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
        let mut last_error = LLMError::Internal("No providers configured".into());
        for provider in &self.providers {
            match provider.chat(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = e;
                    continue;
                }
            }
        }
        Err(last_error)
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError> {
        let mut last_error = LLMError::Internal("No providers configured".into());
        for provider in &self.providers {
            match provider.chat_stream(request.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    last_error = e;
                    continue;
                }
            }
        }
        Err(last_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ollama::{OllamaConfig, OllamaProvider};
    use crate::llm::openrouter::{OpenRouterConfig, OpenRouterProvider};

    #[tokio::test]
    async fn test_fallback_creation() -> Result<(), Box<dyn std::error::Error>> {
        let config1 = OllamaConfig {
            base_url: "http://localhost:19999".into(),
            model: "test".into(),
            timeout_secs: 1,
            keep_alive: "1m".into(),
        };
        let provider1: Box<dyn LLMProvider> = Box::new(OllamaProvider::new(config1));
        let config2 = OpenRouterConfig {
            api_key: "test".into(),
            model: "test".into(),
            base_url: "http://localhost:19999".into(),
            max_retries: 0,
            timeout_secs: 1,
        };
        let provider2: Box<dyn LLMProvider> = Box::new(OpenRouterProvider::new(config2));
        let fallback = FallbackProvider::new(vec![provider1, provider2]);
        let result = fallback
            .chat(ChatRequest {
                model: "test".into(),
                messages: vec![],
                tools: None,
                temperature: None,
                max_tokens: None,
                stream: false,
                reasoning: None,
                response_format: None,
            })
            .await;
        // Both providers should fail (no server running)
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_fallback_all_fail_returns_last_error() -> Result<(), Box<dyn std::error::Error>> {
        let config = OllamaConfig {
            base_url: "http://localhost:19998".into(),
            model: "test".into(),
            timeout_secs: 1,
            keep_alive: "1m".into(),
        };
        let provider: Box<dyn LLMProvider> = Box::new(OllamaProvider::new(config));
        let fallback = FallbackProvider::new(vec![provider]);
        let result = fallback
            .chat(ChatRequest {
                model: "test".into(),
                messages: vec![],
                tools: None,
                temperature: None,
                max_tokens: None,
                stream: false,
                reasoning: None,
                response_format: None,
            })
            .await;
        assert!(result.is_err());
        Ok(())
    }
}
