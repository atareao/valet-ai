use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "system" => Some(Self::System),
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Value>,
    pub tool_results: Option<Value>,
    pub tokens_count: usize,
    pub collapsed_content: Option<String>,
    pub collapsed_tokens_count: usize,
    pub is_indexed: bool,
    pub summary_ref: Option<String>,
    pub location: Option<String>,
    pub tools_used: Option<String>,
    pub widgets: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Value>,
    pub tool_results: Option<Value>,
    pub tools_used: Option<String>,
}

/// Estimate the number of tokens in a markdown text string using a heuristic.
///
/// This improved heuristic accounts for markdown syntax overhead such as
/// headings, bold, lists, and inline code.
pub fn estimate_markdown_tokens_heuristic(text: &str) -> usize {
    let factor = 1.33;
    let mut words = 0usize;
    let mut md_symbols = 0usize;
    let mut in_word = false;

    for &b in text.as_bytes() {
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                if in_word {
                    words += 1;
                    in_word = false;
                }
            }
            // Símbolos de puntuación y sintaxis Markdown
            b'#' | b'*' | b'`' | b'_' | b'[' | b']' | b'(' | b')' | b'>' | b'-' | b'+' | b'!'
            | b'.' | b',' | b';' | b':' | b'?' | b'"' | b'\'' | b'/' | b'\\' | b'=' | b'~'
            | b'|' => {
                md_symbols += 1;
                if in_word {
                    words += 1;
                    in_word = false;
                }
            }
            _ => {
                in_word = true;
            }
        }
    }

    if in_word {
        words += 1;
    }

    ((words as f32 * factor) as usize) + md_symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_location_field() {
        let msg = Message {
            id: "test-id".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            tool_calls: None,
            tool_results: None,
            tokens_count: 0,
            collapsed_content: None,
            collapsed_tokens_count: 0,
            is_indexed: false,
            summary_ref: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            location: Some("Barcelona".to_string()),
            tools_used: None,
            widgets: None,
        };
        assert_eq!(msg.location, Some("Barcelona".to_string()));
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_empty() {
        assert_eq!(estimate_markdown_tokens_heuristic(""), 0);
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_plain_text() {
        assert_eq!(estimate_markdown_tokens_heuristic("Hola"), 1);
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_markdown_heading() {
        assert_eq!(estimate_markdown_tokens_heuristic("# Hello World"), 3);
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_markdown_bold() {
        assert_eq!(estimate_markdown_tokens_heuristic("Some **bold** text"), 7);
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_markdown_list() {
        let text = "- Item one\n- Item two";
        assert_eq!(estimate_markdown_tokens_heuristic(text), 7);
    }

    #[test]
    fn test_estimate_markdown_tokens_heuristic_inline_code() {
        let text = "Use `let x = 1;`";
        assert_eq!(estimate_markdown_tokens_heuristic(text), 9);
    }
}
