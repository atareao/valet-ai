use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use serde_json::Value;

use crate::llm::provider::ToolDef;
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

/// Registry of available tools.
///
/// Tools are stored as `Arc<dyn Tool>` so that the registry can be cheaply
/// cloned (shared via Arc) while still allowing registration at init time.
#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<HashMap<String, Arc<dyn Tool>>>,
    /// Names of tools that are disabled in the database. Disabled tools are
    /// neither advertised to the model nor executable.
    disabled: Arc<RwLock<HashSet<String>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(HashMap::new()),
            disabled: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        let arc_tool: Arc<dyn Tool> = Arc::from(tool);
        // Arc::make_mut requires Clone on the inner HashMap values, which
        // Arc<dyn Tool> satisfies. This will clone the HashMap only when
        // there are multiple references (i.e. after clone).
        let tools = Arc::make_mut(&mut self.tools);
        tools.insert(name, arc_tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Replace the set of disabled tool names.
    pub fn set_disabled(&self, names: impl IntoIterator<Item = String>) {
        let mut disabled = self
            .disabled
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        disabled.clear();
        disabled.extend(names);
    }

    /// Returns `true` when the tool is not present in the disabled set.
    pub fn is_enabled(&self, name: &str) -> bool {
        let disabled = self
            .disabled
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !disabled.contains(name)
    }

    pub fn definitions(&self) -> Vec<ToolDef> {
        self.tools
            .values()
            .filter(|t| self.is_enabled(t.name()))
            .map(|t| ToolDef {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    /// Definiciones de un subconjunto de herramientas, incluyendo **únicamente**
    /// las que estén habilitadas y en un **orden determinista por nombre**.
    /// Los nombres desconocidos se ignoran sin error.
    ///
    /// `definitions_for` acota lo que se **anuncia** al modelo; no es una
    /// frontera de ejecución (`execute` sigue resolviendo cualquier herramienta
    /// habilitada).
    pub fn definitions_for(&self, names: &[&str]) -> Vec<ToolDef> {
        let mut defs: Vec<ToolDef> = self
            .definitions()
            .into_iter()
            .filter(|def| names.contains(&def.name.as_str()))
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<ToolResult, ToolError> {
        if !self.is_enabled(name) {
            return Err(ToolError::PermissionDenied(format!(
                "tool '{name}' is disabled"
            )));
        }
        match self.tools.get(name) {
            Some(tool) => tool.execute(args).await,
            None => Err(ToolError::NotFound(name.to_string())),
        }
    }

    pub fn permission(&self, name: &str, args: &Value) -> Option<Permission> {
        self.tools.get(name).map(|t| t.permission(args))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct DummyTool;

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &'static str {
            "dummy"
        }

        fn description(&self) -> &'static str {
            "Dummy tool"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({"ok": true}),
                message: None,
            })
        }
    }

    #[test]
    fn test_empty_registry() {
        let reg = ToolRegistry::new();
        assert!(reg.get("nonexistent").is_none());
        assert!(reg.definitions().is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));
        assert!(reg.get("dummy").is_some());
    }

    #[test]
    fn test_definitions_returns_all() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));
        let defs = reg.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "dummy");
    }

    #[tokio::test]
    async fn test_execute_registered_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));
        let result = reg.execute("dummy", serde_json::json!({})).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_execute_unknown_tool_returns_error() -> Result<(), Box<dyn std::error::Error>> {
        let reg = ToolRegistry::new();
        let result = reg.execute("unknown", serde_json::json!({})).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_disabled_tool_is_hidden_and_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(DummyTool));

        reg.set_disabled(["dummy".to_string()]);
        assert!(
            reg.definitions().is_empty(),
            "disabled tool must not be advertised"
        );
        assert!(!reg.is_enabled("dummy"));
        let result = reg.execute("dummy", serde_json::json!({})).await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));

        // Re-enabling restores both advertisement and execution.
        reg.set_disabled(Vec::new());
        assert!(reg.is_enabled("dummy"));
        assert_eq!(reg.definitions().len(), 1);
        let result = reg.execute("dummy", serde_json::json!({})).await.unwrap();
        assert!(result.success);
        Ok(())
    }

    #[test]
    fn test_permission_of_unknown_tool() {
        let reg = ToolRegistry::new();
        assert!(reg.permission("unknown", &serde_json::json!({})).is_none());
    }

    // -----------------------------------------------------------------------
    // `definitions_for` (skill-router, tarea 5.1) y baseline de tokens (1.1).
    // -----------------------------------------------------------------------

    /// Tool de prueba con un nombre arbitrario, para poblar el registry con
    /// varios nombres distintos.
    struct NamedTool(&'static str);

    #[async_trait]
    impl Tool for NamedTool {
        fn name(&self) -> &'static str {
            self.0
        }

        fn description(&self) -> &'static str {
            "Named test tool"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }

        fn permission(&self, _args: &Value) -> Permission {
            Permission::NoConfirm
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult, ToolError> {
            Ok(ToolResult {
                success: true,
                data: serde_json::json!({}),
                message: None,
            })
        }
    }

    #[test]
    fn definitions_for_omits_disabled_and_unknown() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(NamedTool("tasks")));
        reg.register(Box::new(NamedTool("weather")));
        reg.set_disabled(["weather".to_string()]);

        let names: Vec<String> = reg
            .definitions_for(&["weather", "tasks", "inexistente"])
            .iter()
            .map(|d| d.name.clone())
            .collect();

        assert_eq!(
            names,
            vec!["tasks"],
            "only enabled, known tools must be returned"
        );
    }

    #[test]
    fn definitions_for_order_is_stable_and_alphabetical() {
        let mut reg = ToolRegistry::new();
        for name in ["zulu", "alpha", "mike"] {
            reg.register(Box::new(NamedTool(name)));
        }

        let first: Vec<String> = reg
            .definitions_for(&["zulu", "alpha", "mike"])
            .iter()
            .map(|d| d.name.clone())
            .collect();
        let second: Vec<String> = reg
            .definitions_for(&["mike", "zulu", "alpha"])
            .iter()
            .map(|d| d.name.clone())
            .collect();

        assert_eq!(first, second, "the order must be stable between calls");
        assert_eq!(
            first,
            vec!["alpha", "mike", "zulu"],
            "the order must be alphabetical by name"
        );
    }

    /// Baseline 1.1: registra el tamaño en tokens del bloque de herramientas
    /// de producción. Es la referencia del «antes» (~3,5-4 k tokens) y debe
    /// quedar **en verde**.
    #[tokio::test]
    async fn baseline_production_tools_block_token_size() {
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

        let registry = crate::build_tool_registry(&pool);
        let defs = registry.definitions();
        let json = serde_json::to_string(&defs).expect("definitions must serialize");
        let tokens = crate::token_estimate::estimate_json_tokens(&json);

        println!(
            "[baseline 1.1] tools block: {} definitions, {} bytes, ~{} tokens (documented floor: > 2000)",
            defs.len(),
            json.len(),
            tokens
        );

        assert!(
            tokens > 2000,
            "production tools block estimated at {tokens} tokens, expected > 2000 \
             (premise of skill-router: ~3.5-4k)"
        );
    }
}
