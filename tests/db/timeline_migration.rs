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
        .bind("hecho")
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
