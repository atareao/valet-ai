//! Schema and seeding tests for the `journal-worker` change:
//! `migrations/20261018000002_journal_worker.sql`.
//!
//! Two chores in one migration, tested in group 7 of `tasks.md` (and, for the
//! seeded prompt text, the reordered tasks 3.1–3.3):
//!
//!   1) Widen the `CHECK` of `llm_requests.kind` to admit `'journal'`. SQLite
//!      cannot ALTER a CHECK in place, so the migration rebuilds the table with
//!      the documented idiom (the same one `20261010000003_timeline_events.sql`
//!      used for `'timeline'`): a scratch table with the widened CHECK, a copy
//!      of every row, a drop of the old table and a rename. **Every existing
//!      row survives with its `kind` unchanged.**
//!   2) Seed the journal settings (`journal_prompt`, `journal_enabled`,
//!      `journal_hour`) with the timeline-seed idiom
//!      (`20261010000004_timeline_prompts.sql`): an upsert that only fills an
//!      absent or empty value and never overwrites a present one.
//!      `journal_last_judged_day` is **NOT** seeded: its absence is the
//!      «never judged» state the worker boots from.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// A brand-new, empty in-memory database.
async fn in_memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap()
}

/// A database with every migration applied (`run_migrations()`).
async fn setup() -> SqlitePool {
    let pool = in_memory_pool().await;
    valet::db::schema::run_migrations(&pool).await.unwrap();
    pool
}

/// Version of `migrations/20261018000002_journal_worker.sql`.
const JOURNAL_WORKER_VERSION: i64 = 20261018000002;

/// Read the journal-worker migration SQL from disk.
fn journal_worker_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261018000002_journal_worker.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `journal-worker` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// The state shipped by the previous release: every migration **older** than
/// `20261018000002` applied by hand, so the journal-worker migration has not
/// run yet, `llm_requests.kind` still forbids `'journal'` and the journal
/// settings are not seeded.
async fn pool_before_journal_worker() -> SqlitePool {
    // `run_migrations` registers the `sqlite-vec` extension before anything
    // else, and `sqlite3_auto_extension` only reaches connections opened *after*
    // it runs. One of the older migrations creates a `vec0` virtual table, so
    // the throwaway pool below exists to trigger the registration.
    let throwaway = in_memory_pool().await;
    valet::db::schema::run_migrations(&throwaway).await.unwrap();
    drop(throwaway);

    let pool = in_memory_pool().await;
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrator = sqlx::migrate::Migrator::new(manifest.join("migrations"))
        .await
        .unwrap();
    let mut migrations: Vec<sqlx::migrate::Migration> = migrator.iter().cloned().collect();
    migrations.sort();
    for migration in &migrations {
        if migration.version < JOURNAL_WORKER_VERSION {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("applying migration {} failed: {e}", migration.version));
        }
    }
    pool
}

/// Insert one `llm_requests` row with the given `kind` (no profile: the tests
/// here are about the rebuilt table, not the FK graph).
async fn insert_request(pool: &SqlitePool, id: &str, kind: &str) {
    sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'm', ?2)")
        .bind(id)
        .bind(kind)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("inserting llm_request '{id}' (kind={kind}) must succeed: {e}"));
}

/// The value of a setting, panicking if the key is missing.
async fn setting_value(pool: &SqlitePool, key: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("setting '{key}' must exist: {e}"))
}

/// The value of a setting, or `None` when the key is absent.
async fn setting_value_opt(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap()
}

// ── 7.1: the widened CHECK, the surviving rows and the idempotent rebuild ────

/// Scenario: after the migration the CHECK of `llm_requests.kind` accepts
/// `'journal'` (and keeps accepting every prior origin, `timeline` included),
/// while an unknown origin is still rejected.
#[tokio::test]
async fn test_llm_requests_kind_check_accepts_journal_and_timeline() {
    let pool = setup().await;

    // The new origin inserts cleanly…
    insert_request(&pool, "row-journal", "journal").await;
    // …and so does the timeline, which already survived the previous rebuild.
    insert_request(&pool, "row-timeline", "timeline").await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "the two rows must exist side by side");

    // An origin outside the closed set is still rejected by the CHECK.
    let invalid = sqlx::query(
        "INSERT INTO llm_requests (id, model, kind) VALUES ('row-bad', 'm', 'invalid_kind')",
    )
    .execute(&pool)
    .await;
    assert!(
        invalid.is_err(),
        "kind='invalid_kind' must be rejected by the widened CHECK"
    );
}

/// Scenario: rows of all the previous origins — `timeline` included — survive
/// the table rebuild with their `kind` unchanged.
#[tokio::test]
async fn test_llm_requests_migration_preserves_all_previous_rows() {
    let pool = pool_before_journal_worker().await;

    for (id, kind) in [
        ("pre-chat", "chat"),
        ("pre-router", "router"),
        ("pre-archivist", "archivist"),
        ("pre-consolidator", "consolidator"),
        ("pre-collapse", "collapse"),
        ("pre-timeline", "timeline"),
    ] {
        insert_request(&pool, id, kind).await;
    }

    let sql = journal_worker_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the journal-worker migration must apply on top of the previous release");

    let stored: Vec<(String, String)> =
        sqlx::query_as("SELECT id, kind FROM llm_requests ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        vec![
            ("pre-archivist".to_string(), "archivist".to_string()),
            ("pre-chat".to_string(), "chat".to_string()),
            ("pre-collapse".to_string(), "collapse".to_string()),
            ("pre-consolidator".to_string(), "consolidator".to_string()),
            ("pre-router".to_string(), "router".to_string()),
            ("pre-timeline".to_string(), "timeline".to_string()),
        ],
        "every previous row must survive the rebuild with its kind intact"
    );
}

/// Scenario: re-executing the migration body is a no-op — no error, no
/// duplicated rows, and the widened CHECK stays.
#[tokio::test]
async fn test_llm_requests_migration_rebuild_is_idempotent() {
    let pool = setup().await;

    insert_request(&pool, "idem-1", "timeline").await;

    let sql = journal_worker_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("re-executing the migration body must succeed");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "no row must be duplicated by a second rebuild");

    // The widened CHECK survived the second run of the body.
    insert_request(&pool, "idem-2", "journal").await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 2,
        "the journal row must insert after the second rebuild"
    );
}

// ── 7.2: the seeded settings ────────────────────────────────────────────────

/// Scenario: on a freshly migrated database the three journal settings exist:
/// `journal_prompt` non-empty, `journal_enabled` = `'true'` and
/// `journal_hour` = `'4'`.
#[tokio::test]
async fn test_journal_settings_seeded_after_migrations() {
    let pool = setup().await;

    let prompt = setting_value(&pool, "journal_prompt").await;
    assert!(
        !prompt.trim().is_empty(),
        "journal_prompt must be seeded with a non-empty value"
    );
    assert_eq!(
        setting_value(&pool, "journal_enabled").await,
        "true",
        "the journal must boot enabled"
    );
    assert_eq!(
        setting_value(&pool, "journal_hour").await,
        "4",
        "the default cut hour must be 4"
    );
}

/// Scenario: a missing key is filled by the seed.
#[tokio::test]
async fn test_journal_seed_fills_a_missing_key() {
    let pool = pool_before_journal_worker().await;

    assert!(
        setting_value_opt(&pool, "journal_prompt").await.is_none(),
        "the previous release must not carry journal_prompt"
    );

    let sql = journal_worker_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the migration must apply on top of the previous release");

    let prompt = setting_value(&pool, "journal_prompt").await;
    assert!(
        !prompt.trim().is_empty(),
        "an absent key must be filled by the seed"
    );
    assert_eq!(setting_value(&pool, "journal_enabled").await, "true");
    assert_eq!(setting_value(&pool, "journal_hour").await, "4");
}

/// Scenario: personalisation is never overwritten — a present
/// `journal_prompt`, a disabled journal and a custom hour survive the seed.
#[tokio::test]
async fn test_journal_seed_respects_custom_values() {
    let pool = pool_before_journal_worker().await;

    for (key, value) in [
        ("journal_prompt", "Mi prompt personalizado"),
        ("journal_enabled", "false"),
        ("journal_hour", "23"),
    ] {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
        )
        .bind(key)
        .bind(value)
        .execute(&pool)
        .await
        .expect("seeding a custom setting must succeed");
    }

    let sql = journal_worker_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the seed must apply without error");

    assert_eq!(
        setting_value(&pool, "journal_prompt").await,
        "Mi prompt personalizado",
        "a personalised prompt must never be overwritten"
    );
    assert_eq!(
        setting_value(&pool, "journal_enabled").await,
        "false",
        "a user-disabled journal must stay disabled"
    );
    assert_eq!(
        setting_value(&pool, "journal_hour").await,
        "23",
        "a custom cut hour must be respected"
    );
}

/// Scenario: the progress mark is NOT seeded — its absence is the «never
/// judged» state the worker boots from.
///
/// This is a zero-observation guard: absence is the state *before* the
/// migration too, so the test only starts failing if a seed is ever added.
#[tokio::test]
async fn test_journal_progress_mark_is_not_seeded() {
    let pool = setup().await;

    assert_eq!(
        setting_value_opt(&pool, "journal_last_judged_day").await,
        None,
        "journal_last_judged_day must NOT be seeded by the migration"
    );
}

// ── 3.1–3.3 (reordenadas al grupo 8): el texto sembrado del prompt ──────────
//
// Las tareas 3.1–3.3 prueban el texto **sembrado** de `journal_prompt`, que
// nace aquí, en la migración (grupo 7/8): por eso se escriben y verifican en
// verde junto a la siembra, no en el RED original del grupo 3.

/// Scenario: el prompt sembrado pide la lectura del día (cómo fue, qué costó,
/// qué ilusionó) y el relato en primera persona.
#[tokio::test]
async fn test_seeded_journal_prompt_reads_the_day_in_first_person() {
    let pool = setup().await;

    let prompt = setting_value(&pool, "journal_prompt").await;
    for needle in ["primera persona", "cómo fue", "costó", "ilusionó"] {
        assert!(
            prompt.contains(needle),
            "el prompt sembrado debe pedir la lectura del día; le falta «{needle}»"
        );
    }
}

/// Scenario: el prompt sembrado prohíbe el recuento de los hechos ya
/// registrados y pide describir en vez de diagnosticar.
#[tokio::test]
async fn test_seeded_journal_prompt_forbids_recounting_facts() {
    let pool = setup().await;

    let prompt = setting_value(&pool, "journal_prompt").await;
    for needle in [
        "no los enumeres",
        "descríbelos",
        "Describe en vez de diagnosticar",
    ] {
        assert!(
            prompt.contains(needle),
            "el prompt sembrado debe prohibir el recuento; le falta «{needle}»"
        );
    }
}

/// Scenario: el prompt sembrado define el contrato de salida: el objeto JSON
/// con `write`, el relato y los campos opcionales, y el `{ "write": false }`.
#[tokio::test]
async fn test_seeded_journal_prompt_declares_the_output_contract() {
    let pool = setup().await;

    let prompt = setting_value(&pool, "journal_prompt").await;
    for needle in ["\"write\"", "\"content\"", "\"mood\"", "\"write\": false"] {
        assert!(
            prompt.contains(needle),
            "el prompt sembrado debe definir el contrato de salida; le falta {needle}"
        );
    }
}
