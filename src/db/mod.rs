pub mod fts;
pub mod repos;
pub mod schema;
pub mod vec_extension;

use sqlx::SqlitePool;

/// Type alias for the shared database pool used by all handlers.
pub type DbPool = sqlx::SqlitePool;

/// Initialize a SQLite database at the given path.
///
/// 1. Registers the `sqlite-vec` extension **before** opening the pool, so
///    every connection `sqlx` opens inherits it (see [`vec_extension`]).
/// 2. Opens (or creates) the database file via sqlx pool.
/// 3. Configures WAL journal mode and foreign-key enforcement.
/// 4. Fail-fast: verifies `SELECT vec_version()` works and aborts otherwise.
/// 5. Runs all sqlx schema migrations.
/// 6. Does NOT seed the `tools` table; callers reconcile it from the registry
///    after `init_db` returns.
/// 7. Creates FTS5 triggers for full-text search.
///
/// [`vec_extension`]: crate::db::vec_extension
pub async fn init_db(db_path: &str) -> Result<DbPool, Box<dyn std::error::Error>> {
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    // Register the extension once, process-wide, before any connection exists.
    vec_extension::register_vec_extension();

    let options = SqliteConnectOptions::from_str(db_path)?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    // Fail-fast: a valet that starts without episodic memory is a silent
    // failure, so refuse to start if the extension is not usable (D11).
    vec_extension::verify_vec0(&pool).await?;

    // Run sqlx migrations
    sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
        .await?
        .run(&pool)
        .await?;

    // Fail-fast (D10): the dimension declared by `vec_memory` must line up with
    // EMBEDDING_DIMENSION, otherwise refuse to start rather than search a
    // misaligned index silently.
    vec_extension::verify_embedding_dimension(&pool).await?;

    // Initialize FTS5 triggers
    fts::create_fts_triggers(&pool).await?;

    Ok(pool)
}
