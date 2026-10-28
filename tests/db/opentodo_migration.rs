//! Migration tests for the `opentodo-integration` change:
//! `migrations/20261027000001_opentodo.sql`.
//!
//! The migration is additive and idempotent, with the same «only overwrite
//! empty/NULL» idiom as the sibling `*_skill`/`*_prompts` migrations. It seeds
//! the OpenTodo connection settings (`opentodo_base_url`, `opentodo_token`) and
//! promotes the `SKILL_PENDIENTES_PROMPT` fragment — but **only** when it still
//! holds the exact value seeded by `20261012000003`, so a user edit survives
//! untouched.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Version of `migrations/20261027000001_opentodo.sql`.
const OPENTODO_VERSION: i64 = 20261027000001;

/// The skill's `prompt_heading` (first line of the fragment).
const HEADING: &str = "# SKILL ACTIVA: PENDIENTES";

/// Substrings the new fragment must carry: OpenTodo by name, the seven views,
/// the priority scale, `someday`, the four labels, the purge warning and the
/// reminder/task distinction.
const FRAGMENT_NEEDLES: [&str; 22] = [
    "OpenTodo",
    "Bandeja de entrada",
    "Hoy",
    "Próximo",
    "En cualquier momento",
    "Algún día",
    "En marcha",
    "En espera",
    "inbox",
    "today",
    "upcoming",
    "anytime",
    "someday",
    "doing",
    "waiting",
    "shared",
    "personal",
    "set_label",
    "purge",
    "reminders",
    "critical",
    "low",
];

/// A database with every migration applied (`run_migrations()`).
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

/// Read the OpenTodo migration SQL from disk.
fn migration_sql() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("migrations/20261027000001_opentodo.sql");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the `opentodo-integration` migration does not exist yet at {}: {e}",
            path.display()
        )
    })
}

/// A pool with every migration **older** than `before` applied, so it is left in
/// the state the previous release had: the OpenTodo settings do not exist yet and
/// `SKILL_PENDIENTES_PROMPT` still holds the value seeded by `20261012000003`.
async fn pool_before(before: i64) -> SqlitePool {
    // `run_migrations` calls `register_vec_extension()` before anything else;
    // `sqlite3_auto_extension` is process-global but only reaches connections
    // opened *after* it runs. A throwaway pool triggers the registration, so the
    // pool below (whose connections open later) inherits `vec0`.
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

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .unwrap();

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
    sqlx::query_scalar::<_, Option<String>>("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|e| panic!("reading setting '{key}' must not fail: {e}"))
        .flatten()
}

/// Run the migration body against `pool`.
async fn apply_migration(pool: &SqlitePool) {
    let sql = migration_sql();
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(pool)
        .await
        .expect("the migration body must run");
}

/// Assert the fragment keeps its heading first and carries every concept.
fn assert_fragment_contract(value: &str) {
    assert!(
        value.starts_with(HEADING),
        "the fragment must still start with the skill's heading {HEADING:?}; got: {value}"
    );
    for needle in FRAGMENT_NEEDLES {
        assert!(
            value.contains(needle),
            "the fragment must mention {needle:?}; got: {value}"
        );
    }
}

// ── 1: freshly migrated database ────────────────────────────────────────────

/// Tarea 4.1 — guardián de contenido: la migración siembra los dos ajustes con
/// sus defaults.
#[test]
fn test_opentodo_migration_seeds_the_two_settings() {
    let sql = migration_sql();

    for (key, value) in [
        ("opentodo_base_url", "https://todo.territoriolinux.es"),
        ("opentodo_token", ""),
    ] {
        assert!(
            sql.contains(&format!("VALUES ('{key}', '{value}'")),
            "the migration must seed `{key}` with its default `{value}`, got SQL:\n{sql}"
        );
    }
}

/// Scenario: los ajustes se siembran con sus defaults.
#[tokio::test]
async fn test_opentodo_settings_seeded() {
    let pool = setup().await;

    assert_eq!(
        settings_value(&pool, "opentodo_base_url").await.as_deref(),
        Some("https://todo.territoriolinux.es"),
        "the base URL must default to the public OpenTodo instance"
    );
    assert_eq!(
        settings_value(&pool, "opentodo_token").await.as_deref(),
        Some(""),
        "the token must be seeded empty (the user fills it in)"
    );
}

/// El fragmento migrado cumple su contrato: encabezado primero y todos los
/// conceptos nuevos.
#[tokio::test]
async fn test_fragment_is_promoted_on_a_fresh_database() {
    let pool = setup().await;

    let fragment = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must exist after migrating");
    assert_fragment_contract(&fragment);
}

// ── 2: the settings respect the user ────────────────────────────────────────

/// Scenario: la migración respeta un valor ya fijado. Una URL base elegida por el
/// usuario no se pisa, y el token tampoco.
#[tokio::test]
async fn test_opentodo_migration_respects_user_values() {
    let pool = pool_before(OPENTODO_VERSION).await;

    for (key, value) in [
        ("opentodo_base_url", "https://todo.midominio.es"),
        ("opentodo_token", "user-token"),
    ] {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2)")
            .bind(key)
            .bind(value)
            .execute(&pool)
            .await
            .unwrap();
    }

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "opentodo_base_url").await.as_deref(),
        Some("https://todo.midominio.es"),
        "a user-configured base URL must be preserved"
    );
    assert_eq!(
        settings_value(&pool, "opentodo_token").await.as_deref(),
        Some("user-token"),
        "a user-set token must be preserved"
    );
}

// ── 3: the fragment promotion ───────────────────────────────────────────────

/// Scenario: el fragmento sembrado se promueve. El `WHERE` del `UPDATE` copia
/// **literalmente** el valor sembrado por `20261012000003`, de modo que solo ese
/// texto se promueve.
#[tokio::test]
async fn test_migration_promotes_the_untouched_fragment() {
    let pool = pool_before(OPENTODO_VERSION).await;

    // El valor vigente antes de esta migración, leído de la base de datos:
    // nunca se escribe a mano en el test, la aserción es que la migración
    // sustituye *ese* valor.
    let seeded = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must be seeded by 20261012000003");
    assert!(
        seeded.starts_with(HEADING),
        "the pre-migration fragment must start with the heading; got: {seeded}"
    );
    assert!(
        !seeded.contains("OpenTodo"),
        "the pre-migration fragment must not know OpenTodo yet; got: {seeded}"
    );

    // El `WHERE` tiene que copiarlo literal: si alguien lo transcribe mal, la
    // promoción no dispara nunca y el fragmento se queda viejo.
    let sql = migration_sql();
    assert!(
        sql.contains(&seeded),
        "the migration's `WHERE` must quote the exact value seeded by \
         20261012000003; got SQL:\n{sql}"
    );

    apply_migration(&pool).await;

    let updated = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must still exist after the migration");
    assert_ne!(
        updated, seeded,
        "the untouched fragment must be replaced by the new one"
    );
    assert_fragment_contract(&updated);
}

/// Scenario: un fragmento editado por el usuario no se pisa.
#[tokio::test]
async fn test_migration_respects_a_customized_fragment() {
    let pool = pool_before(OPENTODO_VERSION).await;

    sqlx::query(
        "UPDATE settings SET value = 'MI PENDIENTES A MEDIDA' \
         WHERE key = 'SKILL_PENDIENTES_PROMPT'",
    )
    .execute(&pool)
    .await
    .unwrap();

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
            .await
            .as_deref(),
        Some("MI PENDIENTES A MEDIDA"),
        "a customised fragment must be left untouched"
    );
}

/// Scenario: una clave ausente se rellena con el texto nuevo.
#[tokio::test]
async fn test_migration_fills_a_missing_fragment() {
    let pool = pool_before(OPENTODO_VERSION).await;

    sqlx::query("DELETE FROM settings WHERE key = 'SKILL_PENDIENTES_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool).await;

    let fragment = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("a missing SKILL_PENDIENTES_PROMPT must be filled by the migration");
    assert_fragment_contract(&fragment);
}

/// Una clave vacía también se rellena.
///
/// El caso `NULL` no se puede ejercitar aquí: `settings.value` es `TEXT NOT
/// NULL` (`20260925000001_initial.sql`), de modo que la guarda `IS NULL` de la
/// migración es defensiva e inalcanzable. Una clave *ausente* sí se cubre en
/// [`test_migration_fills_a_missing_fragment`].
#[tokio::test]
async fn test_migration_fills_an_empty_fragment() {
    let pool = pool_before(OPENTODO_VERSION).await;

    sqlx::query("UPDATE settings SET value = '' WHERE key = 'SKILL_PENDIENTES_PROMPT'")
        .execute(&pool)
        .await
        .unwrap();

    apply_migration(&pool).await;

    let fragment = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("an empty fragment must be filled");
    assert_fragment_contract(&fragment);
}

// ── 4: idempotency ──────────────────────────────────────────────────────────

/// Re-ejecutar el cuerpo de la migración no cambia nada ni duplica filas.
#[tokio::test]
async fn test_migration_body_is_idempotent() {
    let pool = setup().await;

    let first_fragment = settings_value(&pool, "SKILL_PENDIENTES_PROMPT").await;
    let first_base_url = settings_value(&pool, "opentodo_base_url").await;

    apply_migration(&pool).await;

    assert_eq!(
        settings_value(&pool, "SKILL_PENDIENTES_PROMPT").await,
        first_fragment,
        "re-running the body must not change the fragment"
    );
    assert_eq!(
        settings_value(&pool, "opentodo_base_url").await,
        first_base_url,
        "re-running the body must not change the base URL"
    );

    for key in [
        "opentodo_base_url",
        "opentodo_token",
        "SKILL_PENDIENTES_PROMPT",
    ] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "expected exactly one row for key '{key}'");
    }
}

/// Scenario: el migrador, ejecutado dos veces, deja una sola fila por clave.
#[tokio::test]
async fn test_migration_is_idempotent_through_migrator() {
    let pool = setup().await;

    valet::db::schema::run_migrations(&pool).await.unwrap();
    valet::db::schema::run_migrations(&pool).await.unwrap();

    for key in [
        "opentodo_base_url",
        "opentodo_token",
        "SKILL_PENDIENTES_PROMPT",
    ] {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "the key '{key}' must exist exactly once");
    }

    let fragment = settings_value(&pool, "SKILL_PENDIENTES_PROMPT")
        .await
        .expect("SKILL_PENDIENTES_PROMPT must exist");
    assert_fragment_contract(&fragment);
}
