//! Model and repository tests for the `journal-entries` change:
//! `valet::models::journal::{JournalEntry, Mood}` and
//! `valet::db::repos::journal::JournalRepo`, over the `journal_entries` table
//! created by `migrations/20261018000001_journal_entries.sql`.
//!
//! The public key of the journal is `(profile_id, date)` — one entry per profile
//! and day — so the repository is exercised through it, not through the internal
//! `id`.

use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use valet::db::repos::journal::JournalRepo;
use valet::models::journal::{JournalEntry, Mood};

const PROFILE: &str = "p1";
const DAY: &str = "2026-10-18";

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
    sqlx::query("INSERT INTO profiles (id, name, preferences) VALUES ('p2', 'Otro', '{}')")
        .execute(&pool)
        .await
        .unwrap();
    pool
}

/// An entry to write: the `created_at`/`updated_at` it carries are deliberately
/// blank — the database assigns them — and everything else is what the caller
/// wants stored.
fn sample(id: &str, date: &str, content: &str) -> JournalEntry {
    JournalEntry {
        id: id.into(),
        profile_id: PROFILE.into(),
        date: date.into(),
        mood: None,
        content: content.into(),
        achievements: Vec::new(),
        struggles: Vec::new(),
        tags: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

/// How many rows that profile holds for that day.
async fn rows_for(pool: &SqlitePool, profile_id: &str, date: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM journal_entries WHERE profile_id = ?1 AND date = ?2")
        .bind(profile_id)
        .bind(date)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Insert a row behind the repository's back, so `NULL` columns can be tested.
async fn insert_raw(pool: &SqlitePool, id: &str, date: &str, mood: Option<&str>) {
    sqlx::query(
        "INSERT INTO journal_entries (id, profile_id, date, mood, content, achievements, struggles, tags) \
         VALUES (?1, ?2, ?3, ?4, 'relato', NULL, NULL, NULL)",
    )
    .bind(id)
    .bind(PROFILE)
    .bind(date)
    .bind(mood)
    .execute(pool)
    .await
    .expect("the raw insert must succeed");
}

// ── 1.3: the model ─────────────────────────────────────────────────────────

/// Scenario: el ánimo se serializa en snake_case.
#[test]
fn test_mood_serializes_to_snake_case() {
    let cases = [
        (Mood::Great, "great"),
        (Mood::Good, "good"),
        (Mood::Neutral, "neutral"),
        (Mood::Bad, "bad"),
        (Mood::Terrible, "terrible"),
    ];

    for (mood, expected) in cases {
        assert_eq!(
            serde_json::to_value(mood).unwrap(),
            json!(expected),
            "{mood:?} must serialize as {expected:?}"
        );
    }
}

/// Scenario: un ánimo fuera del conjunto no es válido.
#[test]
fn test_mood_outside_the_set_is_not_valid() {
    assert!(
        serde_json::from_value::<Mood>(json!("genial")).is_err(),
        "'genial' is not one of the five moods and must not deserialize"
    );
    assert!(
        serde_json::from_value::<Mood>(json!("Great")).is_err(),
        "the encoding is snake_case: 'Great' must not deserialize"
    );
}

/// Scenario: una entrada con un ánimo fuera del conjunto no deserializa — no se
/// sustituye por ningún ánimo por defecto.
#[test]
fn test_entry_with_a_mood_outside_the_set_does_not_deserialize() {
    let entry = json!({
        "id": "j1",
        "profile_id": PROFILE,
        "date": DAY,
        "mood": "genial",
        "content": "relato",
        "achievements": [],
        "struggles": [],
        "tags": [],
        "created_at": "2026-10-18T20:00:00Z",
        "updated_at": "2026-10-18T20:00:00Z",
    });

    assert!(
        serde_json::from_value::<JournalEntry>(entry).is_err(),
        "a mood outside the closed set must be a deserialization error, not a default"
    );
}

/// Scenario: una entrada sin ánimo es válida.
#[test]
fn test_entry_without_mood_is_valid() {
    let base = json!({
        "id": "j1",
        "profile_id": PROFILE,
        "date": DAY,
        "content": "relato",
        "achievements": [],
        "struggles": [],
        "tags": [],
        "created_at": "2026-10-18T20:00:00Z",
        "updated_at": "2026-10-18T20:00:00Z",
    });

    let with_null = serde_json::from_value::<JournalEntry>({
        let mut value = base.clone();
        value["mood"] = json!(null);
        value
    })
    .expect("a NULL mood must deserialize");
    assert_eq!(with_null.mood, None);

    let without_the_key =
        serde_json::from_value::<JournalEntry>(base).expect("an absent mood must deserialize too");
    assert_eq!(without_the_key.mood, None);
}

/// Scenario: los campos JSON nulos se leen como listas vacías, y el ánimo NULL
/// como `None`, sin perder el resto de la entrada.
#[tokio::test]
async fn test_null_columns_read_as_empty_lists_and_no_mood() {
    let pool = setup().await;
    insert_raw(&pool, "j1", DAY, None).await;

    let entry = JournalRepo::get(&pool, PROFILE, DAY)
        .await
        .unwrap()
        .expect("the raw row must be readable");

    assert_eq!(entry.mood, None, "a NULL mood must read as None");
    assert_eq!(entry.achievements, Vec::<String>::new());
    assert_eq!(entry.struggles, Vec::<String>::new());
    assert_eq!(entry.tags, Vec::<String>::new());
    assert_eq!(
        entry.content, "relato",
        "the rest of the entry must be intact"
    );
    assert_eq!(entry.id, "j1");
    assert_eq!(entry.profile_id, PROFILE);
    assert_eq!(entry.date, DAY);
    assert!(!entry.created_at.is_empty(), "created_at must be stored");
}

/// A mood that is not one of the five is a loud error, never a silent fallback:
/// the table has no `CHECK`, so a hand-written row could carry one.
#[tokio::test]
async fn test_a_mood_outside_the_set_is_not_swallowed_when_reading() {
    let pool = setup().await;
    insert_raw(&pool, "j1", DAY, Some("genial")).await;

    let read = JournalRepo::get(&pool, PROFILE, DAY).await;
    assert!(
        read.is_err(),
        "reading a mood outside the closed set must fail, got {read:?}"
    );
}

// ── 1.4: the repository ────────────────────────────────────────────────────

/// Scenario: `upsert` crea la entrada del día.
#[tokio::test]
async fn test_upsert_creates_the_day_entry() {
    let pool = setup().await;

    let mut to_write = sample("j1", DAY, "un día tranquilo");
    to_write.mood = Some(Mood::Good);
    to_write.achievements = vec!["terminé el PR".into()];
    to_write.struggles = vec!["dormí poco".into()];
    to_write.tags = vec!["trabajo".into()];

    let stored = JournalRepo::upsert(&pool, &to_write).await.unwrap();

    assert_eq!(rows_for(&pool, PROFILE, DAY).await, 1, "one row must exist");
    assert_eq!(stored.content, "un día tranquilo");
    assert_eq!(stored.mood, Some(Mood::Good));
    assert_eq!(stored.achievements, vec!["terminé el PR".to_string()]);
    assert_eq!(stored.struggles, vec!["dormí poco".to_string()]);
    assert_eq!(stored.tags, vec!["trabajo".to_string()]);
    assert_eq!(stored.id, "j1");
    assert_eq!(stored.profile_id, PROFILE);
    assert_eq!(stored.date, DAY);
    assert!(
        !stored.created_at.is_empty() && !stored.updated_at.is_empty(),
        "the database must assign both timestamps, got {stored:?}"
    );
}

/// Scenario: `upsert` reemplaza la entrada del mismo día sin duplicar.
#[tokio::test]
async fn test_upsert_replaces_the_day_without_duplicating() {
    let pool = setup().await;

    let first = JournalRepo::upsert(&pool, &sample("j1", DAY, "un relato"))
        .await
        .unwrap();
    let second = JournalRepo::upsert(&pool, &sample("j2", DAY, "otro relato"))
        .await
        .unwrap();

    assert_eq!(
        rows_for(&pool, PROFILE, DAY).await,
        1,
        "the second write of the day must replace the row, not append another one"
    );

    let stored = JournalRepo::get(&pool, PROFILE, DAY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.content, "otro relato");
    assert_eq!(
        second.content, "otro relato",
        "the returned entry is the stored one"
    );
    assert_eq!(
        stored.id, first.id,
        "the replaced row keeps its internal id: it is the same row"
    );
}

/// Scenario: el reemplazo actualiza `updated_at` y conserva `created_at`.
#[tokio::test]
async fn test_upsert_keeps_created_at_and_bumps_updated_at() {
    let pool = setup().await;

    JournalRepo::upsert(&pool, &sample("j1", DAY, "un relato"))
        .await
        .unwrap();

    // Backdate both stamps: `datetime('now')` has second granularity, so two
    // writes inside the same second would otherwise be indistinguishable.
    let backdated = "2020-01-01 00:00:00";
    sqlx::query("UPDATE journal_entries SET created_at = ?1, updated_at = ?1")
        .bind(backdated)
        .execute(&pool)
        .await
        .unwrap();

    let stored = JournalRepo::upsert(&pool, &sample("j1", DAY, "otro relato"))
        .await
        .unwrap();

    assert_eq!(
        stored.created_at, backdated,
        "created_at must be preserved across a replacement"
    );
    assert_ne!(
        stored.updated_at, backdated,
        "updated_at must move on a replacement"
    );
    assert!(
        stored.updated_at > stored.created_at,
        "updated_at must be later than created_at, got {stored:?}"
    );
}

/// Scenario: `get` devuelve la entrada del día o nada.
#[tokio::test]
async fn test_get_returns_the_day_entry_or_none() {
    let pool = setup().await;

    JournalRepo::upsert(&pool, &sample("j1", DAY, "un relato"))
        .await
        .unwrap();

    let found = JournalRepo::get(&pool, PROFILE, DAY).await.unwrap();
    let found = found.expect("the entry of the day must be returned");
    assert_eq!(found.content, "un relato");

    let missing = JournalRepo::get(&pool, PROFILE, "2026-10-19")
        .await
        .unwrap();
    assert!(missing.is_none(), "a day without an entry must yield None");

    let other_profile = JournalRepo::get(&pool, "p2", DAY).await.unwrap();
    assert!(
        other_profile.is_none(),
        "another profile's day must not be returned"
    );
}

/// Scenario: `list` devuelve el rango de más reciente a más antiguo.
#[tokio::test]
async fn test_list_returns_the_range_most_recent_first() {
    let pool = setup().await;

    for (id, date) in [
        ("j15", "2026-10-15"),
        ("j16", "2026-10-16"),
        ("j20", "2026-10-20"),
    ] {
        JournalRepo::upsert(&pool, &sample(id, date, "relato"))
            .await
            .unwrap();
    }
    // Another profile's entry inside the same range must not leak in.
    let mut other = sample("j-other", "2026-10-16", "relato ajeno");
    other.profile_id = "p2".into();
    JournalRepo::upsert(&pool, &other).await.unwrap();

    let listed = JournalRepo::list(&pool, PROFILE, "2026-10-15", "2026-10-16", 10)
        .await
        .unwrap();
    let dates: Vec<&str> = listed.iter().map(|e| e.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2026-10-16", "2026-10-15"],
        "both ends of the range are inclusive and the newest comes first"
    );
}

/// Scenario: `list` respeta el tope quedándose con lo más reciente.
#[tokio::test]
async fn test_list_honours_the_limit_keeping_the_most_recent() {
    let pool = setup().await;

    for (id, date) in [
        ("j14", "2026-10-14"),
        ("j15", "2026-10-15"),
        ("j16", "2026-10-16"),
        ("j17", "2026-10-17"),
    ] {
        JournalRepo::upsert(&pool, &sample(id, date, "relato"))
            .await
            .unwrap();
    }

    let listed = JournalRepo::list(&pool, PROFILE, "2026-10-14", "2026-10-17", 2)
        .await
        .unwrap();
    let dates: Vec<&str> = listed.iter().map(|e| e.date.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2026-10-17", "2026-10-16"],
        "at most `limit` entries, and the newest ones"
    );
}

/// Scenario: `delete` borra la del día y avisa de lo que no existía.
#[tokio::test]
async fn test_delete_removes_the_day_and_reports_the_absent_one() {
    let pool = setup().await;

    JournalRepo::upsert(&pool, &sample("j1", DAY, "un relato"))
        .await
        .unwrap();

    assert!(
        JournalRepo::delete(&pool, PROFILE, DAY).await.unwrap(),
        "deleting the day that exists must report true"
    );
    assert_eq!(
        rows_for(&pool, PROFILE, DAY).await,
        0,
        "the row must be gone"
    );

    assert!(
        !JournalRepo::delete(&pool, PROFILE, "2026-10-19")
            .await
            .unwrap(),
        "deleting a day that never existed must report false"
    );
}
