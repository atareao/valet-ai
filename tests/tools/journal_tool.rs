//! Tool tests for the `journal-entries` change:
//! `valet::tools::journal::JournalTool` over the `journal_entries` table, its
//! per-operation permissions and its place in the production registry.
//!
//! The tool is a thin, strict façade over `JournalRepo`: it validates the date
//! and the mood **before** touching the database, delegates every write to the
//! repository and answers a day without an entry without failing.

use chrono::Utc;
use chrono_tz::Tz;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

use valet::build_tool_registry;
use valet::models::journal::JournalEntry;
use valet::tools::journal::JournalTool;
use valet::tools::permission::Permission;
use valet::tools::r#trait::{Tool, ToolError};

const PROFILE: &str = "default";
const DAY: &str = "2026-10-10";

/// The timezone the tests pin as the user's: the day of the journal is the
/// **local** day, so «hoy» must be resolved in it, not in UTC.
const TIMEZONE: &str = "Europe/Madrid";

async fn setup() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();
    valet::db::schema::run_migrations(&pool).await.unwrap();
    sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES (?1, 'Test', '{}')")
        .bind(PROFILE)
        .execute(&pool)
        .await
        .unwrap();
    // The user's timezone, as `current_time` reads it.
    valet::db::repos::settings::SettingsRepo::set(&pool, "timezone", TIMEZONE)
        .await
        .unwrap();
    pool
}

fn tool(pool: SqlitePool) -> JournalTool {
    JournalTool::new(pool)
}

/// «Hoy» in the user's timezone: the day the journal would call today.
fn today() -> String {
    let tz = Tz::from_str(TIMEZONE).expect("Europe/Madrid is a valid timezone");
    Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string()
}

/// How many rows the profile holds for that day.
async fn rows_for(pool: &SqlitePool, profile_id: &str, date: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM journal_entries WHERE profile_id = ?1 AND date = ?2")
        .bind(profile_id)
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The entry of that day, read straight from the table (not through the tool),
/// so the tests can prove what was really stored.
async fn stored(pool: &SqlitePool, date: &str) -> Option<JournalEntry> {
    valet::db::repos::journal::JournalRepo::get(pool, PROFILE, date)
        .await
        .unwrap()
}

// ── 3.1 `get_entry` ────────────────────────────────────────────────────────

/// Scenario: `get_entry` con una fecha devuelve esa entrada.
#[tokio::test]
async fn test_journal_get_entry_with_a_date_returns_that_entry(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    tool.execute(json!({
        "operation": "write_entry",
        "date": DAY,
        "content": "un día tranquilo",
        "mood": "good"
    }))
    .await?;

    let result = tool
        .execute(json!({"operation": "get_entry", "date": DAY}))
        .await?;

    assert!(result.success, "reading an existing day must succeed");
    assert_eq!(result.data["date"], DAY);
    assert_eq!(result.data["content"], "un día tranquilo");
    assert_eq!(result.data["mood"], "good");
    Ok(())
}

/// Scenario: `get_entry` sin fecha devuelve la de hoy.
#[tokio::test]
async fn test_journal_get_entry_without_a_date_returns_today(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    tool.execute(json!({
        "operation": "write_entry",
        "date": today(),
        "content": "lo de hoy"
    }))
    .await?;

    let result = tool.execute(json!({"operation": "get_entry"})).await?;
    assert!(
        result.success,
        "reading today without a date must succeed, got {result:?}"
    );
    assert_eq!(
        result.data["content"], "lo de hoy",
        "an omitted date must resolve to today in the user's timezone, got {result:?}"
    );
    assert_eq!(result.data["date"], today());
    Ok(())
}

/// Scenario: `get_entry` de un día sin entrada no falla.
#[tokio::test]
async fn test_journal_get_entry_of_a_day_without_an_entry_does_not_fail(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool);

    let result = tool
        .execute(json!({"operation": "get_entry", "date": DAY}))
        .await?;

    assert!(result.success, "a day without an entry is not an error");
    assert!(result.data.is_null(), "there is no entry to return");
    let message = result.message.unwrap_or_default();
    assert!(
        message.to_lowercase().contains("no existe"),
        "the answer must say the day has no entry, got: {message}"
    );
    Ok(())
}

// ── 3.2 `write_entry` ──────────────────────────────────────────────────────

/// Scenario: `write_entry` crea la entrada del día.
#[tokio::test]
async fn test_journal_write_entry_creates_the_day_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = setup().await;
    let tool = tool(pool.clone());

    let result = tool
        .execute(json!({
            "operation": "write_entry",
            "date": DAY,
            "content": "he salido a correr",
            "mood": "good",
            "achievements": ["12 km"],
            "struggles": ["dormí poco"],
            "tags": ["deporte"]
        }))
        .await?;

    assert!(result.success);
    assert_eq!(rows_for(&pool, PROFILE, DAY).await, 1, "one row must exist");

    let saved = stored(&pool, DAY).await.expect("the entry must be stored");
    assert_eq!(saved.profile_id, PROFILE);
    assert_eq!(saved.date, DAY);
    assert_eq!(saved.content, "he salido a correr");
    assert_eq!(saved.mood, Some(valet::models::journal::Mood::Good));
    assert_eq!(saved.achievements, vec!["12 km".to_string()]);
    assert_eq!(saved.struggles, vec!["dormí poco".to_string()]);
    assert_eq!(saved.tags, vec!["deporte".to_string()]);
    Ok(())
}

/// Scenario: `write_entry` sobrescribe la del mismo día.
#[tokio::test]
async fn test_journal_write_entry_overwrites_the_same_day() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = setup().await;
    let tool = tool(pool.clone());

    for content in ["un relato", "otro relato"] {
        tool.execute(json!({
            "operation": "write_entry",
            "date": DAY,
            "content": content
        }))
        .await?;
    }

    assert_eq!(
        rows_for(&pool, PROFILE, DAY).await,
        1,
        "the second write of the day must replace the row, not append another"
    );
    let saved = stored(&pool, DAY).await.expect("the entry must be stored");
    assert_eq!(saved.content, "otro relato");
    Ok(())
}

/// Scenario: una entrada sin ánimo se guarda igual.
#[tokio::test]
async fn test_journal_write_entry_without_a_mood_is_stored_with_none(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    let result = tool
        .execute(json!({
            "operation": "write_entry",
            "date": DAY,
            "content": "sin ánimo"
        }))
        .await?;

    assert!(result.success);
    assert_eq!(result.data["mood"], serde_json::Value::Null);
    let saved = stored(&pool, DAY).await.expect("the entry must be stored");
    assert_eq!(saved.mood, None, "an absent mood must stay absent");
    assert_eq!(saved.content, "sin ánimo");
    Ok(())
}

// ── 3.3 `update_entry` y `list_entries` ────────────────────────────────────

/// Scenario: `update_entry` solo toca lo que le llega.
#[tokio::test]
async fn test_journal_update_entry_only_touches_what_it_receives(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    tool.execute(json!({
        "operation": "write_entry",
        "date": DAY,
        "content": "el relato de siempre",
        "mood": "bad",
        "tags": ["trabajo", "familia"]
    }))
    .await?;

    let result = tool
        .execute(json!({
            "operation": "update_entry",
            "date": DAY,
            "mood": "good"
        }))
        .await?;

    assert!(result.success);
    let saved = stored(&pool, DAY)
        .await
        .expect("the entry must still exist");
    assert_eq!(saved.mood, Some(valet::models::journal::Mood::Good));
    assert_eq!(
        saved.content, "el relato de siempre",
        "a field that was not sent must be left intact"
    );
    assert_eq!(
        saved.tags,
        vec!["trabajo".to_string(), "familia".to_string()],
        "a field that was not sent must be left intact"
    );
    Ok(())
}

/// Scenario: `list_entries` devuelve el rango ordenado.
#[tokio::test]
async fn test_journal_list_entries_returns_the_range_most_recent_first(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    for (date, content) in [
        ("2026-10-08", "antes del rango"),
        ("2026-10-09", "el primero del rango"),
        ("2026-10-10", "el último del rango"),
        ("2026-10-11", "después del rango"),
    ] {
        tool.execute(json!({
            "operation": "write_entry",
            "date": date,
            "content": content
        }))
        .await?;
    }

    let result = tool
        .execute(json!({
            "operation": "list_entries",
            "from": "2026-10-09",
            "to": "2026-10-10"
        }))
        .await?;

    assert!(result.success);
    let listed: Vec<JournalEntry> = serde_json::from_value(result.data.clone())?;
    let dates: Vec<&str> = listed.iter().map(|entry| entry.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2026-10-10", "2026-10-09"],
        "both ends of the range are inclusive and the newest comes first, got {result:?}"
    );
    Ok(())
}

/// Scenario: `list_entries` respeta el tope quedándose con lo más reciente.
#[tokio::test]
async fn test_journal_list_entries_honours_the_limit() -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    for date in ["2026-10-08", "2026-10-09", "2026-10-10"] {
        tool.execute(json!({
            "operation": "write_entry",
            "date": date,
            "content": "relato"
        }))
        .await?;
    }

    let result = tool
        .execute(json!({
            "operation": "list_entries",
            "from": "2026-10-08",
            "to": "2026-10-10",
            "limit": 2
        }))
        .await?;

    let listed: Vec<JournalEntry> = serde_json::from_value(result.data.clone())?;
    let dates: Vec<&str> = listed.iter().map(|entry| entry.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2026-10-10", "2026-10-09"],
        "at most `limit` entries, and the newest ones, got {result:?}"
    );
    Ok(())
}

// ── 3.4 `delete_entry` ─────────────────────────────────────────────────────

/// Scenario: `delete_entry` borra la del día.
#[tokio::test]
async fn test_journal_delete_entry_removes_the_day() -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    tool.execute(json!({
        "operation": "write_entry",
        "date": DAY,
        "content": "para borrar"
    }))
    .await?;

    let result = tool
        .execute(json!({"operation": "delete_entry", "date": DAY}))
        .await?;

    assert!(result.success);
    assert_eq!(
        rows_for(&pool, PROFILE, DAY).await,
        0,
        "the row must be gone from `journal_entries`"
    );
    assert_eq!(result.data["deleted"], true);
    Ok(())
}

/// Scenario: `delete_entry` de lo que no existe no falla.
#[tokio::test]
async fn test_journal_delete_entry_of_an_absent_day_does_not_fail(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool);

    let result = tool
        .execute(json!({"operation": "delete_entry", "date": DAY}))
        .await?;

    assert!(
        result.success,
        "deleting what was never there is not an error"
    );
    assert_eq!(result.data["deleted"], false);
    let message = result.message.unwrap_or_default();
    assert!(
        message.to_lowercase().contains("no existe"),
        "the answer must say the day has no entry, got: {message}"
    );
    Ok(())
}

// ── 3.5 validación ─────────────────────────────────────────────────────────

/// Scenario: una fecha mal formada es un argumento inválido y **no consulta la
/// base de datos**.
///
/// La prueba cierra el pool antes de ejecutar: cualquier consulta fallaría con
/// un error de ejecución, así que recibir `InvalidArguments` demuestra que la
/// validación ocurrió antes de tocar la base.
#[tokio::test]
async fn test_journal_malformed_date_is_an_invalid_argument_before_any_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        json!({"operation": "get_entry", "date": "18-10-2026"}),
        json!({"operation": "get_entry", "date": "ayer"}),
        json!({"operation": "write_entry", "date": "2026-1-1", "content": "x"}),
        json!({"operation": "update_entry", "date": "2026/10/10", "mood": "good"}),
        json!({"operation": "delete_entry", "date": ""}),
        json!({"operation": "list_entries", "from": "sin fecha", "to": "2026-10-10"}),
        json!({"operation": "list_entries", "from": "2026-10-10", "to": "2026-13-45"}),
    ];

    for args in cases {
        let pool = setup().await;
        pool.close().await;
        let err = tool(pool).execute(args.clone()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "{args} must be rejected as an invalid argument, not reach the database; got {err:?}"
        );
    }
    Ok(())
}

/// Scenario: un ánimo fuera del conjunto es un argumento inválido y la entrada
/// no se modifica.
#[tokio::test]
async fn test_journal_mood_outside_the_set_is_an_invalid_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    tool.execute(json!({
        "operation": "write_entry",
        "date": DAY,
        "content": "el relato",
        "mood": "good",
        "tags": ["trabajo"]
    }))
    .await?;

    for args in [
        json!({"operation": "update_entry", "date": DAY, "mood": "genial"}),
        json!({"operation": "update_entry", "date": DAY, "mood": "Great"}),
        json!({"operation": "update_entry", "date": DAY, "mood": "muy bueno", "content": "otro"}),
    ] {
        let err = tool.execute(args.clone()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "{args} must be rejected as an invalid argument; got {err:?}"
        );
    }

    let saved = stored(&pool, DAY)
        .await
        .expect("the entry must still exist");
    assert_eq!(
        saved.mood,
        Some(valet::models::journal::Mood::Good),
        "a rejected mood must not change the stored one"
    );
    assert_eq!(
        saved.content, "el relato",
        "a rejected call must not change anything else either"
    );
    assert_eq!(saved.tags, vec!["trabajo".to_string()]);

    // Un día nuevo con un ánimo inválido no llega a crear fila.
    let err = tool
        .execute(json!({
            "operation": "write_entry",
            "date": "2026-10-12",
            "content": "no debe existir",
            "mood": "genial"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
    assert_eq!(
        rows_for(&pool, PROFILE, "2026-10-12").await,
        0,
        "a rejected write must not touch the database"
    );
    Ok(())
}

// ── 3.6 permisos ───────────────────────────────────────────────────────────

/// Scenario: leer y escribir no piden confirmación.
#[tokio::test]
async fn test_journal_read_and_write_operations_do_not_ask_for_confirmation(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let registry = build_tool_registry(&pool);

    for operation in ["get_entry", "write_entry", "update_entry", "list_entries"] {
        let permission = registry
            .permission("journal", &json!({"operation": operation}))
            .expect("`journal` must be in the production registry");
        assert_eq!(
            permission,
            Permission::NoConfirm,
            "`{operation}` does not need confirmation"
        );
    }
    Ok(())
}

/// Scenario: borrar exige aprobación explícita.
#[tokio::test]
async fn test_journal_delete_entry_requires_explicit_approval(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let registry = build_tool_registry(&pool);

    let permission = registry
        .permission(
            "journal",
            &json!({"operation": "delete_entry", "date": DAY}),
        )
        .expect("`journal` must be in the production registry");
    assert_eq!(
        permission,
        Permission::ExplicitApproval,
        "deleting the journal is destructive and needs the user's approval"
    );
    Ok(())
}

// ── 3.7 definición ─────────────────────────────────────────────────────────

/// La definición de `journal` tal como viaja al modelo.
async fn journal_definition(pool: &SqlitePool) -> valet::llm::provider::ToolDef {
    build_tool_registry(pool)
        .definitions()
        .into_iter()
        .find(|def| def.name == "journal")
        .expect("`journal` must be part of the definitions")
}

/// Scenario: la herramienta figura en las definiciones.
#[tokio::test]
async fn test_journal_is_in_the_definitions_with_its_five_operations(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let definition = journal_definition(&pool).await;

    assert_eq!(
        definition.parameters["type"], "object",
        "the parameters must be an object schema"
    );

    let operations = definition.parameters["properties"]["operation"]["enum"]
        .as_array()
        .expect("`operation` must expose an enum")
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect::<Vec<String>>();
    assert_eq!(
        operations,
        vec![
            "get_entry",
            "write_entry",
            "update_entry",
            "delete_entry",
            "list_entries"
        ],
        "the tool must dispatch exactly those five operations"
    );

    assert_eq!(
        definition.parameters["required"],
        json!(["operation"]),
        "every action carries its own required fields, documented in `operation`"
    );
    Ok(())
}

/// Scenario: la descripción de `operation` enumera los obligatorios.
#[tokio::test]
async fn test_journal_operation_description_documents_the_required_fields(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let definition = journal_definition(&pool).await;

    let description = definition.parameters["properties"]["operation"]["description"]
        .as_str()
        .unwrap_or_default();
    assert!(
        !description.is_empty(),
        "the `operation` parameter must carry a description"
    );
    assert!(
        description.contains("`content`"),
        "the description must mention that `content` is required for write_entry, got: {description}"
    );
    assert!(
        description.contains("`date`"),
        "the description must mention that `date` is required, got: {description}"
    );
    for operation in ["write_entry", "update_entry", "delete_entry"] {
        assert!(
            description.contains(operation),
            "the description must say which operations require `date`, starting with `{operation}`, got: {description}"
        );
    }
    Ok(())
}

/// Scenario: las descripciones están en español.
#[tokio::test]
async fn test_journal_descriptions_are_in_spanish() -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let definition = journal_definition(&pool).await;

    assert!(
        definition.description.contains("bitácora"),
        "the tool must describe itself in Spanish, got: {}",
        definition.description
    );

    let properties = definition.parameters["properties"]
        .as_object()
        .expect("`properties` must be an object");
    for (name, schema) in properties {
        let description = schema["description"].as_str().unwrap_or_default();
        assert!(
            !description.is_empty(),
            "la descripción de `{name}` no debe estar vacía"
        );
        for english in ["Required", "the ", "Comma-separated", "Note", "entry date"] {
            assert!(
                !description.contains(english),
                "la descripción de `{name}` no debe contener «{english}», got: {description}"
            );
        }
    }

    // Corrección del RED: el marcador se busca sin distinguir mayúsculas —una
    // descripción en español empieza por mayúscula— sin rebajar lo que exige la
    // spec, que es que estén redactadas en español.
    let date = properties["date"]["description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        date.contains("fecha"),
        "`date` debe describirse en español, got: {date}"
    );
    let content = properties["content"]["description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        content.contains("relato") || content.contains("contenido"),
        "`content` debe describirse en español, got: {content}"
    );
    let mood = properties["mood"]["description"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        mood.contains("ánimo"),
        "`mood` debe describirse en español, got: {mood}"
    );
    Ok(())
}

/// Scenario: el registry de producción registra la herramienta.
#[tokio::test]
async fn test_journal_is_registered_once_among_the_production_tools(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let registry = build_tool_registry(&pool);

    assert!(
        registry.get("journal").is_some(),
        "`journal` must be registered in the production registry"
    );

    let definitions = registry.definitions();
    let names: Vec<&str> = definitions.iter().map(|def| def.name.as_str()).collect();
    assert!(
        names.contains(&"journal"),
        "`journal` must be offered among the registered tools: {names:?}"
    );

    let mut sorted = names.clone();
    sorted.sort_unstable();
    let count = sorted.len();
    sorted.dedup();
    assert_eq!(
        count,
        sorted.len(),
        "no two tools may share a name: {names:?}"
    );
    Ok(())
}

/// The tool reports an unknown operation as an invalid argument, like its
/// neighbours.
#[tokio::test]
async fn test_journal_unknown_operation_is_an_invalid_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let err = tool(pool)
        .execute(json!({"operation": "nope"}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
    Ok(())
}

/// `write_entry` requires the day and the story: without them there is nothing
/// to store.
#[tokio::test]
async fn test_journal_write_entry_requires_date_and_content(
) -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool.clone());

    for args in [
        json!({"operation": "write_entry", "content": "sin fecha"}),
        json!({"operation": "write_entry", "date": DAY}),
        json!({"operation": "write_entry", "date": DAY, "content": "   "}),
    ] {
        let err = tool.execute(args.clone()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "{args} must be rejected as an invalid argument; got {err:?}"
        );
    }
    assert_eq!(
        rows_for(&pool, PROFILE, DAY).await,
        0,
        "nothing may be stored from a rejected call"
    );
    Ok(())
}

/// `list_entries` requires the range: without it there is no window to read.
#[tokio::test]
async fn test_journal_list_entries_requires_the_range() -> Result<(), Box<dyn std::error::Error>> {
    let pool = setup().await;
    let tool = tool(pool);

    for args in [
        json!({"operation": "list_entries", "to": DAY}),
        json!({"operation": "list_entries", "from": DAY}),
    ] {
        let err = tool.execute(args.clone()).await.unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidArguments(_)),
            "{args} must be rejected as an invalid argument; got {err:?}"
        );
    }
    Ok(())
}
