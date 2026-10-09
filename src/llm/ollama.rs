use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::Stream;

use super::provider::{
    ChatMessage, ChatRequest, ChatResponse, LLMError, LLMProvider, StreamEvent, ToolCall,
};

/// Configuration for the Ollama LLM provider.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
    pub keep_alive: String,
}

/// Provider that communicates with a local Ollama instance.
pub struct OllamaProvider {
    config: OllamaConfig,
    client: Client,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// Parse an Ollama JSON response body into a `ChatResponse`.
    ///
    /// This method is extracted for testability — it can be tested without an HTTP server.
    ///
    /// Ollama returns `tool_calls` in `body["message"]["tool_calls"]` as an array.
    /// Each tool call has `function.name` (string) and `function.arguments`
    /// (a JSON object used directly, NOT a string). Ollama does not provide an
    /// `id`, so sequential ids `"ollama-call-0"`, `"ollama-call-1"`, ... are generated.
    fn parse_response(body: &Value) -> Result<ChatResponse, LLMError> {
        let content = body["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let tool_calls = body["message"]["tool_calls"].as_array().map(|calls| {
            calls
                .iter()
                .enumerate()
                .map(|(i, call)| {
                    let id = format!("ollama-call-{}", i);
                    let name = call["function"]["name"].as_str().unwrap_or("").to_string();
                    let arguments = call["function"]["arguments"].clone();
                    ToolCall {
                        id,
                        name,
                        arguments,
                    }
                })
                .collect()
        });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".into(),
                content,
                tool_calls,
                tool_result: None,
                tool_call_id: None,
            },
            usage: None,
        })
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
        let url = format!("{}/api/chat", self.config.base_url);
        let body = serde_json::json!({
            "model": self.config.model,
            "messages": request.messages.iter().map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(ref tool_call_id) = m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(tool_call_id);
                }
                if let Some(ref tool_calls) = m.tool_calls {
                    msg["tool_calls"] = serde_json::json!(tool_calls.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments.to_string(),
                            }
                        })
                    }).collect::<Vec<_>>());
                }
                msg
            }).collect::<Vec<_>>(),
            "stream": false,
            "keep_alive": self.config.keep_alive,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout(e.to_string())
                } else if e.is_connect() {
                    LLMError::HttpError(format!("Connection refused: {}", e))
                } else {
                    LLMError::HttpError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            // Read the response body to get error details from the API
            let body_text = response
                .text()
                .await
                .map_err(|e| LLMError::HttpError(format!("Failed to read error body: {}", e)))?;
            let details = if body_text.is_empty() {
                format!("HTTP {}", status)
            } else if let Ok(body_json) = serde_json::from_str::<Value>(&body_text) {
                let msg = body_json["error"]
                    .as_str()
                    .or_else(|| body_json["message"].as_str())
                    .unwrap_or(&body_text);
                format!("HTTP {} — {}", status, msg)
            } else {
                format!("HTTP {} — {}", status, body_text.trim())
            };
            if status == 429 {
                return Err(LLMError::RateLimited { retry_after: 30 });
            }
            return Err(LLMError::HttpError(details));
        }

        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LLMError::HttpError(format!("Failed to parse: {}", e)))?;

        Self::parse_response(&response_body)
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError> {
        let result = self.chat(request).await?;
        let stream = futures::stream::once(async move { Ok(StreamEvent::Done(result)) });
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::{ReasoningSpec, ResponseFormat};

    #[test]
    fn test_ollama_config_defaults() {
        let config = OllamaConfig {
            base_url: "http://localhost:11434".into(),
            model: "llama3.2:3b".into(),
            timeout_secs: 120,
            keep_alive: "5m".into(),
        };
        assert_eq!(config.model, "llama3.2:3b");
    }

    #[test]
    fn test_ollama_provider_creation() {
        let config = OllamaConfig {
            base_url: "http://localhost:11434".into(),
            model: "test-model".into(),
            timeout_secs: 120,
            keep_alive: "5m".into(),
        };
        let provider = OllamaProvider::new(config);
        let _ = provider;
    }

    // ---------------------------------------------------------------------------
    // RED phase: These tests will FAIL because parse_response currently does NOT
    //            parse tool_calls (returns None).
    // ---------------------------------------------------------------------------

    fn make_ollama_response_with_tool_calls() -> Value {
        serde_json::json!({
            "model": "llama3.2:3b",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "geo",
                            "arguments": {
                                "operation": "geocode",
                                "query": "Silla, Valencia"
                            }
                        }
                    },
                    {
                        "function": {
                            "name": "get_weather",
                            "arguments": {
                                "city": "Madrid",
                                "units": "celsius"
                            }
                        }
                    }
                ]
            },
            "done": true
        })
    }

    fn make_ollama_response_no_tool_calls() -> Value {
        serde_json::json!({
            "model": "llama3.2:3b",
            "created_at": "2024-01-01T00:00:00Z",
            "message": {
                "role": "assistant",
                "content": "Hello! I'm an AI assistant."
            },
            "done": true
        })
    }

    #[test]
    fn test_ollama_parses_tool_calls() {
        let body = make_ollama_response_with_tool_calls();
        let response = OllamaProvider::parse_response(&body).unwrap();

        let tool_calls = response.message.tool_calls.expect(
            "Expected tool_calls to be Some, but got None. BUG: parse_response ignores tool_calls.",
        );

        assert_eq!(tool_calls.len(), 2, "Expected 2 tool calls");

        // First tool call
        assert_eq!(tool_calls[0].id, "ollama-call-0");
        assert_eq!(tool_calls[0].name, "geo");
        assert_eq!(
            tool_calls[0].arguments,
            serde_json::json!({"operation": "geocode", "query": "Silla, Valencia"})
        );

        // Second tool call
        assert_eq!(tool_calls[1].id, "ollama-call-1");
        assert_eq!(tool_calls[1].name, "get_weather");
        assert_eq!(
            tool_calls[1].arguments,
            serde_json::json!({"city": "Madrid", "units": "celsius"})
        );

        // Content should be empty string when LLM returns tool_calls
        assert_eq!(
            response.message.content, "",
            "Expected empty content when LLM returns tool_calls"
        );
    }

    #[test]
    fn test_ollama_no_tool_calls() {
        let body = make_ollama_response_no_tool_calls();
        let response = OllamaProvider::parse_response(&body).unwrap();

        // No tool_calls in this response
        assert!(
            response.message.tool_calls.is_none(),
            "Expected no tool_calls in normal response"
        );

        // Content should contain the response text
        assert_eq!(response.message.content, "Hello! I'm an AI assistant.");
    }

    #[test]
    fn test_ollama_tool_call_arguments_are_objects() {
        let body = make_ollama_response_with_tool_calls();
        let response = OllamaProvider::parse_response(&body).unwrap();

        let tool_calls = response
            .message
            .tool_calls
            .expect("Expected tool_calls to be Some");

        // Ollama returns `function.arguments` as a JSON object directly, NOT a string.
        // It should be used as-is (a `Value`), not parsed from a string.
        assert!(
            tool_calls[0].arguments.is_object(),
            "Expected arguments to be a JSON object, but got: {:?}. BUG: arguments not used as object.",
            tool_calls[0].arguments
        );
        assert_eq!(tool_calls[0].arguments["operation"], "geocode");
        assert_eq!(tool_calls[0].arguments["query"], "Silla, Valencia");
    }

    #[test]
    fn test_ollama_multiple_tool_calls() {
        let body = make_ollama_response_with_tool_calls();
        let response = OllamaProvider::parse_response(&body).unwrap();

        let tool_calls = response
            .message
            .tool_calls
            .expect("Expected tool_calls to be Some");

        assert_eq!(tool_calls.len(), 2, "Expected 2 tool calls");

        // Ollama does not provide an `id` in tool_calls, so they are generated
        // sequentially as "ollama-call-0", "ollama-call-1", ...
        assert_eq!(tool_calls[0].id, "ollama-call-0");
        assert_eq!(tool_calls[1].id, "ollama-call-1");

        assert_eq!(tool_calls[0].name, "geo");
        assert_eq!(tool_calls[1].name, "get_weather");
    }

    // -----------------------------------------------------------------------
    // Contract tests — Ollama ignores reasoning / response_format
    // -----------------------------------------------------------------------

    /// Scenario: Ollama ignora reasoning y response_format
    #[tokio::test]
    async fn test_ollama_ignores_reasoning_and_response_format() {
        use wiremock::matchers::{any, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let captured: std::sync::Arc<std::sync::Mutex<Option<Value>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = captured.clone();
        Mock::given(any())
            .and(method("POST"))
            .respond_with(move |req: &wiremock::Request| {
                *sink.lock().unwrap() = serde_json::from_slice::<Value>(&req.body).ok();
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "message": { "role": "assistant", "content": "ok" },
                    "done": true
                }))
            })
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(OllamaConfig {
            base_url: server.uri(),
            model: "test-model".into(),
            timeout_secs: 5,
            keep_alive: "5m".into(),
        });

        let request = ChatRequest {
            model: "test-model".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "Hi".into(),
                tool_calls: None,
                tool_result: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            reasoning: Some(ReasoningSpec::Off),
            response_format: Some(ResponseFormat::JsonObject),
        };

        let result = provider.chat(request).await;
        assert!(
            result.is_ok(),
            "Ollama must not fail on unknown generation fields: {:?}",
            result.err()
        );

        let body = captured.lock().unwrap().clone().expect("body captured");
        assert!(
            body.get("reasoning").is_none(),
            "Ollama must ignore (not forward) `reasoning`: {body}"
        );
        assert!(
            body.get("response_format").is_none(),
            "Ollama must ignore (not forward) `response_format`: {body}"
        );
    }
}
