use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use valet::embeddings::provider::EmbeddingError;
use valet::embeddings::EmbeddingProvider;
use valet::llm::provider::{
    ChatMessage, ChatRequest, ChatResponse, LLMError, LLMProvider, StreamEvent, TokenUsage,
};
use valet::workers::episodic_memory::{EpisodicMemoryConfig, EpisodicMemoryWorker};

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
    pool
}

/// Asserts that `run_migrations` creates the `messages` table
/// and does NOT include a `conversation_id` column.
#[tokio::test]
async fn test_migrations_creates_messages_table() {
    let pool = setup().await;

    let has_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='messages'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(
        has_table,
        "Expected 'messages' table to exist after migration"
    );

    // Verify no conversation_id column exists
    let column_names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert!(
        !column_names.contains(&"conversation_id".to_string()),
        "Column 'conversation_id' should NOT exist in messages table"
    );
}

/// Asserts that `run_migrations` does NOT create the `conversations` table.
#[tokio::test]
async fn test_migrations_does_not_create_conversations() {
    let pool = setup().await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='conversations'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        count, 0,
        "Table 'conversations' should NOT exist after migration"
    );
}

/// Asserts that migration is idempotent (can be called twice).
#[tokio::test]
async fn test_migration_is_idempotent() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

    // First call
    valet::db::schema::run_migrations(&pool).await.unwrap();

    // Second call — should not error
    valet::db::schema::run_migrations(&pool).await.unwrap();
}

/// The `messages` table gains a nullable `widgets` TEXT column.
#[tokio::test]
async fn test_messages_table_has_widgets_column() {
    let pool = setup().await;

    let columns: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('messages')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let widgets = columns
        .iter()
        .find(|(_cid, name, _ty, _notnull, _dflt, _pk)| name == "widgets")
        .expect("Column 'widgets' should exist in messages table");

    assert_eq!(widgets.2.to_uppercase(), "TEXT", "'widgets' should be TEXT");
    assert_eq!(widgets.3, 0, "'widgets' should be nullable");
}

/// Running the migrations twice keeps the `widgets` column and does not fail.
#[tokio::test]
async fn test_message_widgets_migration_is_idempotent() {
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
    valet::db::schema::run_migrations(&pool).await.unwrap();

    let column_names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
            .fetch_all(&pool)
            .await
            .unwrap();

    assert!(
        column_names.contains(&"widgets".to_string()),
        "Column 'widgets' must exist after running migrations twice"
    );
}

// ── F5c: Tools de Valor — Schema tests ─────────────────────────────────────

/// Asserts that `run_migrations` does NOT leave the tables of the removed tools.
#[tokio::test]
async fn test_migrations_drop_removed_tools_tables() {
    let pool = setup().await;

    for table in &[
        "contacts",
        "contacts_fts",
        "meal_plans",
        "shopping_list",
        "habits",
        "habit_logs",
    ] {
        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(
            !has_table,
            "Expected '{table}' table to be dropped by migration"
        );
    }
}

/// Asserts that idempotent migrations do NOT recreate the removed tools' tables.
#[tokio::test]
async fn test_idempotent_does_not_recreate_removed_tables() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

    // Call twice
    valet::db::schema::run_migrations(&pool).await.unwrap();
    valet::db::schema::run_migrations(&pool).await.unwrap();

    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();

    for table in &[
        "contacts",
        "contacts_fts",
        "meal_plans",
        "shopping_list",
        "habits",
        "habit_logs",
    ] {
        assert!(
            !tables.contains(&table.to_string()),
            "Removed '{table}' table must not be recreated by idempotent migration"
        );
    }
}

// ── Prompts migration (20260929000001_prompts.sql) ─────────────────────────

/// Reads the prompts migration SQL from disk.
fn prompts_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20260929000001_prompts.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// Reads the widget-prompt-guidance migration SQL from disk.
fn widget_guidance_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261004000001_widget_prompt_guidance.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// Reads the consolidator-reliability migration SQL from disk.
fn consolidator_reliability_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261003000003_consolidator_reliability.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// Reads a single setting value, panicking if the key is missing.
async fn setting_value(pool: &SqlitePool, key: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("Failed to read setting '{key}': {e}"))
}

/// Asserts that `run_migrations` seeds a non-empty `system_prompt`.
#[tokio::test]
async fn test_migration_seeds_system_prompt() {
    let pool = setup().await;

    let value = setting_value(&pool, "system_prompt").await;
    assert!(!value.is_empty(), "system_prompt should not be empty");
    assert!(
        value.contains("asistente personal británico"),
        "system_prompt should contain the British assistant personality"
    );
}

/// Asserts that `run_migrations` seeds the archivist prompt with its placeholder.
#[tokio::test]
async fn test_migration_seeds_archivist_prompt() {
    let pool = setup().await;

    let value = setting_value(&pool, "archivist_prompt").await;
    assert!(!value.is_empty(), "archivist_prompt should not be empty");
    assert!(
        value.contains("archivista de memoria"),
        "archivist_prompt should contain 'archivista de memoria'"
    );
    assert!(
        value.contains("{{ BLOQUE_DE_MENSAJES }}"),
        "archivist_prompt should contain the message block placeholder"
    );
}

/// Asserts that `run_migrations` seeds the collapse prompt.
#[tokio::test]
async fn test_migration_seeds_collapse_prompt() {
    let pool = setup().await;

    let value = setting_value(&pool, "collapse_prompt").await;
    assert!(!value.is_empty(), "collapse_prompt should not be empty");
    assert!(
        value.contains("Resume el siguiente texto"),
        "collapse_prompt should contain 'Resume el siguiente texto'"
    );
}

/// An old database with an empty `system_prompt` gets backfilled by the migration.
#[tokio::test]
async fn test_migration_fills_empty_system_prompt() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = '' WHERE key = 'system_prompt'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = prompts_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "system_prompt").await;
    assert!(
        !value.is_empty(),
        "system_prompt should be backfilled when it was empty"
    );
}

/// A non-empty custom `system_prompt` is preserved by the migration.
#[tokio::test]
async fn test_migration_respects_custom_system_prompt() {
    let pool = setup().await;

    sqlx::query(
        "UPDATE settings SET value = 'Mi prompt personalizado' WHERE key = 'system_prompt'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql = prompts_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "system_prompt").await;
    assert_eq!(
        value, "Mi prompt personalizado",
        "A non-empty custom system_prompt must be preserved"
    );
}

// ── Widget prompt guidance (20261004000001_widget_prompt_guidance.sql) ─────

/// The migration appends the widget-guidance section to the `system_prompt`.
#[tokio::test]
async fn test_migration_appends_widget_guidance_section() {
    let pool = setup().await;

    let value = setting_value(&pool, "system_prompt").await;
    assert!(
        value.contains("# Instrucciones de Interfaz y Widgets Interactivos"),
        "system_prompt must contain the widget-guidance header"
    );
    assert!(
        value.contains("render_widget"),
        "system_prompt must mention the render_widget tool"
    );
    assert!(
        value.contains("DEBES invocar"),
        "system_prompt must contain the imperative 'DEBES invocar'"
    );
    assert!(
        value.contains("NO invoques la herramienta"),
        "system_prompt must contain the 'NO invoques la herramienta' rule"
    );
}

/// A custom `system_prompt` is preserved and the section is appended after it.
#[tokio::test]
async fn test_migration_preserves_custom_system_prompt_and_appends_section() {
    let pool = setup().await;

    sqlx::query(
        "UPDATE settings SET value = 'Mi prompt personalizado' WHERE key = 'system_prompt'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql = widget_guidance_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "system_prompt").await;
    assert!(
        value.contains("Mi prompt personalizado"),
        "the custom system_prompt must be preserved"
    );
    assert!(
        value.contains("# Instrucciones de Interfaz y Widgets Interactivos"),
        "system_prompt must contain the widget-guidance header"
    );
    assert!(
        value.find("Mi prompt personalizado")
            < value.find("# Instrucciones de Interfaz y Widgets Interactivos"),
        "the user's custom prompt must appear before the appended section"
    );
}

/// Running the widget-guidance migration twice appends the section only once.
#[tokio::test]
async fn test_widget_guidance_migration_is_idempotent() {
    let pool = setup().await;

    let sql = widget_guidance_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "system_prompt").await;
    assert_eq!(
        value
            .matches("# Instrucciones de Interfaz y Widgets Interactivos")
            .count(),
        1,
        "the widget-guidance header must appear exactly once after two runs"
    );
}

// ── Consolidator reliability (20261003000003_consolidator_reliability.sql) ──

/// After every migration, the Semantic generation role does not reason.
#[tokio::test]
async fn test_migration_semantic_reasoning_is_off_after_migrations() {
    let pool = setup().await;
    assert_eq!(
        setting_value(&pool, "GENERATION_SEMANTIC_REASONING").await,
        "off"
    );
}

/// A legacy `low` is corrected to `off` by the reliability migration.
#[tokio::test]
async fn test_consolidator_migration_forces_semantic_reasoning_off() {
    let pool = setup().await;
    sqlx::query("UPDATE settings SET value='low' WHERE key='GENERATION_SEMANTIC_REASONING'")
        .execute(&pool)
        .await
        .unwrap();
    let sql = consolidator_reliability_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "GENERATION_SEMANTIC_REASONING").await,
        "off"
    );
}

/// Any reasoning value other than `low` is respected.
#[tokio::test]
async fn test_consolidator_migration_respects_other_reasoning() {
    let pool = setup().await;
    sqlx::query("UPDATE settings SET value='medium' WHERE key='GENERATION_SEMANTIC_REASONING'")
        .execute(&pool)
        .await
        .unwrap();
    let sql = consolidator_reliability_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "GENERATION_SEMANTIC_REASONING").await,
        "medium"
    );
}

/// After every migration, the persistent-memory budget is 800.
#[tokio::test]
async fn test_migration_bumps_persistent_memory_budget_to_800() {
    let pool = setup().await;
    assert_eq!(
        setting_value(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS").await,
        "800"
    );
}

/// A custom budget (not 500) is respected by the reliability migration.
#[tokio::test]
async fn test_consolidator_migration_respects_custom_budget() {
    let pool = setup().await;
    sqlx::query("UPDATE settings SET value='1200' WHERE key='PERSISTENT_MEMORY_BUDGET_TOKENS'")
        .execute(&pool)
        .await
        .unwrap();
    let sql = consolidator_reliability_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "PERSISTENT_MEMORY_BUDGET_TOKENS").await,
        "1200"
    );
}

/// The seeded consolidator prompt carries the taxonomy, the placeholders and the
/// marker, and forbids duplicating and inventing.
#[tokio::test]
async fn test_migration_seeds_consolidator_prompt_taxonomy() {
    let pool = setup().await;
    let value = setting_value(&pool, "consolidator_prompt").await;
    for needle in [
        "preferences_and_tastes",
        "dislikes_and_dealbreakers",
        "{{ ESTADO_ACTUAL }}",
        "{{ BLOQUE_DE_MENSAJES }}",
        "consolidador de memoria persistente",
    ] {
        assert!(
            value.contains(needle),
            "consolidator_prompt must contain {needle}"
        );
    }
}

/// A custom consolidator prompt is preserved by the reliability migration.
#[tokio::test]
async fn test_migration_preserves_custom_consolidator_prompt() {
    let pool = setup().await;
    sqlx::query(
        "UPDATE settings SET value='Mi consolidador personalizado' WHERE key='consolidator_prompt'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let sql = consolidator_reliability_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        setting_value(&pool, "consolidator_prompt").await,
        "Mi consolidador personalizado"
    );
}

// ── 11.1: reset of the index source ────────────────────────────────────────

/// A no-network LLM double: returns a parseable memory card immediately.
struct NoNetworkLLM;

#[async_trait]
impl LLMProvider for NoNetworkLLM {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, LLMError> {
        // The worker now makes two calls per pass: the archivist (Layer B) and
        // the consolidator (Layer C). Reply with a valid JSON state for the
        // latter, told apart by its prompt marker.
        let system_content = request
            .messages
            .first()
            .map(|m| m.content.as_str())
            .unwrap_or_default();
        let content = if system_content.contains("consolidador de memoria persistente") {
            r#"{"schema_version":1,"user_profile":{"note":"reset test"},"system_rules":["una regla"]}"#
                .to_string()
        } else {
            "\
- FECHA/CONTEXTO: test de reseteo
- TEMAS TRATADOS: reconstrucción del índice
- HECHOS Y DECISIONES: la fuente se ha reseteado
- SÍNTESIS: el worker rearchiva desde el mensaje original"
                .to_string()
        };
        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".into(),
                content,
                tool_calls: None,
                tool_result: None,
                tool_call_id: None,
            },
            usage: Some(TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                cached_tokens: 0,
                reasoning_tokens: 0,
                cost: 0.0,
            }),
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> Result<
        Pin<Box<dyn tokio_stream::Stream<Item = Result<StreamEvent, LLMError>> + Send>>,
        LLMError,
    > {
        unimplemented!("chat_stream is not used in this test")
    }
}

/// A no-network embedding double: returns a valid 1024-dim vector.
struct NoNetworkEmbedding;

#[async_trait]
impl EmbeddingProvider for NoNetworkEmbedding {
    async fn embed(&self, _input: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.1f32; 1024])
    }
}

/// 11.1 — after `reset_memory_source`, the derived data is gone, the messages
/// are un-indexed, and the worker re-archives from the original messages using
/// test doubles (no network at all).
#[tokio::test]
async fn test_reset_source_makes_worker_rearchive() {
    let pool = setup().await;

    // Simulate an already-built index: one indexed message and one derived card
    // plus its vector.
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, role, content, tokens_count, is_indexed, summary_ref, created_at) \
         VALUES ('m1', 'user', 'contenido original', 500, 1, 'mem-old', ?1)",
    )
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO memory (id, content, tokens_count) VALUES ('mem-old', 'vieja ficha', 10)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let old_vector = serde_json::to_string(&vec![0.1f32; 1024]).unwrap();
    sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES ('mem-old', vec_f32(?1))")
        .bind(&old_vector)
        .execute(&pool)
        .await
        .unwrap();

    // Reset the source.
    let report = valet::embeddings::reindex::reset_memory_source(&pool)
        .await
        .expect("reset_memory_source should succeed");
    assert_eq!(report.messages_reset, 1, "one message must be un-indexed");
    assert_eq!(report.memories_deleted, 1, "one card must be deleted");
    assert_eq!(report.vectors_deleted, 1, "one vector must be deleted");

    // Derived data is gone and the message is un-indexed.
    let count_memory: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_memory, 0, "memory must be empty after reset");

    let count_vec: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vec_memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_vec, 0, "vec_memory must be empty after reset");

    let (is_indexed, summary_ref): (bool, Option<String>) =
        sqlx::query_as("SELECT is_indexed, summary_ref FROM messages WHERE id = 'm1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!is_indexed, "message must be un-indexed after reset");
    assert!(
        summary_ref.is_none(),
        "summary_ref must be NULL after reset"
    );

    // The worker must re-archive from the original message. `batch_tokens = 1`
    // makes the single 500-token message meet the batch condition immediately.
    let (memory_tx, memory_rx) = tokio::sync::mpsc::channel::<()>(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    let _handle = EpisodicMemoryWorker::start(
        pool.clone(),
        Arc::new(NoNetworkLLM),
        Arc::new(NoNetworkEmbedding),
        memory_rx,
        shutdown_rx,
        EpisodicMemoryConfig {
            batch_tokens: 1,
            ..Default::default()
        },
    );

    memory_tx.send(()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let count_memory_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count_memory_after, 1,
        "the worker must re-archive a card from the message after the reset"
    );

    let count_vec_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vec_memory")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count_vec_after, 1,
        "the worker must write a fresh vector into vec_memory after the reset"
    );

    let reindexed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE is_indexed = 1 AND summary_ref IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        reindexed, 1,
        "the message must be indexed again by the worker"
    );

    let _ = shutdown_tx.send(());
}

/// Running the prompts migration twice is idempotent and keeps one row per key.
#[tokio::test]
async fn test_migration_prompts_idempotent() {
    let pool = setup().await;

    let sql = prompts_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    for key in &["system_prompt", "archivist_prompt", "collapse_prompt"] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "Expected exactly one row for key '{key}'");
    }
}

// ── Skill router (20261007000001_skill_router.sql) ─────────────────────────

/// Reads the skill-router migration SQL from disk.
fn skill_router_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261007000001_skill_router.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// After migrating, the `ROUTER_*` knobs hold their defaults and the eight
/// `SKILL_*_PROMPT` fragments exist and are non-empty.
#[tokio::test]
async fn test_skill_router_settings_seeded_with_defaults() {
    let pool = setup().await;

    let router_defaults = [
        ("ROUTER_ENABLED", "false"),
        ("ROUTER_MODEL", "typesafe/jev-1.13"),
        ("ROUTER_THRESHOLD", "0.3"),
        ("ROUTER_TIMEOUT_MS", "800"),
        ("ROUTER_HISTORY_TURNS", "2"),
    ];
    for (key, value) in router_defaults {
        assert_eq!(
            setting_value(&pool, key).await,
            value,
            "after migrations, settings.{key} must hold its default"
        );
    }

    let skill_keys = [
        "SKILL_AGENDA_PROMPT",
        "SKILL_TAREAS_PROMPT",
        "SKILL_RECORDATORIOS_PROMPT",
        "SKILL_NOTAS_PROMPT",
        "SKILL_CLIMA_PROMPT",
        "SKILL_LUGARES_PROMPT",
        "SKILL_BUSQUEDA_WEB_PROMPT",
        "SKILL_MEMORIA_PROMPT",
    ];
    for key in skill_keys {
        let value = setting_value(&pool, key).await;
        assert!(!value.trim().is_empty(), "fragment {key} must not be empty");
        assert!(
            value.contains("# SKILL ACTIVA:"),
            "fragment {key} must carry its section heading"
        );
    }
}

/// A hand-edited, non-empty value survives a second run of the migration.
#[tokio::test]
async fn test_skill_router_migration_respects_edited_value() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = '0.9' WHERE key = 'ROUTER_THRESHOLD'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = skill_router_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD").await,
        "0.9",
        "a hand-edited, non-empty value must not be overwritten by the upsert"
    );
}
