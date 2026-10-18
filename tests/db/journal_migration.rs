//! Schema tests for the `journal-entries` change:
//! `migrations/20261018000001_journal_entries.sql`.
//!
//! The migration is additive and idempotent: it creates `journal_entries` with
//! `UNIQUE(profile_id, date)` — one entry per profile and day — plus the
//! `(profile_id, date DESC)` index that serves the listing. It adds no column to
//! any existing table and there is no `CHECK` on `mood`: the closed set lives in
//! the model (`valet::models::journal::Mood`), covered by `journal_repo.rs`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// The ten columns the migration must declare, in `pragma_table_info` order.
const EXPECTED_COLUMNS: [&str; 10] = [
    "id",
    "profile_id",
    "date",
    "mood",
    "content",
    "achievements",
    "struggles",
    "tags",
    "created_at",
    "updated_at",
];

async fn setup() -> SqlitePool {
    let pool = in_memory_pool().await;
    valet::db::schema::run_migrations(&pool).await.unwrap();
    pool
}

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

/// A struct mirroring one row of `pragma_table_info`.
#[derive(sqlx::FromRow)]
struct ColumnInfo {
    name: String,
    r#type: String,
    notnull: bool,
    dflt_value: Option<String>,
    pk: i64,
}

/// The columns of `journal_entries`, in `pragma_table_info` order — that is, in
/// the order the table declares them.
async fn columns(pool: &SqlitePool) -> Vec<ColumnInfo> {
    sqlx::query_as("SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('journal_entries')")
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("'journal_entries' must exist to be inspected: {e}"))
}

/// The description of one column, or a panic naming the missing one.
fn column<'a>(columns: &'a [ColumnInfo], name: &str) -> &'a ColumnInfo {
    columns
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("column '{name}' must exist"))
}

/// Every explicit index declared on `journal_entries` — `sql` is NULL for the
/// implicit indexes SQLite creates for `UNIQUE`/`PRIMARY KEY`.
async fn index_sqls(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master \
         WHERE type='index' AND tbl_name='journal_entries' AND sql IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// `true` when one of the declarations is an index over `(profile_id, date
/// DESC)`, compared ignoring case and whitespace.
fn has_profile_date_desc_index(sqls: &[String]) -> bool {
    sqls.iter().any(|sql| {
        let normalized = sql.to_lowercase().replace(' ', "");
        normalized.contains("journal_entries(profile_id,datedesc)")
    })
}

/// The `sql` of the table itself, as the schema holds it.
async fn table_sql(pool: &SqlitePool) -> String {
    sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='journal_entries'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("'journal_entries' must exist: {e}"))
}

/// A bare profile, so `journal_entries.profile_id` has a row to point at.
async fn insert_profile(pool: &SqlitePool, id: &str) {
    sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES (?1, 'Test', '{}')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seeding profile '{id}' must succeed: {e}"));
}

/// Insert one row with only the columns the table forces, and `mood` NULL: the
/// point of these tests is `(profile_id, date)`, not the payload.
async fn insert_entry(
    pool: &SqlitePool,
    id: &str,
    profile_id: &str,
    date: &str,
) -> Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "INSERT INTO journal_entries (id, profile_id, date, content) VALUES (?1, ?2, ?3, 'relato')",
    )
    .bind(id)
    .bind(profile_id)
    .bind(date)
    .execute(pool)
    .await
}

// ── 1.1: the table, its columns and its listing index ───────────────────────

/// Scenario: la tabla se crea con sus columnas.
#[tokio::test]
async fn test_journal_entries_table_exists_with_columns() {
    let pool = setup().await;

    let has_table: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='journal_entries'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        has_table,
        "'journal_entries' must exist after run_migrations()"
    );

    let columns = columns(&pool).await;
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        EXPECTED_COLUMNS.to_vec(),
        "'journal_entries' must carry exactly the declared columns, in the declared order; got {names:?}"
    );

    // `id` is the row's identity, so what is asserted is that it *is* the
    // primary key — through the pragma flag and the declaration — and not a
    // `notnull` flag: SQLite does not report a plain `TEXT PRIMARY KEY` as
    // `NOT NULL` (verified against this very table: `pk = 1`, `notnull = 0`),
    // only an explicit `NOT NULL` would set it. The table follows the
    // repository's spelling (`id TEXT PRIMARY KEY`, as in `notes`,
    // `timeline_events` and the other seven tables), and the spec asks for
    // «clave primaria» here while spelling `NOT NULL` out for every other
    // column — so the flag is the engine's quirk, not a missing constraint.
    let id = column(&columns, "id");
    assert_eq!(id.r#type.to_uppercase(), "TEXT");
    assert_eq!(id.pk, 1, "'id' must be the PRIMARY KEY");
    assert!(
        table_sql(&pool).await.contains("id TEXT PRIMARY KEY"),
        "the declaration must spell the primary key out"
    );

    for required in ["profile_id", "date", "content"] {
        let column = column(&columns, required);
        assert_eq!(column.r#type.to_uppercase(), "TEXT");
        assert!(column.notnull, "'{required}' must be NOT NULL");
    }

    for nullable in ["mood", "achievements", "struggles", "tags"] {
        let column = column(&columns, nullable);
        assert_eq!(column.r#type.to_uppercase(), "TEXT");
        assert!(
            !column.notnull,
            "'{nullable}' must admit NULL (a day without mood, or without the field)"
        );
    }

    for stamp in ["created_at", "updated_at"] {
        let column = column(&columns, stamp);
        assert_eq!(column.r#type.to_uppercase(), "TEXT");
        assert!(column.notnull, "'{stamp}' must be NOT NULL");
        assert_eq!(
            column.dflt_value.as_deref(),
            Some("datetime('now')"),
            "'{stamp}' must default to datetime('now')"
        );
    }
}

/// Scenario: el índice sirve el listado por perfil y rango de fechas.
#[tokio::test]
async fn test_journal_entries_has_a_profile_date_desc_index() {
    let pool = setup().await;

    let sqls = index_sqls(&pool).await;
    assert!(
        has_profile_date_desc_index(&sqls),
        "an index over (profile_id, date DESC) must exist on 'journal_entries'; got {sqls:?}"
    );
}

// ── 1.2: one entry per profile and day ─────────────────────────────────────

/// Scenario: la unicidad por perfil y día rechaza la segunda entrada.
#[tokio::test]
async fn test_second_entry_for_the_same_profile_and_day_is_rejected() {
    let pool = setup().await;
    insert_profile(&pool, "p1").await;

    insert_entry(&pool, "j1", "p1", "2026-10-18")
        .await
        .expect("the first entry of the day must insert cleanly");

    let second = insert_entry(&pool, "j2", "p1", "2026-10-18").await;
    assert!(
        second.is_err(),
        "a second row with the same (profile_id, date) must be rejected by UNIQUE"
    );

    let stored: (String, String) =
        sqlx::query_as("SELECT id, content FROM journal_entries WHERE profile_id = 'p1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        ("j1".to_string(), "relato".to_string()),
        "the first row must survive the rejected insert untouched"
    );
}

/// Scenario: el mismo día de perfiles distintos son entradas distintas.
#[tokio::test]
async fn test_the_same_day_of_two_profiles_coexists() {
    let pool = setup().await;
    insert_profile(&pool, "p1").await;
    insert_profile(&pool, "p2").await;

    insert_entry(&pool, "j1", "p1", "2026-10-18")
        .await
        .expect("profile p1's day must insert cleanly");
    insert_entry(&pool, "j2", "p2", "2026-10-18")
        .await
        .expect("the same day of another profile is another entry");

    let dates: Vec<(String, String)> =
        sqlx::query_as("SELECT profile_id, date FROM journal_entries ORDER BY profile_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        dates,
        vec![
            ("p1".to_string(), "2026-10-18".to_string()),
            ("p2".to_string(), "2026-10-18".to_string()),
        ],
        "both rows must coexist"
    );
}

/// Scenario: la migración es idempotente.
#[tokio::test]
async fn test_journal_entries_migration_is_idempotent() {
    let pool = setup().await;

    let table_before = table_sql(&pool).await;
    let indexes_before = index_sqls(&pool).await;

    valet::db::schema::run_migrations(&pool)
        .await
        .expect("re-running the migrations must not fail");

    let tables: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'journal_entries'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(tables, 1, "the table must exist exactly once");
    assert_eq!(
        table_sql(&pool).await,
        table_before,
        "the table schema must be the same after re-running the migration"
    );
    assert_eq!(
        index_sqls(&pool).await,
        indexes_before,
        "the indexes must be the same after re-running the migration"
    );
}

/// The state shipped by the previous release: every migration **older** than
/// `20261018000001` applied, so `journal_entries` does not exist yet.
async fn pool_before_journal_entries() -> SqlitePool {
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
        if migration.version < JOURNAL_ENTRIES_VERSION {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration.sql.as_ref()))
                .execute(&pool)
                .await
                .unwrap_or_else(|e| panic!("applying migration {} failed: {e}", migration.version));
        }
    }
    pool
}

/// The migration is additive: it must not touch any pre-existing table.
#[tokio::test]
async fn test_journal_entries_migration_adds_no_column_to_other_tables() {
    let pool = pool_before_journal_entries().await;

    let messages_before: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
            .fetch_all(&pool)
            .await
            .unwrap();

    let sql = journal_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("the journal migration must apply on top of the previous release");

    let messages_after: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        messages_after, messages_before,
        "an existing table must not gain or lose a column"
    );
}

/// Version of `migrations/20261018000001_journal_entries.sql`.
const JOURNAL_ENTRIES_VERSION: i64 = 20261018000001;

/// Read the journal migration SQL from disk.
fn journal_migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261018000001_journal_entries.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `journal-entries` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// The migration body is re-runnable as is: no `ALTER TABLE`, and both the table
/// and the index are declared with `IF NOT EXISTS`.
#[tokio::test]
async fn test_journal_entries_migration_body_is_re_runnable() {
    let pool = setup().await;

    let sql = journal_migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&pool)
        .await
        .expect("re-executing the migration body must succeed");

    let tables: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'journal_entries'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(tables, 1, "the table must still exist exactly once");
}
