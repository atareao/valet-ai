//! Schema + repo tests for the `activity-timeline` change:
//! `timeline_events` (migration `20261010000003_timeline_events.sql`) and the
//! widened `llm_requests.kind` `CHECK`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use valet::db::repos::timeline::TimelineRepo;
use valet::models::timeline::{normalize_category, NewTimelineEvent, DEFAULT_TIMELINE_CATEGORY};

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

/// Read the timeline-events migration SQL from disk.
fn timeline_events_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261010000003_timeline_events.sql");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
}

fn sample(id: &str, timestamp: &str, category: &str, fact: &str) -> NewTimelineEvent {
    NewTimelineEvent {
        id: id.into(),
        timestamp: timestamp.into(),
        category: category.into(),
        fact: fact.into(),
        source_message_id: None,
        location: None,
    }
}

fn sample_with_location(
    id: &str,
    timestamp: &str,
    category: &str,
    fact: &str,
    location: &str,
) -> NewTimelineEvent {
    NewTimelineEvent {
        id: id.into(),
        timestamp: timestamp.into(),
        category: category.into(),
        fact: fact.into(),
        source_message_id: None,
        location: Some(location.into()),
    }
}

// ── 1.1: the `timeline_events` table, its CHECK and its three indexes ───────

#[tokio::test]
async fn test_timeline_events_table_exists_with_columns() {
    let pool = setup().await;

    let has_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='timeline_events'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(has_table, "'timeline_events' must exist after migration");

    #[derive(sqlx::FromRow)]
    struct ColumnInfo {
        name: String,
        r#type: String,
        notnull: bool,
        dflt_value: Option<String>,
        pk: bool,
    }

    let columns: Vec<ColumnInfo> =
        sqlx::query_as("SELECT * FROM pragma_table_info('timeline_events')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let col_map: std::collections::HashMap<&str, &ColumnInfo> =
        columns.iter().map(|c| (c.name.as_str(), c)).collect();

    let id = col_map.get("id").expect("column 'id' missing");
    assert_eq!(id.r#type.to_uppercase(), "TEXT");
    assert!(id.pk, "'id' must be PRIMARY KEY");

    let timestamp = col_map
        .get("timestamp")
        .expect("column 'timestamp' missing");
    assert_eq!(timestamp.r#type.to_uppercase(), "TEXT");
    assert!(timestamp.notnull, "'timestamp' must be NOT NULL");

    let category = col_map.get("category").expect("column 'category' missing");
    assert_eq!(category.r#type.to_uppercase(), "TEXT");
    assert!(category.notnull, "'category' must be NOT NULL");
    assert_eq!(
        category.dflt_value.as_deref(),
        Some("'lifestyle'"),
        "'category' default must be 'lifestyle'"
    );

    let fact = col_map.get("fact").expect("column 'fact' missing");
    assert_eq!(fact.r#type.to_uppercase(), "TEXT");
    assert!(fact.notnull, "'fact' must be NOT NULL");

    let source = col_map
        .get("source_message_id")
        .expect("column 'source_message_id' missing");
    assert!(
        !source.notnull,
        "'source_message_id' must be nullable (ON DELETE SET NULL)"
    );

    let created_at = col_map
        .get("created_at")
        .expect("column 'created_at' missing");
    assert_eq!(created_at.r#type.to_uppercase(), "TEXT");
    assert!(created_at.notnull, "'created_at' must be NOT NULL");
    assert_eq!(
        created_at.dflt_value.as_deref(),
        Some("datetime('now')"),
        "'created_at' must default to datetime('now')"
    );
}

#[tokio::test]
async fn test_timeline_events_category_check_roundtrip() {
    let pool = setup().await;

    // Every declared category is accepted.
    //
    // Each row carries a distinct `fact`: the unique index over
    // `(lower(trim(fact)), substr(timestamp,1,10))` makes the same fact on the
    // same day a duplicate, which is orthogonal to the category CHECK under test.
    for (i, category) in [
        "sport",
        "lifestyle",
        "work",
        "shopping",
        "health",
        "social",
        "system",
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query(
            "INSERT INTO timeline_events (id, timestamp, category, fact) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(format!("ev-{i}"))
        .bind("2026-10-10T08:00:00Z")
        .bind(category)
        .bind(format!("hecho {i}"))
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("category '{category}' must be accepted: {e}"));
    }

    // A value outside the closed set is rejected by the CHECK.
    let invalid = sqlx::query(
        "INSERT INTO timeline_events (id, timestamp, category, fact) VALUES (?1, ?2, 'nope', ?3)",
    )
    .bind("ev-bad")
    .bind("2026-10-10T08:00:00Z")
    .bind("hecho")
    .execute(&pool)
    .await;
    assert!(
        invalid.is_err(),
        "category 'nope' must be rejected by the CHECK constraint"
    );
}

#[tokio::test]
async fn test_timeline_events_indexes_exist() {
    let pool = setup().await;

    for name in [
        "idx_timeline_events_timestamp",
        "idx_timeline_events_category",
        "idx_timeline_events_source_message",
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name = ?1",
        )
        .bind(name)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "index '{name}' must exist");
    }
}

#[tokio::test]
async fn test_timeline_events_migration_is_idempotent() {
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

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'timeline_events'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "the table must exist exactly once");
}

// ── 1.2: widened `llm_requests.kind` CHECK without losing rows ──────────────

#[tokio::test]
async fn test_llm_requests_accepts_timeline_kind() {
    let pool = setup().await;

    sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', 'timeline')")
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .expect("kind='timeline' must be accepted");

    // The five previous origins are still accepted.
    for kind in ["chat", "router", "archivist", "consolidator", "collapse"] {
        sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', ?2)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(kind)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("kind='{kind}' must still be accepted: {e}"));
    }

    // A bogus origin is rejected.
    let invalid =
        sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', 'nope')")
            .bind(uuid::Uuid::new_v4().to_string())
            .execute(&pool)
            .await;
    assert!(invalid.is_err(), "kind='nope' must be rejected");

    // The table SQL advertises the new origin.
    let table_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='llm_requests'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        table_sql.contains("'timeline'"),
        "the CHECK must mention 'timeline'; table SQL: {table_sql}"
    );
}

#[tokio::test]
async fn test_llm_requests_rebuild_preserves_rows() {
    let pool = setup().await;

    // Insert a row of each historical origin.
    for kind in ["chat", "router", "archivist", "consolidator", "collapse"] {
        sqlx::query("INSERT INTO llm_requests (id, model, kind) VALUES (?1, 'gpt-4o', ?2)")
            .bind(format!("req-{kind}"))
            .bind(kind)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Re-apply the rebuild body on top of the already-migrated table: every row
    // must survive the `DROP`/`RENAME` round-trip.
    let sql = timeline_events_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 5, "no row may be lost by the rebuild");

    for kind in ["chat", "router", "archivist", "consolidator", "collapse"] {
        let stored: String = sqlx::query_scalar("SELECT kind FROM llm_requests WHERE id = ?1")
            .bind(format!("req-{kind}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, kind, "kind must be preserved across the rebuild");
    }
}

#[tokio::test]
async fn test_llm_requests_kind_migration_is_idempotent() {
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

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_requests")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "the table must be empty and readable after idempotency"
    );
}

// ── Repo: insert_many shares the caller's executor ──────────────────────────

#[tokio::test]
async fn test_insert_many_commits_within_shared_transaction() {
    let pool = setup().await;

    let mut tx = pool.begin().await.unwrap();
    TimelineRepo::insert_many(
        &mut *tx,
        &[
            sample("e1", "2026-10-10T08:00:00Z", "sport", "Fui a correr"),
            sample("e2", "2026-10-10T09:00:00Z", "work", "Cerré el PR"),
        ],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "both events must be committed");
}

#[tokio::test]
async fn test_insert_many_respects_rollback() {
    let pool = setup().await;

    let mut tx = pool.begin().await.unwrap();
    TimelineRepo::insert_many(
        &mut *tx,
        &[sample(
            "e1",
            "2026-10-10T08:00:00Z",
            "sport",
            "Fui a correr",
        )],
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "a rolled-back batch must leave no rows");
}

#[tokio::test]
async fn test_insert_many_empty_batch_is_a_noop() {
    let pool = setup().await;

    TimelineRepo::insert_many(&pool, &[]).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ── Repo: list ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_orders_most_recent_first() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[
            sample("old", "2026-10-08T08:00:00Z", "work", "antiguo"),
            sample("new", "2026-10-10T08:00:00Z", "work", "reciente"),
            sample("mid", "2026-10-09T08:00:00Z", "work", "medio"),
        ],
    )
    .await
    .unwrap();

    let events = TimelineRepo::list(&pool, None, None, None, 10)
        .await
        .unwrap();
    let ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["new", "mid", "old"]);
}

#[tokio::test]
async fn test_list_range_is_inclusive_on_both_ends() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[
            sample("before", "2026-10-09T23:59:59Z", "work", "antes"),
            sample("at-start", "2026-10-10T00:00:00Z", "work", "inicio"),
            sample("in", "2026-10-10T05:00:00Z", "work", "dentro"),
            sample("at-end", "2026-10-10T10:00:00Z", "work", "fin"),
            sample("after", "2026-10-10T10:00:01Z", "work", "despues"),
        ],
    )
    .await
    .unwrap();

    let events = TimelineRepo::list(
        &pool,
        Some("2026-10-10T00:00:00Z"),
        Some("2026-10-10T10:00:00Z"),
        None,
        10,
    )
    .await
    .unwrap();
    let ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["at-end", "in", "at-start"]);
}

#[tokio::test]
async fn test_list_filters_by_category() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[
            sample("s1", "2026-10-10T08:00:00Z", "sport", "correr"),
            sample("w1", "2026-10-10T09:00:00Z", "work", "programar"),
            sample("s2", "2026-10-10T10:00:00Z", "sport", "nadar"),
        ],
    )
    .await
    .unwrap();

    let events = TimelineRepo::list(&pool, None, None, Some("sport"), 10)
        .await
        .unwrap();
    let ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["s2", "s1"]);
}

#[tokio::test]
async fn test_list_honours_limit() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[
            sample("a", "2026-10-08T08:00:00Z", "work", "a"),
            sample("b", "2026-10-09T08:00:00Z", "work", "b"),
            sample("c", "2026-10-10T08:00:00Z", "work", "c"),
        ],
    )
    .await
    .unwrap();

    let events = TimelineRepo::list(&pool, None, None, None, 2)
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
    let ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["c", "b"]);
}

// ── Repo: delete ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_existing_returns_true_and_removes_row() {
    let pool = setup().await;
    TimelineRepo::insert_many(&pool, &[sample("e1", "2026-10-10T08:00:00Z", "work", "x")])
        .await
        .unwrap();

    assert!(TimelineRepo::delete(&pool, "e1").await.unwrap());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_delete_missing_returns_false() {
    let pool = setup().await;
    assert!(!TimelineRepo::delete(&pool, "does-not-exist").await.unwrap());
}

// ── Foreign key: ON DELETE SET NULL ─────────────────────────────────────────

#[tokio::test]
async fn test_source_message_id_is_nulled_when_message_deleted() {
    let pool = setup().await;

    sqlx::query("INSERT INTO messages (id, role, content) VALUES ('m1', 'user', 'hola')")
        .execute(&pool)
        .await
        .unwrap();

    TimelineRepo::insert_many(
        &pool,
        &[NewTimelineEvent {
            id: "e1".into(),
            timestamp: "2026-10-10T08:00:00Z".into(),
            category: "social".into(),
            fact: "Quedé con Ana".into(),
            location: None,
            source_message_id: Some("m1".into()),
        }],
    )
    .await
    .unwrap();

    sqlx::query("DELETE FROM messages WHERE id = 'm1'")
        .execute(&pool)
        .await
        .unwrap();

    let (id, source): (String, Option<String>) =
        sqlx::query_as("SELECT id, source_message_id FROM timeline_events WHERE id = 'e1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(id, "e1", "the event must survive the message deletion");
    assert!(
        source.is_none(),
        "source_message_id must be nulled when the message is deleted"
    );
}

// ── normalize_category ──────────────────────────────────────────────────────

#[test]
fn test_normalize_category_four_cases() {
    assert_eq!(normalize_category("sport"), "sport");
    assert_eq!(normalize_category("random"), DEFAULT_TIMELINE_CATEGORY);
    assert_eq!(normalize_category(""), DEFAULT_TIMELINE_CATEGORY);
    assert_eq!(normalize_category("SPORT"), DEFAULT_TIMELINE_CATEGORY);
}

// ════════════════════════════════════════════════════════════════════════════
// timeline-fact-integrity (RED): the new migration, its unique index, the
// wipe and the updated seeded prompts. Nothing here passes until the change is
// implemented: `fact_integrity_migration_sql()` reads a file that does not
// exist yet, so every test that touches it fails with the helper's `panic!`.
// ════════════════════════════════════════════════════════════════════════════

/// Version of `migrations/20261011000001_timeline_fact_integrity.sql`.
const FACT_INTEGRITY_VERSION: i64 = 20261011000001;

/// Read the fact-integrity migration SQL from disk.
fn fact_integrity_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261011000001_timeline_fact_integrity.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `timeline-fact-integrity` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left in
/// the state the previous release had: `timeline_events` without `location`, sin
/// índice único y con las filas que le metas. La migración nueva NO se aplica.
async fn pool_before(before: i64) -> SqlitePool {
    // 1) `run_migrations` calls `register_vec_extension()` before anything else,
    //    and `sqlite3_auto_extension` is process-global but only reaches
    //    connections opened *after* it runs. A throwaway pool is enough to
    //    trigger the registration; one of the older migrations creates a `vec0`
    //    virtual table, so the new pool below needs the extension in place.
    let throwaway = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();
    valet::db::schema::run_migrations(&throwaway).await.unwrap();
    drop(throwaway);

    // 2) A brand-new in-memory pool. Its connections are opened after the
    //    registration, so they inherit `vec0`.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

    // 3) Apply, in version order, every migration strictly older than `before`.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrator = sqlx::migrate::Migrator::new(manifest.join("migrations"))
        .await
        .unwrap();
    let mut migrations: Vec<sqlx::migrate::Migration> = migrator.iter().cloned().collect();
    migrations.sort();
    for migration in &migrations {
        if migration.version < before {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "failed to apply migration {} ({}): {e}",
                        migration.version, migration.description
                    )
                });
        }
    }

    pool
}

/// Read a `settings` value, or `None` when the key is absent.
async fn settings_value(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|e| panic!("reading setting '{key}' must not fail: {e}"))
}

/// Insert a fact, letting `category` and `created_at` take their defaults.
async fn insert_fact(
    pool: &SqlitePool,
    id: &str,
    timestamp: &str,
    fact: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO timeline_events (id, timestamp, fact) VALUES (?1, ?2, ?3)")
        .bind(id)
        .bind(timestamp)
        .bind(fact)
        .execute(pool)
        .await
        .map(|_| ())
}

/// The `sql` of every index declared on `timeline_events` (NULL for implicit
/// indexes, which `sqlite_master` does not spell out).
async fn timeline_index_sqls(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master \
         WHERE type='index' AND tbl_name='timeline_events' AND sql IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Whether one of the index declarations is a `UNIQUE` index over
/// `lower(trim(fact))` (whitespace- and case-insensitive match).
fn has_unique_fact_index(sqls: &[String]) -> bool {
    sqls.iter().any(|sql| {
        let normalized = sql.to_lowercase().replace(' ', "");
        normalized.contains("unique") && normalized.contains("lower(trim(fact))")
    })
}

/// The exact rules the new seeded extractor prompt must state, plus the phrase
/// it must no longer contain.
const NEW_PROMPT_RULES: [&str; 6] = [
    "un solo hecho",
    "estados transitorios",
    "órdenes dirigidas al asistente",
    "ISO 8601",
    "location",
    "hechos ya registrados",
];

fn assert_new_prompt_contract(prompt: &str) {
    for needle in NEW_PROMPT_RULES {
        assert!(
            prompt.contains(needle),
            "the seeded prompt must contain {needle:?}; got: {prompt}"
        );
    }
    assert!(
        !prompt.contains("NO devuelvas ninguna fecha"),
        "the seeded prompt must no longer forbid dates; got: {prompt}"
    );
}

// ── 1.1: the `location` column and the unique index (freshly migrated db) ───

#[tokio::test]
async fn test_timeline_events_has_location_column() {
    let pool = setup().await;

    #[derive(sqlx::FromRow)]
    struct ColumnInfo {
        name: String,
        r#type: String,
        notnull: bool,
    }

    let columns: Vec<ColumnInfo> =
        sqlx::query_as("SELECT * FROM pragma_table_info('timeline_events')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let location = columns
        .iter()
        .find(|c| c.name == "location")
        .expect("column 'location' must exist after the migration");

    assert_eq!(location.r#type.to_uppercase(), "TEXT");
    assert!(
        !location.notnull,
        "'location' must be nullable (a fact without a place stores NULL)"
    );
}

#[tokio::test]
async fn test_timeline_events_unique_index_exists() {
    let pool = setup().await;

    // (a) a UNIQUE index over the normalised fact.
    let sqls = timeline_index_sqls(&pool).await;
    assert!(
        has_unique_fact_index(&sqls),
        "a UNIQUE index over lower(trim(fact)) must exist; indexes: {sqls:?}"
    );

    // (b) the three previous indexes survive the new migration.
    for name in [
        "idx_timeline_events_timestamp",
        "idx_timeline_events_category",
        "idx_timeline_events_source_message",
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name = ?1",
        )
        .bind(name)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "index '{name}' must survive the new migration");
    }
}

#[tokio::test]
async fn test_same_fact_same_day_is_rejected() {
    let pool = setup().await;

    insert_fact(&pool, "ev-1", "2026-10-11T20:21:00+00:00", "Cené mochis")
        .await
        .expect("the first row must insert cleanly");

    let second = insert_fact(&pool, "ev-2", "2026-10-11T21:00:00+00:00", "Cené mochis").await;
    assert!(
        second.is_err(),
        "the same fact on the same day must be rejected by the unique index"
    );

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events WHERE fact = 'Cené mochis'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "the first row must survive the rejected insert");
}

#[tokio::test]
async fn test_same_fact_different_day_coexists() {
    let pool = setup().await;

    insert_fact(&pool, "ev-1", "2026-10-11T20:21:00+00:00", "Cené mochis")
        .await
        .expect("the first day must insert cleanly");
    insert_fact(&pool, "ev-2", "2026-10-12T20:21:00+00:00", "Cené mochis")
        .await
        .expect("the same fact on a different day is a different fact and must insert");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "the same fact on two days must coexist");
}

#[tokio::test]
async fn test_normalization_ignores_case_and_spaces() {
    let pool = setup().await;

    insert_fact(&pool, "ev-1", "2026-10-11T10:21:00+00:00", "Corrí 9,34 km")
        .await
        .expect("the first row must insert cleanly");

    let padded = insert_fact(
        &pool,
        "ev-2",
        "2026-10-11T17:23:00+00:00",
        "  corrí 9,34 km  ",
    )
    .await;
    assert!(
        padded.is_err(),
        "'  corrí 9,34 km  ' must collide with 'Corrí 9,34 km' the same day"
    );
}

// ── 1.2: the wipe (pre-migration state) ─────────────────────────────────────

#[tokio::test]
async fn test_migration_wipes_timeline_events() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    // Two exact duplicates (same fact and day, distinct id and created_at) plus
    // one unrelated fact: none may survive.
    for (id, timestamp, created_at) in [
        ("a1", "2026-10-11T10:00:00+00:00", "2026-10-11T10:05:00Z"),
        ("a2", "2026-10-11T10:00:00+00:00", "2026-10-11T10:06:00Z"),
        ("b1", "2026-10-10T20:00:00+00:00", "2026-10-10T20:05:00Z"),
    ] {
        sqlx::query(
            "INSERT INTO timeline_events (id, timestamp, fact, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(id)
        .bind(timestamp)
        .bind("Corrí 9,34 km")
        .bind(created_at)
        .execute(&pool)
        .await
        .unwrap();
    }

    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(before >= 3, "the pre-migration table must hold the rows");

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, 0, "the migration must leave timeline_events empty");
}

#[tokio::test]
async fn test_migration_does_not_touch_messages() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    for (id, content) in [("m1", "hola"), ("m2", "qué tal")] {
        sqlx::query("INSERT INTO messages (id, role, content) VALUES (?1, 'user', ?2)")
            .bind(id)
            .bind(content)
            .execute(&pool)
            .await
            .unwrap();
    }
    insert_fact(&pool, "e1", "2026-10-11T10:00:00+00:00", "Corrí 9,34 km")
        .await
        .unwrap();

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(messages, 2, "messages must keep every row");

    let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(facts, 0, "timeline_events is the only table emptied");
}

#[tokio::test]
async fn test_migration_succeeds_with_prior_duplicates_and_creates_the_index() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    // Duplicates that would make `CREATE UNIQUE INDEX` fail on a non-empty table.
    insert_fact(&pool, "d1", "2026-10-11T10:00:00+00:00", "Corrí 9,34 km")
        .await
        .unwrap();
    insert_fact(
        &pool,
        "d2",
        "2026-10-11T17:00:00+00:00",
        "  corrí 9,34 km  ",
    )
    .await
    .unwrap();

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the migration must wipe before creating the index and not fail");

    let sqls = timeline_index_sqls(&pool).await;
    assert!(
        has_unique_fact_index(&sqls),
        "the unique index must exist after the migration; indexes: {sqls:?}"
    );
}

#[tokio::test]
async fn test_migration_adds_location_to_a_pre_migration_table() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    let before: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('timeline_events')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        !before.contains(&"location".to_string()),
        "the pre-migration table must NOT have 'location'; got: {before:?}"
    );

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let after: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('timeline_events')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        after.contains(&"location".to_string()),
        "the post-migration table must have 'location'; got: {after:?}"
    );
}

// ── 1.3: the seeded prompts (pre-migration state) ───────────────────────────

#[tokio::test]
async fn test_migration_updates_the_untouched_prompt() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    // Read the value seeded by `20261010000004` from the database; never hardcode
    // it, the assertion is that the migration replaces *that* value.
    let seeded = settings_value(&pool, "timeline_prompt")
        .await
        .expect("timeline_prompt must be seeded by 20261010000004");

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let updated = settings_value(&pool, "timeline_prompt")
        .await
        .expect("timeline_prompt must still exist after the migration");
    assert_ne!(updated, seeded, "the untouched prompt must be replaced");
    assert_new_prompt_contract(&updated);
}

#[tokio::test]
async fn test_migration_respects_a_customized_prompt() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    sqlx::query("UPDATE settings SET value = 'MI PROMPT A MEDIDA' WHERE key = 'timeline_prompt'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        settings_value(&pool, "timeline_prompt").await.as_deref(),
        Some("MI PROMPT A MEDIDA"),
        "a customised prompt must be left untouched"
    );
}

#[tokio::test]
async fn test_migration_fills_a_missing_prompt() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    sqlx::query("DELETE FROM settings WHERE key = 'timeline_prompt'")
        .execute(&pool)
        .await
        .unwrap();

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let value = settings_value(&pool, "timeline_prompt")
        .await
        .expect("a missing timeline_prompt must be filled by the migration");
    assert_new_prompt_contract(&value);
}

#[tokio::test]
async fn test_migration_updates_the_skill_prompt() {
    let pool = pool_before(FACT_INTEGRITY_VERSION).await;

    let sql = fact_integrity_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .unwrap();

    let skill = settings_value(&pool, "SKILL_TIMELINE_PROMPT")
        .await
        .expect("SKILL_TIMELINE_PROMPT must exist after the migration");
    assert!(
        skill.contains("location"),
        "the updated skill prompt must mention location; got: {skill}"
    );

    // A customisation is respected exactly like the extractor prompt.
    let custom = pool_before(FACT_INTEGRITY_VERSION).await;
    sqlx::query(
        "UPDATE settings SET value = 'MI SKILL A MEDIDA' WHERE key = 'SKILL_TIMELINE_PROMPT'",
    )
    .execute(&custom)
    .await
    .unwrap();

    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&custom)
        .await
        .unwrap();

    assert_eq!(
        settings_value(&custom, "SKILL_TIMELINE_PROMPT")
            .await
            .as_deref(),
        Some("MI SKILL A MEDIDA"),
        "a customised skill prompt must be left untouched"
    );
}

#[tokio::test]
async fn test_seeded_prompt_states_the_new_rules() {
    let pool = setup().await;

    let value = settings_value(&pool, "timeline_prompt")
        .await
        .expect("timeline_prompt must be seeded in a freshly migrated database");
    assert_new_prompt_contract(&value);
}

// ════════════════════════════════════════════════════════════════════════════
// timeline-fact-integrity (RED): the repository contract. `insert_many` now
// returns how many rows really entered (INSERT OR IGNORE, so a duplicate of the
// unique index is a silent no-op) and `list` carries each event's `location`.
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_insert_many_returns_the_number_of_rows_inserted() {
    let pool = setup().await;

    let inserted = TimelineRepo::insert_many(
        &pool,
        &[
            sample("e1", "2026-10-10T08:00:00Z", "sport", "Fui a correr"),
            sample("e2", "2026-10-10T09:00:00Z", "work", "Cerré el PR"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(inserted, 2, "both distinct rows must be counted");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "both rows must be stored");
}

#[tokio::test]
async fn test_insert_many_ignores_a_duplicate_and_returns_zero() {
    let pool = setup().await;

    let first = TimelineRepo::insert_many(
        &pool,
        &[sample(
            "e1",
            "2026-10-11T20:21:00+00:00",
            "lifestyle",
            "Cené mochis",
        )],
    )
    .await
    .unwrap();
    assert_eq!(first, 1, "the first row must enter");

    let second = TimelineRepo::insert_many(
        &pool,
        &[sample(
            "e2",
            "2026-10-11T21:00:00+00:00",
            "lifestyle",
            "Cené mochis",
        )],
    )
    .await
    .expect("a duplicate of the unique index must be ignored, not an error");
    assert_eq!(second, 0, "the ignored duplicate must not be counted");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "the duplicate must leave a single row");
}

#[tokio::test]
async fn test_insert_many_ignores_duplicates_inside_the_batch() {
    let pool = setup().await;

    let inserted = TimelineRepo::insert_many(
        &pool,
        &[
            sample(
                "e1",
                "2026-10-11T20:21:00+00:00",
                "lifestyle",
                "Cené mochis",
            ),
            sample(
                "e2",
                "2026-10-11T21:00:00+00:00",
                "lifestyle",
                "Cené mochis",
            ),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        inserted, 1,
        "only one of the two identical rows inside the batch may enter"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "the batch must leave a single row");
}

#[tokio::test]
async fn test_insert_many_counts_only_the_rows_that_entered() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[sample(
            "e1",
            "2026-10-11T20:21:00+00:00",
            "lifestyle",
            "Cené mochis",
        )],
    )
    .await
    .unwrap();

    let inserted = TimelineRepo::insert_many(
        &pool,
        &[
            // Repeats the already-stored fact the same day: must be ignored.
            sample(
                "e2",
                "2026-10-11T21:00:00+00:00",
                "lifestyle",
                "Cené mochis",
            ),
            sample("e3", "2026-10-11T08:00:00+00:00", "sport", "Corrí 9,34 km"),
            sample("e4", "2026-10-11T09:00:00+00:00", "work", "Cerré el PR"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(inserted, 2, "only the two brand-new rows may be counted");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM timeline_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3, "the pre-existing row plus the two new ones");
}

#[tokio::test]
async fn test_list_returns_the_location() {
    let pool = setup().await;

    TimelineRepo::insert_many(
        &pool,
        &[
            sample_with_location(
                "with",
                "2026-10-10T08:00:00Z",
                "sport",
                "Paseé en bici",
                "vía verde",
            ),
            sample("without", "2026-10-10T09:00:00Z", "work", "Cerré el PR"),
        ],
    )
    .await
    .unwrap();

    let events = TimelineRepo::list(&pool, None, None, None, 10)
        .await
        .unwrap();

    let with = events
        .iter()
        .find(|e| e.id == "with")
        .expect("the located event must be listed");
    let without = events
        .iter()
        .find(|e| e.id == "without")
        .expect("the unlocated event must be listed");

    assert_eq!(
        with.location.as_deref(),
        Some("vía verde"),
        "the event with a place must return it"
    );
    assert_eq!(
        without.location, None,
        "the event without a place must return NULL"
    );
}
