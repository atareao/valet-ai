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

// ── Local tasks removal (20261028000001_remove_local_tasks.sql) ───────────

/// Asserts that `run_migrations` drops the local `tasks` table and its
/// external-content FTS index, leaving the other tables and indexes untouched.
#[tokio::test]
async fn test_migrations_drop_local_tasks_tables() {
    let pool = setup().await;

    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();

    for removed in &["tasks", "tasks_fts"] {
        assert!(
            !tables.contains(&removed.to_string()),
            "Expected '{removed}' table to be dropped by migration"
        );
    }

    for kept in &[
        "events",
        "notes",
        "messages",
        "reminders",
        "events_fts",
        "notes_fts",
        "messages_fts",
    ] {
        assert!(
            tables.contains(&kept.to_string()),
            "Expected '{kept}' table to survive the migration"
        );
    }
}

/// Asserts that running the migrations twice does not fail and does not
/// recreate the `tasks` tables (both `DROP`s carry `IF EXISTS`).
#[tokio::test]
async fn test_remove_local_tasks_migration_is_idempotent() {
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

    for removed in &["tasks", "tasks_fts"] {
        assert!(
            !tables.contains(&removed.to_string()),
            "Removed '{removed}' table must not be recreated by idempotent migration"
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

/// The guidance migration appends the widget-guidance section to a base prompt
/// that does not already carry it. The tuning migration later relocates that
/// section to `SKILL_WIDGETS_PROMPT`, so this exercises the append in isolation.
#[tokio::test]
async fn test_migration_appends_widget_guidance_section() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = 'base' WHERE key = 'system_prompt'")
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
            finish_reason: None,
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

/// Reads the router-enabled migration SQL from disk.
fn router_enabled_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261008000002_router_enabled_by_default.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// After migrating, the `ROUTER_*` knobs hold their defaults and the eight
/// `SKILL_*_PROMPT` fragments exist and are non-empty.
#[tokio::test]
async fn test_skill_router_settings_seeded_with_defaults() {
    let pool = setup().await;

    let router_defaults = [
        // Flipped to true by 20261008000002_router_enabled_by_default.sql.
        ("ROUTER_ENABLED", "true"),
        ("ROUTER_MODEL", "typesafe/jev-1.13"),
        ("ROUTER_TIMEOUT_MS", "800"),
        // Bumped from 2 by 20261008000001_skill_router_tuning.sql (measured).
        ("ROUTER_HISTORY_TURNS", "6"),
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

/// After the migrations, the router boots enabled: 20261008000002 flips the
/// seeded `ROUTER_ENABLED` from `false` to `true` so a migrated database routes
/// out of the box.
#[tokio::test]
async fn test_router_enabled_is_true_after_migrations() {
    let pool = setup().await;

    assert_eq!(
        setting_value(&pool, "ROUTER_ENABLED").await,
        "true",
        "a freshly migrated database must boot with the router enabled"
    );
}

/// A hand-edited, non-empty value survives a second run of the migration.
#[tokio::test]
async fn test_skill_router_migration_respects_edited_value() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = 'custom/model' WHERE key = 'ROUTER_MODEL'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = skill_router_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_MODEL").await,
        "custom/model",
        "a hand-edited, non-empty value must not be overwritten by the upsert"
    );
}

/// The flip only touches the exact seeded value: a user who turned the router
/// off with the `0` spelling keeps it off after re-applying the migration.
#[tokio::test]
async fn test_router_enabled_migration_respects_user_value() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = '0' WHERE key = 'ROUTER_ENABLED'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = router_enabled_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_ENABLED").await,
        "0",
        "a user-chosen value that is not the seeded default must survive the migration"
    );
}

/// The documented caveat is executable: an explicit `false` that started from
/// the seeded value is indistinguishable from it, so the migration flips it
/// once. That is the price of a plain SQL migration; the window is small.
#[tokio::test]
async fn test_router_enabled_migration_flips_the_seeded_false_once() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = 'false' WHERE key = 'ROUTER_ENABLED'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = router_enabled_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_ENABLED").await,
        "true",
        "the seeded `false` is flipped once — see the migration's caveat"
    );
}

// ── Skill-router tuning (20261008000001_skill_router_tuning.sql) ────────────

/// Reads the skill-router-tuning migration SQL from disk.
fn skill_router_tuning_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261008000001_skill_router_tuning.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// After migrating, the measured history window holds its new default and the
/// widget's own threshold has been retired by the always-on migration.
#[tokio::test]
async fn test_skill_router_tuning_seeds_defaults() {
    let pool = setup().await;

    assert_eq!(
        setting_value(&pool, "ROUTER_HISTORY_TURNS").await,
        "6",
        "the measured history window must be 6"
    );
    assert_eq!(
        setting_value_opt(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
        None,
        "the seeded widget override is retired by 20261101000001"
    );
}

/// A user value is never overwritten: the bump only touches our own default.
/// The widget override is gone after the full migration set —`20261101000001`
/// retires it— so the test has to write it; that write is exactly what a user
/// tuning the threshold from settings does.
#[tokio::test]
async fn test_skill_router_tuning_respects_user_values() {
    let pool = setup().await;

    for (key, value) in [
        ("ROUTER_HISTORY_TURNS", "9"),
        ("ROUTER_THRESHOLD_WIDGETS", "0.77"),
    ] {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) \
             VALUES (?1, ?2, datetime('now')) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
             updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await
        .unwrap();
    }

    let sql = skill_router_tuning_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_HISTORY_TURNS").await,
        "9",
        "a user-tuned history window must not be overwritten"
    );
    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
        "0.77",
        "a user-tuned widget threshold must not be overwritten"
    );
}

/// The widget guide leaves the base prompt and lands in the widgets fragment.
#[tokio::test]
async fn test_skill_router_tuning_moves_widget_guide_out_of_system_prompt() {
    let pool = setup().await;

    let system_prompt = setting_value(&pool, "system_prompt").await;
    assert!(
        !system_prompt.contains("# Instrucciones de Interfaz y Widgets Interactivos"),
        "the base prompt must no longer carry the widget guide"
    );

    let fragment = setting_value(&pool, "SKILL_WIDGETS_PROMPT").await;
    assert!(
        !fragment.trim().is_empty(),
        "SKILL_WIDGETS_PROMPT must not be empty"
    );
    assert!(
        fragment.contains("# Instrucciones de Interfaz y Widgets Interactivos"),
        "the fragment must carry the relocated guide header"
    );
    assert!(
        fragment.contains("render_widget"),
        "the fragment must carry the relocated guide body"
    );
}

/// An edited widget block is left where it is; nothing of the user's is removed.
#[tokio::test]
async fn test_skill_router_tuning_keeps_edited_widget_block() {
    let pool = setup().await;

    let edited = "Mi prompt personalizado\n\n# Instrucciones de Interfaz y Widgets Interactivos\n\n(editado por el usuario)";
    sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'system_prompt'")
        .bind(edited)
        .execute(&pool)
        .await
        .unwrap();

    let sql = skill_router_tuning_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "system_prompt").await,
        edited,
        "an edited widget block must not be removed"
    );
}

/// The five regrouped/new fragments exist and are non-empty.
#[tokio::test]
async fn test_skill_router_tuning_seeds_new_fragments() {
    let pool = setup().await;

    for key in [
        "SKILL_PENDIENTES_PROMPT",
        "SKILL_RECUERDOS_PROMPT",
        "SKILL_ENTORNO_PROMPT",
        "SKILL_WEB_PROMPT",
        "SKILL_WIDGETS_PROMPT",
    ] {
        let value = setting_value(&pool, key).await;
        assert!(!value.trim().is_empty(), "fragment {key} must not be empty");
    }
}

/// Re-applying the tuning migration leaves the same state. The always-on
/// migration has already retired the seeded widget threshold, so the first
/// application re-seeds it; the second must be a no-op on top of that.
#[tokio::test]
async fn test_skill_router_tuning_is_idempotent() {
    let pool = setup().await;

    let sql = skill_router_tuning_migration_sql();

    // First application over the always-on state.
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = |pool: SqlitePool| async move {
        (
            setting_value(&pool, "ROUTER_HISTORY_TURNS").await,
            setting_value(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
            setting_value(&pool, "system_prompt").await,
            setting_value(&pool, "SKILL_WIDGETS_PROMPT").await,
        )
    };
    let before = snapshot(pool.clone()).await;

    // Second application.
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let after = snapshot(pool.clone()).await;
    assert_eq!(
        before, after,
        "re-applying the migration must leave the same state"
    );
}

// ── Skill selection (20261009000002_skill_selection.sql) ────────────────────

/// Reads the skill-selection migration SQL from disk.
fn skill_selection_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261009000002_skill_selection.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// Like `setting_value`, but returns `None` for an absent key instead of
/// panicking. Needed to assert that a retired key is really gone.
async fn setting_value_opt(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap()
}

/// Rebuilds the pre-migration state that the forward DDL needs: the retired
/// `tools.enabled` column, a global `ROUTER_THRESHOLD` and the widget override.
/// Re-running the new migration requires this because `ALTER TABLE ... DROP
/// COLUMN` is a one-shot DDL (a second run would otherwise fail with "no such
/// column"). The widget override is part of that historical state —it was
/// seeded by `20261008000001`— and `20261101000001` retires it, so the full
/// migration set no longer leaves it behind and the reconstruction must restore
/// it.
async fn reconstruct_pre_skill_selection(pool: &SqlitePool, global_threshold: &str) {
    sqlx::query("ALTER TABLE tools ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) \
         VALUES ('ROUTER_THRESHOLD', ?1, datetime('now'))",
    )
    .bind(global_threshold)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) \
         VALUES ('ROUTER_THRESHOLD_WIDGETS', '0.20', datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
         updated_at = datetime('now')",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// After migrating, every skill carries an explicit `ROUTER_SKILL_<ID>_ENABLED`
/// seeded to `true`.
#[tokio::test]
async fn test_skill_selection_seeds_skill_enabled_flags() {
    let pool = setup().await;

    for id in [
        "agenda",
        "pendientes",
        "recuerdos",
        "entorno",
        "web",
        "widgets",
    ] {
        let key = format!("ROUTER_SKILL_{}_ENABLED", id.to_ascii_uppercase());
        assert_eq!(
            setting_value(&pool, &key).await,
            "true",
            "skill {id} must be seeded enabled"
        );
    }
}

/// The retired global threshold is gone, as is the seeded widget override.
#[tokio::test]
async fn test_skill_selection_removes_the_global_threshold() {
    let pool = setup().await;

    assert_eq!(
        setting_value_opt(&pool, "ROUTER_THRESHOLD").await,
        None,
        "ROUTER_THRESHOLD must be retired"
    );
    assert_eq!(
        setting_value_opt(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
        None,
        "the seeded widget override is retired by 20261101000001"
    );
    // With the default global there is nothing to materialise: the compiled
    // fallback already yields 0.10 for the five domains.
    for id in ["AGENDA", "PENDIENTES", "RECUERDOS", "ENTORNO", "WEB"] {
        let key = format!("ROUTER_THRESHOLD_{id}");
        assert_eq!(
            setting_value_opt(&pool, &key).await,
            None,
            "{key} must not be created when the global was still the default"
        );
    }
}

/// The `enabled` column is dropped from `tools`.
#[tokio::test]
async fn test_skill_selection_drops_the_tools_enabled_column() {
    let pool = setup().await;

    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('tools')")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert!(
        !columns.contains(&"enabled".to_string()),
        "the tools table must no longer carry `enabled`, got {columns:?}"
    );
}

/// A customised global threshold is materialised into the five routed domains
/// that have no override of their own.
#[tokio::test]
async fn test_skill_selection_materializes_a_custom_global_threshold() {
    let pool = setup().await;

    reconstruct_pre_skill_selection(&pool, "0.42").await;

    let sql = skill_selection_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    for id in ["AGENDA", "PENDIENTES", "RECUERDOS", "ENTORNO", "WEB"] {
        let key = format!("ROUTER_THRESHOLD_{id}");
        assert_eq!(
            setting_value(&pool, &key).await,
            "0.42",
            "{key} must inherit the customised global threshold"
        );
    }
    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
        "0.20",
        "widgets keeps its own threshold, not the global one"
    );
    assert_eq!(
        setting_value_opt(&pool, "ROUTER_THRESHOLD").await,
        None,
        "the global key is retired after materialisation"
    );
}

/// A non-numeric global threshold must not be materialised: a garbage value
/// would only spawn rows the reader rejects anyway.
#[tokio::test]
async fn test_skill_selection_ignores_a_non_numeric_global_threshold() {
    let pool = setup().await;

    reconstruct_pre_skill_selection(&pool, "not-a-number").await;

    let sql = skill_selection_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    for id in ["AGENDA", "PENDIENTES", "RECUERDOS", "ENTORNO", "WEB"] {
        let key = format!("ROUTER_THRESHOLD_{id}");
        assert_eq!(
            setting_value_opt(&pool, &key).await,
            None,
            "{key} must not be created from a non-numeric global"
        );
    }
    assert_eq!(
        setting_value_opt(&pool, "ROUTER_THRESHOLD").await,
        None,
        "the global key is retired even when its value was unusable"
    );
}

/// An existing per-skill override wins over the materialised global.
#[tokio::test]
async fn test_skill_selection_does_not_overwrite_a_skill_override() {
    let pool = setup().await;

    reconstruct_pre_skill_selection(&pool, "0.42").await;
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) \
         VALUES ('ROUTER_THRESHOLD_AGENDA', '0.90', datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql = skill_selection_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD_AGENDA").await,
        "0.90",
        "an explicit per-skill override must survive the migration"
    );
    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD_WEB").await,
        "0.42",
        "the domains without an override still inherit the global"
    );
}

/// A skill a user explicitly disabled keeps its `false` after the migration.
#[tokio::test]
async fn test_skill_selection_respects_a_disabled_skill() {
    let pool = setup().await;

    reconstruct_pre_skill_selection(&pool, "0.10").await;

    sqlx::query("UPDATE settings SET value = 'false' WHERE key = 'ROUTER_SKILL_WEB_ENABLED'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = skill_selection_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_SKILL_WEB_ENABLED").await,
        "false",
        "a user-disabled skill must stay disabled"
    );
}

/// Re-applying the whole migration leaves the same state. The one-shot
/// `DROP COLUMN` is re-armed by rebuilding the pre-migration column first.
#[tokio::test]
async fn test_skill_selection_is_idempotent() {
    let pool = setup().await;

    let sql = skill_selection_migration_sql();

    reconstruct_pre_skill_selection(&pool, "0.42").await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = |pool: SqlitePool| async move {
        (
            setting_value(&pool, "ROUTER_SKILL_AGENDA_ENABLED").await,
            setting_value(&pool, "ROUTER_THRESHOLD_AGENDA").await,
            setting_value(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
            setting_value_opt(&pool, "ROUTER_THRESHOLD").await,
        )
    };
    let before = snapshot(pool.clone()).await;

    // Second application, from the same (reconstructed) pre-state.
    reconstruct_pre_skill_selection(&pool, "0.42").await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let after = snapshot(pool.clone()).await;
    assert_eq!(
        before, after,
        "re-applying the migration must leave the same state"
    );
}

// ── Widgets always on (20261101000001_widgets_always_on.sql) ────────────────

/// Reads the widgets-always-on migration SQL from disk.
fn widgets_always_on_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261101000001_widgets_always_on.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

/// The widget guide exactly as `20261008000001` seeded it, extracted from that
/// migration so the test never hard-codes the old text.
fn seeded_widgets_guide() -> String {
    let sql = skill_router_tuning_migration_sql();
    let marker = "VALUES ('SKILL_WIDGETS_PROMPT', '";
    let start = sql
        .find(marker)
        .expect("the tuning migration must seed SKILL_WIDGETS_PROMPT")
        + marker.len();
    let end = sql[start..]
        .find("', datetime('now'))")
        .expect("the seeded guide must be a single-quoted literal")
        + start;
    sql[start..end].to_string()
}

/// The seeded `ROUTER_THRESHOLD_WIDGETS` is deleted, in either spelling of the
/// default, so the widget stops being conditioned by the classifier.
#[tokio::test]
async fn test_widgets_always_on_retires_the_seeded_widget_threshold() {
    let pool = setup().await;

    let sql = widgets_always_on_migration_sql();
    for spelling in ["0.20", "0.2"] {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) \
             VALUES ('ROUTER_THRESHOLD_WIDGETS', ?1, datetime('now')) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
             updated_at = datetime('now')",
        )
        .bind(spelling)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            setting_value_opt(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
            None,
            "the seeded widget threshold ({spelling}) must be retired"
        );
    }
}

/// A user-raised threshold re-conditions the skill and is never deleted.
#[tokio::test]
async fn test_widgets_always_on_respects_a_user_threshold() {
    let pool = setup().await;

    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) \
         VALUES ('ROUTER_THRESHOLD_WIDGETS', '0.77', datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
        "0.77",
        "a user-raised threshold must survive the retirement"
    );
}

/// The pristine seeded guide is replaced by the new one.
#[tokio::test]
async fn test_widgets_always_on_replaces_the_pristine_guide() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'SKILL_WIDGETS_PROMPT'")
        .bind(seeded_widgets_guide())
        .execute(&pool)
        .await
        .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "SKILL_WIDGETS_PROMPT").await;
    assert!(
        value.contains("## Regla de oro"),
        "the new guide must have replaced the seeded one"
    );
    assert!(
        value.contains("dos o más opciones etiquetadas"),
        "the new guide's option rule must be present"
    );
    assert!(
        !value.contains("Reglas de Invocación"),
        "the old guide's section must be gone"
    );
}

/// An edited guide is user content and is never overwritten.
#[tokio::test]
async fn test_widgets_always_on_keeps_an_edited_guide() {
    let pool = setup().await;

    let edited = "Mi guía de widgets personalizada";
    sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'SKILL_WIDGETS_PROMPT'")
        .bind(edited)
        .execute(&pool)
        .await
        .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "SKILL_WIDGETS_PROMPT").await,
        edited,
        "an edited guide must not be overwritten"
    );
}

/// When the fragment is absent it is seeded with the new guide.
#[tokio::test]
async fn test_widgets_always_on_seeds_a_missing_guide() {
    let pool = setup().await;

    sqlx::query("DELETE FROM settings WHERE key = 'SKILL_WIDGETS_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = setting_value(&pool, "SKILL_WIDGETS_PROMPT").await;
    assert!(
        value.contains("# Instrucciones de Interfaz y Widgets Interactivos"),
        "the missing guide must be seeded with the new header"
    );
    assert!(
        value.contains("## Regla de oro"),
        "the missing guide must be seeded with the new body"
    );
}

/// A present-but-blank fragment is user content: it is neither overwritten nor
/// re-seeded, because the seeded-key guard only fires for an absent key.
#[tokio::test]
async fn test_widgets_always_on_leaves_a_blank_guide_alone() {
    let pool = setup().await;

    sqlx::query("UPDATE settings SET value = '   ' WHERE key = 'SKILL_WIDGETS_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        setting_value(&pool, "SKILL_WIDGETS_PROMPT").await,
        "   ",
        "a blank fragment must be left alone"
    );
}

/// Re-applying the migration over the pre-migration state leaves it unchanged.
#[tokio::test]
async fn test_widgets_always_on_is_idempotent() {
    let pool = setup().await;

    // Reinstate the state the migration is meant to transform.
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) \
         VALUES ('ROUTER_THRESHOLD_WIDGETS', '0.20', datetime('now'))",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'SKILL_WIDGETS_PROMPT'")
        .bind(seeded_widgets_guide())
        .execute(&pool)
        .await
        .unwrap();

    let sql = widgets_always_on_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = |pool: SqlitePool| async move {
        (
            setting_value_opt(&pool, "ROUTER_THRESHOLD_WIDGETS").await,
            setting_value(&pool, "SKILL_WIDGETS_PROMPT").await,
        )
    };
    let before = snapshot(pool.clone()).await;

    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let after = snapshot(pool.clone()).await;
    assert_eq!(
        before, after,
        "re-applying the migration must leave the same state"
    );
}

/// The default guide is prescriptive: it names the three widgets, points at
/// `render_widget`, states the option rule, makes that rule trump the no-widget
/// cases and scopes those cases tightly.
#[tokio::test]
async fn test_default_widget_guide_is_prescriptive() {
    let pool = setup().await;
    let guide = setting_value(&pool, "SKILL_WIDGETS_PROMPT").await;

    for widget in ["QuickForm", "Checklist", "LocationWidget"] {
        assert!(
            guide.contains(widget),
            "the guide must name the {widget} widget"
        );
    }
    assert!(
        guide.contains("render_widget"),
        "the guide must point at the render_widget tool"
    );
    assert!(
        guide.contains("dos o más opciones etiquetadas"),
        "the guide must state the option rule"
    );
    assert!(
        guide.contains("La regla de las opciones tiene prioridad sobre todo lo demás"),
        "the option rule must be declared to trump the no-widget cases"
    );
    assert!(
        guide.contains(
            "Es una explicación conceptual que no pide elegir nada ni lleva lista que marcar"
        ),
        "the first no-widget case must exclude options and lists"
    );
    assert!(
        guide.contains("Es un dato único y directo: una fecha, un número, un nombre"),
        "the second no-widget case must be a single direct datum"
    );

    // Point 1 — a widget only appears if `render_widget` runs in that same turn,
    // and claiming otherwise is forbidden. Anchored on the exact literals of the
    // new guide (`design.md` → «La guía nueva (texto exacto)»).
    assert!(
        guide.contains("## Regla de oro"),
        "point 1: the guide must carry the «Regla de oro» section"
    );
    assert!(
        guide.contains(
            "Un widget **solo** aparece si ejecutas `render_widget` en esa misma respuesta"
        ),
        "point 1: the guide must state a widget only appears when the tool runs"
    );
    assert!(
        guide
            .contains("NUNCA digas que has mostrado un widget, un formulario, una lista o un mapa"),
        "point 1: the guide must forbid claiming a widget was shown without running the tool"
    );

    // Point 6 — the widget-response processing rule is preserved. Exact literals
    // from the «Después de un widget» section.
    assert!(
        guide.contains("[widget:Checklist#w3] submit"),
        "point 6: the guide must show the widget submission message shape"
    );
    assert!(
        guide.contains("Procesa esa respuesta de inmediato y confirma el resultado en texto"),
        "point 6: the guide must keep processing the widget response immediately"
    );

    // Negative of point 4 — the guide must NOT reintroduce the generic prose
    // authorization the retired guide carried (`Responde únicamente con texto
    // plano`, under `## Reglas de Invocación`). Reintroducing it is precisely the
    // regression point 4 forbids: it could amparar a list of options in prose.
    assert!(
        !guide.contains("Responde únicamente con texto plano"),
        "point 4: the guide must NOT authorize answering in plain prose \
         (old-guide regression)"
    );
    assert!(
        !guide.contains("## Reglas de Invocación"),
        "point 4: the retired «Reglas de Invocación» heading must not come back"
    );
}
