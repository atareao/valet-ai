use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::tools::permission::Permission;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<sqlx::Error> for ToolError {
    fn from(e: sqlx::Error) -> Self {
        ToolError::ExecutionError(e.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: Value,
    pub message: Option<String>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;
    fn permission(&self, args: &Value) -> Permission;
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTool;

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &'static str {
            "test_tool"
        }

        fn description(&self) -> &'static str {
            "A test tool"
        }

        fn parameters(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                }
            })
        }

        fn permission(&self, _args: &Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
            if args.get("input").is_none() {
                return Err(ToolError::InvalidArguments("missing input".into()));
            }
            Ok(ToolResult {
                success: true,
                data: args,
                message: None,
            })
        }
    }

    #[tokio::test]
    async fn test_tool_name_and_description() {
        let tool = TestTool;
        assert_eq!(tool.name(), "test_tool");
        assert_eq!(tool.description(), "A test tool");
    }

    #[tokio::test]
    async fn test_tool_execute_success() {
        let tool = TestTool;
        let result = tool
            .execute(serde_json::json!({"input": "hello"}))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["input"], "hello");
    }

    #[tokio::test]
    async fn test_tool_execute_invalid_args() {
        let tool = TestTool;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_tool_returns_permission() {
        let tool = TestTool;
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
    }

    #[tokio::test]
    async fn test_tool_parameters_is_valid_json_schema() {
        let tool = TestTool;
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params.get("properties").is_some());
    }
}
