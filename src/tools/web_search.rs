//! Web search tool using the Brave Search API.
//!
//! Reads the API key (`brave_search_api_key`) from the settings database at
//! runtime and calls `GET https://api.search.brave.com/res/v1/web/search`
//! with the user's query. Returns up to 5 results from `web.results`.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

// ---------------------------------------------------------------------------
// Data types — mirrors the Brave Search API response shape
// ---------------------------------------------------------------------------

/// Top-level response from the Brave Search API.
#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    #[serde(default)]
    web: Option<WebResults>,
}

/// Web results container.
#[derive(Debug, Deserialize, Default)]
struct WebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

/// A single web search result.
#[derive(Debug, Deserialize)]
struct BraveResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
}

/// Tool that searches the web via the Brave Search API.
///
/// The API key is read from the `settings` table at runtime
/// (key: `brave_search_api_key`).
pub struct WebSearchTool {
    db: SqlitePool,
    client: reqwest::Client,
}

impl WebSearchTool {
    /// Create a new tool instance.
    ///
    /// The API key is **not** read at construction time; it is fetched from
    /// the settings database on each `execute` call.
    pub fn new(db: SqlitePool) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("valet/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build reqwest Client");
        Self { db, client }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Busca en la web con Brave Search API. Devuelve hasta 5 resultados con título, URL y descripción."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Consulta o palabras clave a buscar en internet"
                }
            },
            "required": ["query"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing query".into()))?;

        if query.trim().is_empty() {
            return Err(ToolError::InvalidArguments("Missing query".into()));
        }

        // Read API key from settings DB first, fallback to Config/ENV
        let api_key =
            match crate::db::repos::settings::SettingsRepo::get(&self.db, "brave_search_api_key")
                .await
            {
                Ok(Some(key)) if !key.is_empty() => key,
                _ => crate::config::Config::from_env()
                    .brave_search_api_key
                    .ok_or_else(|| {
                        ToolError::ExecutionError("Brave Search API key is not configured".into())
                    })?,
            };

        let encoded_query = urlencoding(query);
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}",
            encoded_query
        );

        let resp = self
            .client
            .get(&url)
            .header("X-Subscription-Token", &api_key)
            .send()
            .await
            .map_err(|e| {
                ToolError::ExecutionError(format!("Brave Search request failed: {}", e))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionError(format!(
                "Brave Search API returned {}: {}",
                status.as_u16(),
                body
            )));
        }

        let search_response: BraveSearchResponse = resp.json().await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to parse Brave Search response: {}", e))
        })?;

        // Extract up to 5 results
        let results: Vec<Value> = search_response
            .web
            .unwrap_or_default()
            .results
            .into_iter()
            .take(5)
            .map(|r| {
                serde_json::json!({
                    "title": r.title.unwrap_or_default(),
                    "url": r.url.unwrap_or_default(),
                    "description": r.description.unwrap_or_default(),
                })
            })
            .collect();

        Ok(ToolResult {
            success: true,
            data: serde_json::json!(results),
            message: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// URL-encode a string for use in a query parameter.
fn urlencoding(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    /// Helper: create a WebSearchTool with an in-memory DB (no API key set).
    async fn setup_tool() -> (SqlitePool, WebSearchTool) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true),
            )
            .await
            .expect("Failed to create in-memory DB");

        crate::db::schema::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        let tool = WebSearchTool::new(pool.clone());
        (pool, tool)
    }

    #[tokio::test]
    async fn test_web_search_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        assert_eq!(tool.name(), "web_search");
        assert!(
            tool.description().contains("Brave"),
            "Description should mention Brave: {}",
            tool.description()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_permission() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_parameters() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let params = tool.parameters();
        assert_eq!(params["type"], "object");

        let required = params["required"].as_array().unwrap();
        let req_values: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(req_values, vec!["query"]);

        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("query"), "Should have query");
        assert_eq!(props.len(), 1, "Should only have one property");
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_missing_query() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(_))),
            "Expected InvalidArguments error for missing query"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_empty_query() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let result = tool.execute(serde_json::json!({"query": ""})).await;
        assert!(
            matches!(result, Err(ToolError::InvalidArguments(ref msg)) if msg.contains("Missing query")),
            "Expected InvalidArguments with 'Missing query', got {:?}",
            result
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_missing_api_key() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;
        let result = tool
            .execute(serde_json::json!({"query": "test query"}))
            .await;
        assert!(
            matches!(result, Err(ToolError::ExecutionError(_))),
            "Expected ExecutionError when API key is missing"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: web_search definitions must be in Spanish.
    // -----------------------------------------------------------------------

    /// Scenario: Las tools en inglés se traducen al español
    #[tokio::test]
    async fn test_web_search_description_is_in_spanish() -> Result<(), Box<dyn std::error::Error>> {
        let (_, tool) = setup_tool().await;

        let desc = tool.description();
        assert!(
            !desc.contains("Search the web"),
            "tool description must be in Spanish, got: {desc}"
        );
        assert!(
            desc.contains("Busca"),
            "tool description must be in Spanish (must contain 'Busca'), got: {desc}"
        );

        let params = tool.parameters();
        let query_desc = params["properties"]["query"]["description"]
            .as_str()
            .unwrap_or("");
        assert!(
            !query_desc.contains("Search query"),
            "query description must be in Spanish, got: {query_desc}"
        );
        Ok(())
    }
}
