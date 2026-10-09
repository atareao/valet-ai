use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use thiserror::Error;
use tokio_stream::Stream;

/// A message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A tool call instruction from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Definition of a tool that can be provided to the LLM.
///
/// # Serialization
///
/// This type serializes as:
/// ```json
/// {"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}
/// ```
/// which is the format expected by OpenAI-compatible APIs (including OpenRouter).
///
/// # Deserialization
///
/// Deserialization supports both the serialized form (with `type`/`function` wrapper) and
/// the flat form (`name`, `description`, `parameters` directly) for backward compatibility.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl serde::Serialize for ToolDef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ToolDef", 2)?;
        s.serialize_field("type", "function")?;
        let function = serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": self.parameters,
        });
        s.serialize_field("function", &function)?;
        s.end()
    }
}

impl<'de> serde::Deserialize<'de> for ToolDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Accept both:
        //   1. Flat format: {"name", "description", "parameters"}
        //   2. Wrapped format: {"type": "function", "function": {"name", ...}}
        let v = serde_json::Value::deserialize(deserializer)?;

        // Try wrapped format first (function key exists)
        if let Some(func) = v.get("function") {
            if let (Some(name), Some(description), Some(parameters)) = (
                func.get("name").and_then(|s| s.as_str()),
                func.get("description").and_then(|s| s.as_str()),
                func.get("parameters"),
            ) {
                return Ok(ToolDef {
                    name: name.to_string(),
                    description: description.to_string(),
                    parameters: parameters.clone(),
                });
            }
        }

        // Fall back to flat format (name key exists at top level)
        if let (Some(name), Some(description), Some(parameters)) = (
            v.get("name").and_then(|s| s.as_str()),
            v.get("description").and_then(|s| s.as_str()),
            v.get("parameters"),
        ) {
            return Ok(ToolDef {
                name: name.to_string(),
                description: description.to_string(),
                parameters: parameters.clone(),
            });
        }

        Err(serde::de::Error::custom(
            "expected ToolDef with name, description, and parameters (flat or wrapped)",
        ))
    }
}

/// Reasoning control for capable OpenRouter models.
///
/// Serializes to the exact OpenRouter shape:
/// - `Off` → `{ "enabled": false }`
/// - `Effort(e)` → `{ "effort": "<level>" }` (level lowercased)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasoningSpec {
    /// Explicitly disable reasoning.
    Off,
    /// Request a specific reasoning effort level.
    Effort(ReasoningEffort),
}

/// A reasoning effort level accepted by OpenRouter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl serde::Serialize for ReasoningSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            ReasoningSpec::Off => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("enabled", &false)?;
                map.end()
            }
            ReasoningSpec::Effort(effort) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("effort", effort)?;
                map.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for ReasoningSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
            // Any `enabled:false` means Off. `enabled:true` has no dedicated
            // representation yet, so it is treated as Off as well.
            let _ = enabled;
            return Ok(ReasoningSpec::Off);
        }
        if let Some(effort) = value.get("effort") {
            let effort: ReasoningEffort =
                serde_json::from_value(effort.clone()).map_err(serde::de::Error::custom)?;
            return Ok(ReasoningSpec::Effort(effort));
        }
        Err(serde::de::Error::custom(
            "expected reasoning object with `enabled` or `effort`",
        ))
    }
}

/// Response format constrain for structured outputs.
///
/// Serializes to `{ "type": "json_object" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    JsonObject,
}

/// Request payload for an LLM chat completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDef>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
    /// Optional reasoning control. `None` means "omit the key".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningSpec>,
    /// Optional structured-output constrain. `None` means "omit the key".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

/// Response from an LLM chat completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub usage: Option<TokenUsage>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cached_tokens: u32,
    pub reasoning_tokens: u32,
    pub cost: f64,
}

/// Events emitted during streaming chat completions.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A text chunk from the stream.
    Chunk(String),
    /// The stream is complete with the final response.
    Done(ChatResponse),
    /// A tool call was requested during streaming.
    ToolCall(ToolCall),
}

/// Errors that can occur during LLM operations.
#[derive(Error, Debug, Clone)]
pub enum LLMError {
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Auth error: {0}")]
    AuthError(String),
    #[error("Model not available: {0}")]
    ModelNotAvailable(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for LLMError {
    fn from(e: reqwest::Error) -> Self {
        LLMError::HttpError(e.to_string())
    }
}

/// Trait that all LLM providers must implement.
///
/// Provides chat completions (both streaming and non-streaming). Embeddings are
/// handled exclusively by [`crate::embeddings::EmbeddingProvider`].
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a non-streaming chat completion request.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError>;

    /// Send a streaming chat completion request.
    ///
    /// Returns a stream of [`StreamEvent`] values.
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, LLMError>> + Send>>, LLMError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage {
            role: "user".into(),
            content: "Hello".into(),
            tool_calls: None,
            tool_result: None,
            tool_call_id: None,
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_tool_call_creation() {
        let tc = ToolCall {
            id: "call_1".into(),
            name: "get_weather".into(),
            arguments: serde_json::json!({"city": "Madrid"}),
        };
        assert_eq!(tc.name, "get_weather");
    }

    #[test]
    fn test_chat_request_defaults() {
        let req = ChatRequest {
            model: "test-model".into(),
            messages: vec![],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            reasoning: None,
            response_format: None,
        };
        assert!(!req.stream);
    }

    #[test]
    fn test_llm_error_display() {
        let err = LLMError::HttpError("connection failed".into());
        assert!(err.to_string().contains("connection failed"));
        let err = LLMError::RateLimited { retry_after: 30 };
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_llm_error_clone() {
        let err = LLMError::Timeout("slow".into());
        let cloned = err.clone();
        assert!(matches!(cloned, LLMError::Timeout(_)));
    }

    // -----------------------------------------------------------------------
    // Contract tests — generation params (reasoning / response_format)
    // -----------------------------------------------------------------------

    /// Scenario: Off se serializa a enabled=false
    #[test]
    fn test_reasoning_spec_off_serializes_to_enabled_false() {
        let value =
            serde_json::to_value(ReasoningSpec::Off).expect("ReasoningSpec::Off should serialize");
        assert_eq!(value, serde_json::json!({ "enabled": false }));
        assert!(
            value.get("effort").is_none(),
            "Off must NOT carry an `effort` key: {value}"
        );
    }

    /// Scenario: Un nivel se serializa a effort en minúsculas
    #[test]
    fn test_reasoning_effort_serializes_to_lowercase_level() {
        let levels = [
            (ReasoningEffort::Minimal, "minimal"),
            (ReasoningEffort::Low, "low"),
            (ReasoningEffort::Medium, "medium"),
            (ReasoningEffort::High, "high"),
            (ReasoningEffort::XHigh, "xhigh"),
            (ReasoningEffort::Max, "max"),
        ];
        for (effort, expected) in levels {
            let value = serde_json::to_value(ReasoningSpec::Effort(effort))
                .expect("ReasoningSpec::Effort should serialize");
            assert_eq!(
                value,
                serde_json::json!({ "effort": expected }),
                "effort level must serialize lowercase"
            );
            assert!(
                value.get("enabled").is_none(),
                "Effort must NOT carry an `enabled` key: {value}"
            );
        }
    }

    /// Scenario: JsonObject se serializa a type=json_object
    #[test]
    fn test_response_format_json_object_serializes() {
        let value = serde_json::to_value(ResponseFormat::JsonObject)
            .expect("ResponseFormat::JsonObject should serialize");
        assert_eq!(value, serde_json::json!({ "type": "json_object" }));
    }

    /// Scenario: ChatRequest por defecto omite los campos
    #[test]
    fn test_chat_request_defaults_reasoning_and_response_format_to_none() {
        let req = ChatRequest {
            model: "test-model".into(),
            messages: vec![],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            reasoning: None,
            response_format: None,
        };
        assert!(
            req.reasoning.is_none(),
            "a ChatRequest built without reasoning must default to None"
        );
        assert!(
            req.response_format.is_none(),
            "a ChatRequest built without response_format must default to None"
        );
    }

    /// `None` must mean "omit the key", never "send an empty object".
    #[test]
    fn test_chat_request_none_fields_are_omitted_when_serialized() {
        let req = ChatRequest {
            model: "test-model".into(),
            messages: vec![],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
            reasoning: None,
            response_format: None,
        };
        let value = serde_json::to_value(&req).expect("ChatRequest should serialize");
        assert!(
            value.get("reasoning").is_none(),
            "None reasoning must be omitted from the serialized request: {value}"
        );
        assert!(
            value.get("response_format").is_none(),
            "None response_format must be omitted from the serialized request: {value}"
        );
    }
}
