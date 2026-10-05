use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::repos::notes::{Note, NotesRepo};
use crate::tools::permission::Permission;
use crate::tools::r#trait::{Tool, ToolError, ToolResult};

pub struct NotesTool {
    db: SqlitePool,
}

impl NotesTool {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    async fn create_note(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("idea")
            .to_string();
        let tags = args
            .get("tags")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if content.is_empty() {
            return Err(ToolError::InvalidArguments("content is required".into()));
        }

        let now = Utc::now().to_rfc3339();
        let note = Note {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id,
            content,
            category,
            tags,
            created_at: now.clone(),
            updated_at: now,
        };

        NotesRepo::create(&self.db, &note).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&note).unwrap_or_default(),
            message: Some("Note created".into()),
        })
    }

    async fn list_notes(&self, args: Value) -> Result<ToolResult, ToolError> {
        let profile_id = args
            .get("profile_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let category = args.get("category").and_then(|v| v.as_str());

        let notes = NotesRepo::list(&self.db, profile_id, category).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::to_value(&notes).unwrap_or_default(),
            message: Some(format!("Found {} notes", notes.len())),
        })
    }

    async fn delete_note(&self, args: Value) -> Result<ToolResult, ToolError> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if id.is_empty() {
            return Err(ToolError::InvalidArguments("id is required".into()));
        }

        NotesRepo::delete(&self.db, &id).await?;

        Ok(ToolResult {
            success: true,
            data: serde_json::json!({"id": id}),
            message: Some("Note deleted".into()),
        })
    }
}

#[async_trait]
impl Tool for NotesTool {
    fn name(&self) -> &'static str {
        "notes"
    }

    fn description(&self) -> &'static str {
        "Gestión de notas personales con categorías (idea, journal, fact)"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["create_note", "list_notes", "delete_note"],
                    "description": "Acción a realizar. `content` es obligatorio para `create_note`; `id` para `delete_note`."
                },
                "profile_id": { "type": "string", "description": "ID del perfil (por defecto 'default')" },
                "content": { "type": "string", "description": "Contenido de la nota" },
                "category": {
                    "type": "string",
                    "enum": ["idea", "journal", "fact"],
                    "description": "Categoría de la nota (por defecto 'idea')"
                },
                "tags": { "type": "string", "description": "Etiquetas separadas por comas" },
                "id": { "type": "string", "description": "ID de la nota" }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self, _args: &Value) -> Permission {
        Permission::NoConfirm
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("");

        match operation {
            "create_note" => self.create_note(args).await,
            "list_notes" => self.list_notes(args).await,
            "delete_note" => self.delete_note(args).await,
            _ => Err(ToolError::InvalidArguments(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_db() -> Result<SqlitePool, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        run_migrations(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO profiles (id, name, preferences) VALUES ('profile-1', 'Test', '{}')",
        )
        .execute(&pool)
        .await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn test_notes_name_and_description() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        assert_eq!(tool.name(), "notes");
        assert!(tool.description().contains("categorías"));
        assert!(
            !tool.description().contains("todo"),
            "the tool description must not mention the `todo` category, got: {}",
            tool.description()
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_notes_permission() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        assert_eq!(
            tool.permission(&serde_json::json!({})),
            Permission::NoConfirm
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_idea() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let args = serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Una idea genial",
            "category": "idea",
            "tags": "creatividad,proyecto"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["content"], "Una idea genial");
        assert_eq!(result.data["category"], "idea");
        assert_eq!(result.data["tags"], "creatividad,proyecto");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_journal() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let args = serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Hoy fue un gran día",
            "category": "journal"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["category"], "journal");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_fact() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let args = serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "La velocidad de la luz es 299.792.458 m/s",
            "category": "fact"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["category"], "fact");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_todo() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let args = serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Comprar regalos de Navidad",
            "category": "todo"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["category"], "todo");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_default_category() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let args = serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Nota sin categoría explícita"
        });

        let result = tool.execute(args).await.unwrap();
        assert!(result.success);
        assert_eq!(result.data["category"], "idea");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_note_missing_content() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let err = tool
            .execute(serde_json::json!({
                "operation": "create_note",
                "category": "idea"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));
        Ok(())
    }

    #[tokio::test]
    async fn test_list_notes() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        // Create notes in different categories
        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Idea 1",
            "category": "idea"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Journal entry",
            "category": "journal"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Fact 1",
            "category": "fact"
        }))
        .await
        .unwrap();

        // List all
        let result = tool
            .execute(serde_json::json!({
                "operation": "list_notes",
                "profile_id": "profile-1"
            }))
            .await
            .unwrap();
        let all: Vec<Note> = serde_json::from_value(result.data).unwrap();
        assert_eq!(all.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_notes_by_category() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Idea A",
            "category": "idea"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Idea B",
            "category": "idea"
        }))
        .await
        .unwrap();

        tool.execute(serde_json::json!({
            "operation": "create_note",
            "profile_id": "profile-1",
            "content": "Todo item",
            "category": "todo"
        }))
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({
                "operation": "list_notes",
                "profile_id": "profile-1",
                "category": "idea"
            }))
            .await
            .unwrap();
        let ideas: Vec<Note> = serde_json::from_value(result.data).unwrap();
        assert_eq!(ideas.len(), 2);

        let result = tool
            .execute(serde_json::json!({
                "operation": "list_notes",
                "profile_id": "profile-1",
                "category": "todo"
            }))
            .await
            .unwrap();
        let todos: Vec<Note> = serde_json::from_value(result.data).unwrap();
        assert_eq!(todos.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_note() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let created = tool
            .execute(serde_json::json!({
                "operation": "create_note",
                "profile_id": "profile-1",
                "content": "Para borrar",
                "category": "todo"
            }))
            .await
            .unwrap();
        let id = created.data["id"].as_str().unwrap().to_string();

        let result = tool
            .execute(serde_json::json!({
                "operation": "delete_note",
                "id": id
            }))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.data["id"], id);

        // Verify it's gone
        let list = tool
            .execute(serde_json::json!({
                "operation": "list_notes",
                "profile_id": "profile-1"
            }))
            .await
            .unwrap();
        let notes: Vec<Note> = serde_json::from_value(list.data).unwrap();
        assert!(notes.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_note_missing_id() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let err = tool
            .execute(serde_json::json!({
                "operation": "delete_note"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));
        Ok(())
    }

    #[tokio::test]
    async fn test_unknown_operation() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);

        let err = tool
            .execute(serde_json::json!({
                "operation": "nope"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments(_)));
        Ok(())
    }

    #[tokio::test]
    async fn test_parameters_returns_valid_json_schema() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        let params = tool.parameters();
        assert_eq!(params["type"], "object");
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: `todo` leaves the category enum and the
    // `operation` description documents the required fields per action.
    // -----------------------------------------------------------------------

    /// Scenario: La categoría todo no se ofrece
    #[tokio::test]
    async fn test_notes_category_enum_excludes_todo() -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        let params = tool.parameters();

        let categories = params["properties"]["category"]["enum"]
            .as_array()
            .expect("category must expose an enum");
        let values: Vec<&str> = categories.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(
            values,
            vec!["idea", "journal", "fact"],
            "the category enum must be exactly [idea, journal, fact] (no `todo`), got: {values:?}"
        );
        Ok(())
    }

    /// Scenario: La operación documenta los obligatorios
    #[tokio::test]
    async fn test_notes_operation_description_documents_required_fields(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        let params = tool.parameters();

        let desc = params["properties"]["operation"]["description"]
            .as_str()
            .unwrap_or("");
        assert!(
            !desc.is_empty(),
            "the `operation` parameter must carry a description"
        );
        assert!(
            desc.contains("`content`"),
            "the `operation` description must mention `content` for create_note, got: {desc}"
        );
        assert!(
            desc.contains("`id`"),
            "the `operation` description must mention `id` for delete_note, got: {desc}"
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RED — improve-tool-schemas: todas las descripciones de parámetros deben
    // estar en español (requisito transversal de `tools/registry`).
    // -----------------------------------------------------------------------

    /// Scenario: Las definiciones de herramientas están en español y documentan
    /// los obligatorios; aquí se cubre `notes`.
    #[tokio::test]
    async fn test_notes_parameter_descriptions_are_in_spanish(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = setup_db().await?;
        let tool = NotesTool::new(db);
        let params = tool.parameters();
        let properties = params["properties"]
            .as_object()
            .expect("`properties` must be an object");

        // Toda propiedad debe llevar una descripción no vacía.
        for (name, schema) in properties {
            let desc = schema["description"].as_str().unwrap_or("");
            assert!(
                !desc.is_empty(),
                "la descripción de `{name}` no debe estar vacía"
            );
        }

        // Las descripciones de `content`, `category` e `id` están en español.
        let content = properties["content"]["description"].as_str().unwrap_or("");
        assert!(
            content.contains("Contenido"),
            "`content` debe describirse en español, got: {content}"
        );
        let category = properties["category"]["description"].as_str().unwrap_or("");
        assert!(
            category.contains("Categoría"),
            "`category` debe describirse en español, got: {category}"
        );
        let id = properties["id"]["description"].as_str().unwrap_or("");
        assert!(
            id.contains("ID"),
            "`id` debe describirse en español usando «ID», got: {id}"
        );

        // Ninguna descripción conserva términos en inglés.
        for (name, schema) in properties {
            let desc = schema["description"].as_str().unwrap_or("");
            for term in [
                "Profile ID",
                "Note content",
                "Note category",
                "Comma-separated",
                "Note ID",
            ] {
                assert!(
                    !desc.contains(term),
                    "la descripción de `{name}` no debe contener «{term}», got: {desc}"
                );
            }
        }
        Ok(())
    }
}
