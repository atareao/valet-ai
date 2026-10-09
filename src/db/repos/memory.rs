use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::db::repos::settings::SettingsRepo;
use crate::models::Memory;

/// Repository for the episodic `memory` + `vec_memory` tables.
pub struct MemoryRepo;

#[allow(unused_variables)]
impl MemoryRepo {
    /// Create a new episodic memory card.
    ///
    /// Generates a UUID `id` and a `created_at` timestamp automatically.
    /// Persists the row in both `memory` and `vec_memory` tables.
    pub async fn create(
        pool: &SqlitePool,
        content: &str,
        tokens_count: usize,
        metadata: &serde_json::Value,
    ) -> Result<Memory, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let memory = Self::create_in_tx(&mut tx, content, tokens_count, metadata).await?;
        tx.commit().await?;
        Ok(memory)
    }

    /// Create a new episodic memory card inside an existing transaction.
    ///
    /// Identical to [`create`](Self::create) but participates in the caller's
    /// transaction, so the `memory` row can be committed atomically together
    /// with its `vec_memory` embedding (no orphan rows on failure).
    pub async fn create_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        content: &str,
        tokens_count: usize,
        metadata: &serde_json::Value,
    ) -> Result<Memory, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let metadata_str = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO memory (id, content, tokens_count, created_at, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&id)
        .bind(content)
        .bind(tokens_count as i64)
        .bind(&now)
        .bind(&metadata_str)
        .execute(&mut **tx)
        .await?;

        Ok(Memory {
            id,
            content: content.to_string(),
            tokens_count,
            created_at: now,
            metadata: metadata.clone(),
        })
    }

    /// Find a memory card by its primary key.
    pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Memory>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, content, tokens_count, created_at, metadata \
             FROM memory WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| {
            let metadata_str: String = r.get(4);
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
            Memory {
                id: r.get(0),
                content: r.get(1),
                tokens_count: r.get::<i64, _>(2) as usize,
                created_at: r.get(3),
                metadata,
            }
        }))
    }

    /// List memory cards with pagination, ordered by `created_at DESC`.
    ///
    /// Returns `(items, total_count)`.
    pub async fn list(
        pool: &SqlitePool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Memory>, i64), sqlx::Error> {
        let actual_limit = limit.clamp(1, 100);
        let actual_offset = offset.max(0);

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory")
            .fetch_one(pool)
            .await?;

        let rows = sqlx::query(
            "SELECT id, content, tokens_count, created_at, metadata \
             FROM memory ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(actual_limit)
        .bind(actual_offset)
        .fetch_all(pool)
        .await?;

        let items: Vec<Memory> = rows
            .iter()
            .map(|r| {
                let metadata_str: String = r.get(4);
                let metadata: serde_json::Value =
                    serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
                Memory {
                    id: r.get(0),
                    content: r.get(1),
                    tokens_count: r.get::<i64, _>(2) as usize,
                    created_at: r.get(3),
                    metadata,
                }
            })
            .collect();

        Ok((items, total))
    }

    /// Delete a memory card by id.
    ///
    /// Returns `true` if a row was actually deleted, `false` otherwise.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM memory WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Search memory cards by cosine similarity of their embeddings.
    ///
    /// This is the **retrieval** half of the episodic-memory pipeline. It reads
    /// the three retrieval knobs from `settings` on **every** call (so editing
    /// them takes effect without a restart) and runs the documented pipeline:
    ///
    /// 1. `vec0` KNN: `MATCH … AND k = MEMORY_KNN_CANDIDATES ORDER BY distance`
    /// 2. `similitud = 1 - distance` (cosine metric: identical → 0, orthogonal → 1)
    /// 3. drop `similitud < SIMILARITY_THRESHOLD` — applied to the **similarity**,
    ///    never to the decayed score, so an old but highly similar card stays
    ///    reachable (D3)
    /// 4. `final = similitud × exp(-ln2 × días / MEMORY_HALF_LIFE_DAYS)` — in
    ///    Rust via `f64::exp()`, because the SQLite `sqlx` links has no math
    ///    functions (D4)
    /// 5. order by `final` descending
    ///
    /// The token budget (`RAG_BUDGET_TOKENS`) is **not** applied here: it
    /// concerns the size of the prompt, not retrieval, and lives in
    /// `ContextBuilder` (see the `episodic-memory-injection` design).
    ///
    /// `MEMORY_KNN_CANDIDATES` bounds the *candidates*, not the result: because
    /// the KNN is ordered by ascending distance and the threshold keeps a
    /// prefix, one candidate above the threshold is enough for a non-empty
    /// result (D5).
    pub async fn search_by_vector(
        pool: &SqlitePool,
        query_embedding: &[f32],
    ) -> Result<Vec<Memory>, sqlx::Error> {
        // Degenerate case: empty query → no results
        if query_embedding.is_empty() {
            return Ok(Vec::new());
        }

        let knn_candidates = read_usize_setting(pool, "MEMORY_KNN_CANDIDATES", 20).await?;
        let similarity_threshold = read_f64_setting(pool, "SIMILARITY_THRESHOLD", 0.5).await?;
        let half_life_days = read_f64_setting(pool, "MEMORY_HALF_LIFE_DAYS", 90.0).await?;

        let query_json =
            serde_json::to_string(query_embedding).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        // KNN over the `vec0` index, joined back to the source-of-truth
        // `memory` table by id, ordered by ascending cosine distance (which is
        // descending similarity).
        let rows = sqlx::query(
            "SELECT m.id, m.content, m.tokens_count, m.created_at, m.metadata, v.distance \
             FROM memory m JOIN vec_memory v ON m.id = v.id \
             WHERE v.embedding MATCH vec_f32(?1) AND k = ?2 \
             ORDER BY v.distance",
        )
        .bind(&query_json)
        .bind(knn_candidates as i64)
        .fetch_all(pool)
        .await?;

        let now = chrono::Utc::now();
        let mut scored: Vec<(f64, Memory)> = Vec::with_capacity(rows.len());
        for r in rows {
            // `distance_metric=cosine` ⇒ `distance = 1 - similarity`.
            let distance: f64 = r.get(5);
            let similarity = 1.0 - distance;

            // Step 3 (D3): the threshold is applied to the SIMILARITY, never to
            // the decayed score. The KNN already ordered the candidates by
            // ascending distance (descending similarity), so once one falls
            // below the threshold every later candidate does too and we can
            // stop: the surviving set is a prefix.
            if similarity < similarity_threshold {
                break;
            }

            let metadata_str: String = r.get(4);
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_str).unwrap_or(serde_json::json!({}));
            let created_at: String = r.get(3);

            // Steps 4–5 (D4): exponential half-life decay, computed in Rust.
            // The age is measured from `metadata.last_message_at` (the date of
            // the newest origin message: a card is as current as its newest
            // fact). Cards written before the `episodic-memory-occurred-at`
            // change have no such key and fall back to `memory.created_at`.
            // A non-positive or non-finite half-life disables decay rather than
            // producing nonsensical scores.
            let anchor = metadata
                .get("last_message_at")
                .and_then(|v| v.as_str())
                .unwrap_or(created_at.as_str());
            let decay = if half_life_days > 0.0 && half_life_days.is_finite() {
                (-std::f64::consts::LN_2 * days_since(anchor, &now) / half_life_days).exp()
            } else {
                1.0
            };
            let final_score = similarity * decay;

            scored.push((
                final_score,
                Memory {
                    id: r.get(0),
                    content: r.get(1),
                    tokens_count: r.get::<i64, _>(2) as usize,
                    created_at,
                    metadata,
                },
            ));
        }

        // Step 6: order by relevance descending.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored.into_iter().map(|(_, m)| m).collect())
    }
}

/// Read an `f64` knob from `settings`, falling back to `default` when the key
/// is missing or its value cannot be parsed.
async fn read_f64_setting(pool: &SqlitePool, key: &str, default: f64) -> Result<f64, sqlx::Error> {
    Ok(SettingsRepo::get(pool, key)
        .await?
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default))
}

/// Read a non-negative integer knob from `settings`, falling back to `default`
/// when the key is missing or its value cannot be parsed.
async fn read_usize_setting(
    pool: &SqlitePool,
    key: &str,
    default: usize,
) -> Result<usize, sqlx::Error> {
    Ok(SettingsRepo::get(pool, key)
        .await?
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default))
}

/// Age of a card in days, measured from `anchor` — the card's
/// `metadata.last_message_at` when present, or its `created_at` otherwise (RFC
/// 3339, always in UTC).
///
/// A timestamp in the future (clock skew, hand-edited data) yields `0.0`
/// rather than a negative age, so it simply does not decay; an unreadable
/// anchor also yields `0.0` (treated as "no decay" rather than dropped, since
/// the card's similarity already passed the threshold).
fn days_since(anchor: &str, now: &chrono::DateTime<chrono::Utc>) -> f64 {
    match chrono::DateTime::parse_from_rfc3339(anchor) {
        Ok(dt) => ((now.timestamp() - dt.timestamp()) as f64 / 86_400.0).max(0.0),
        Err(_) => 0.0,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// RED tests – these will fail (compile or panic with todo!) until the GREEN
// phase provides implementations.
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create a single-connection in-memory pool and run all migrations.
    /// The `memory` and `vec_memory` tables are created by migration
    /// `20260926000003_episodic_memory.sql`.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create in-memory pool");
        run_migrations(&pool).await.expect("migrations failed");
        pool
    }

    // ─── create ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_memory_with_metadata() {
        let pool = setup_pool().await;

        let metadata = serde_json::json!({"tags": ["rust"]});
        let mem = MemoryRepo::create(&pool, "Test memory content", 150, &metadata)
            .await
            .expect("create should succeed");

        // UUID is generated
        assert!(!mem.id.is_empty(), "id should be a non-empty UUID");

        // created_at is populated
        assert!(!mem.created_at.is_empty(), "created_at should not be empty");

        // Exact token count
        assert_eq!(mem.tokens_count, 150, "tokens_count should match input");

        // Content is preserved
        assert_eq!(mem.content, "Test memory content");

        // Metadata round-trips correctly
        assert_eq!(mem.metadata, metadata, "metadata should round-trip");
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let pool = setup_pool().await;

        let created = MemoryRepo::create(&pool, "Find me", 10, &serde_json::json!({}))
            .await
            .expect("create should succeed");

        let found = MemoryRepo::find_by_id(&pool, &created.id)
            .await
            .expect("find_by_id should not error");

        assert!(found.is_some(), "should find the memory by id");
        let mem = found.unwrap();
        assert_eq!(mem.content, "Find me");
        assert_eq!(mem.tokens_count, 10);
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let pool = setup_pool().await;

        let found = MemoryRepo::find_by_id(&pool, "nonexistent-id")
            .await
            .expect("find_by_id should not error");

        assert!(found.is_none(), "should return None for missing id");
    }

    // ─── list ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_pagination() {
        let pool = setup_pool().await;

        // Insert three memories with staggered delays to guarantee
        // deterministic created_at ordering.
        let _m1 = MemoryRepo::create(&pool, "Alpha", 10, &serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let m2 = MemoryRepo::create(&pool, "Beta", 20, &serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let m3 = MemoryRepo::create(&pool, "Gamma", 30, &serde_json::json!({}))
            .await
            .unwrap();

        let (items, total) = MemoryRepo::list(&pool, 2, 0)
            .await
            .expect("list should succeed");

        // limit=2 → 2 items
        assert_eq!(items.len(), 2, "should return at most `limit` items");

        // total = 3
        assert_eq!(total, 3, "total should reflect all rows");

        // Descending order by created_at: Gamma (latest) first, then Beta
        assert_eq!(
            items[0].id, m3.id,
            "first item should be the most recent (Gamma)"
        );
        assert_eq!(
            items[1].id, m2.id,
            "second item should be the second most recent (Beta)"
        );
    }

    // ─── delete ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_found() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "To delete", 5, &serde_json::json!({}))
            .await
            .expect("create should succeed");

        let deleted = MemoryRepo::delete(&pool, &mem.id)
            .await
            .expect("delete should not error");
        assert!(deleted, "delete should return true when a row is removed");
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = setup_pool().await;

        let deleted = MemoryRepo::delete(&pool, "nonexistent-id")
            .await
            .expect("delete should not error");
        assert!(!deleted, "delete should return false for missing id");
    }

    // ─── search_by_vector ────────────────────────────────────────────────────
    //
    // EXCEPTION 2 TO THE TEST INVARIANT: these tests used to store the
    // embedding as JSON text in a regular `vec_memory` table. That storage is
    // gone (`vec_memory` is now a `vec0` virtual table holding binary
    // vectors), so the helper was changed on purpose to write through
    // `vec_f32(?)` and to pad vectors to the declared 1024 dimensions. The
    // assertions themselves are unchanged.

    /// Pad a leading slice to the 1024 dimensions `vec0` requires.
    fn v1024(leading: &[f32]) -> Vec<f32> {
        let mut v = leading.to_vec();
        v.resize(1024, 0.0);
        v
    }

    /// Helper: insert an embedding row directly into `vec_memory`.
    ///
    /// CHANGED ON PURPOSE (invariant exception 2): written through
    /// `vec_f32(?)` with the declared 1024 dimensions, instead of a raw JSON
    /// string in the old regular table.
    async fn insert_embedding(pool: &SqlitePool, id: &str, embedding: &[f32]) {
        let json = serde_json::to_string(embedding).expect("failed to serialize embedding");
        sqlx::query("INSERT INTO vec_memory (id, embedding) VALUES (?1, vec_f32(?2))")
            .bind(id)
            .bind(&json)
            .execute(pool)
            .await
            .expect("failed to insert vec_memory row");
    }

    /// Helper: insert a `memory` row with an explicit `created_at`, so tests
    /// can control the temporal decay (the repository's `create` always stamps
    /// `now`).
    async fn insert_memory_at(
        pool: &SqlitePool,
        id: &str,
        content: &str,
        tokens_count: usize,
        created_at: &str,
    ) {
        sqlx::query(
            "INSERT INTO memory (id, content, tokens_count, created_at, metadata) \
             VALUES (?1, ?2, ?3, ?4, '{}')",
        )
        .bind(id)
        .bind(content)
        .bind(tokens_count as i64)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("failed to insert memory row");
    }

    /// Helper: insert a `memory` row with an explicit `created_at` **and** an
    /// explicit `metadata` JSON, so tests can probe which timestamp the decay
    /// actually reads.
    async fn insert_memory_with_metadata_at(
        pool: &SqlitePool,
        id: &str,
        content: &str,
        tokens_count: usize,
        created_at: &str,
        metadata: &serde_json::Value,
    ) {
        let metadata_str = serde_json::to_string(metadata).expect("metadata should serialize");
        sqlx::query(
            "INSERT INTO memory (id, content, tokens_count, created_at, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(content)
        .bind(tokens_count as i64)
        .bind(created_at)
        .bind(&metadata_str)
        .execute(pool)
        .await
        .expect("failed to insert memory row");
    }

    /// Helper: read a numeric setting (panics if missing/unparseable).
    async fn setting_f64(pool: &SqlitePool, key: &str) -> f64 {
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("setting '{key}' missing: {e}"))
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("setting '{key}' not a float: {e}"))
    }

    /// Helper: upsert a setting value.
    async fn set_setting(pool: &SqlitePool, key: &str, value: &str) {
        crate::db::repos::settings::SettingsRepo::set(pool, key, value)
            .await
            .expect("failed to set setting");
    }

    /// A unit vector at cosine similarity `s` to `(1, 0, …, 0)`, padded to
    /// the `vec0` table's 1024 dimensions.
    fn unit_vector_at_cosine(s: f32) -> Vec<f32> {
        let orthogonal = (1.0 - s * s).max(0.0).sqrt();
        v1024(&[s, orthogonal, 0.0])
    }

    #[tokio::test]
    async fn test_search_by_vector_returns_most_similar_first() {
        let pool = setup_pool().await;

        // Two memories with clearly different vectors
        let mem_a = MemoryRepo::create(&pool, "A: x-axis", 100, &serde_json::json!({}))
            .await
            .unwrap();
        let mem_b = MemoryRepo::create(&pool, "B: z-axis", 50, &serde_json::json!({}))
            .await
            .unwrap();

        // Embeddings: mem_a → [1,0,0],  mem_b → [0,0,1]
        insert_embedding(&pool, &mem_a.id, &v1024(&[1.0, 0.0, 0.0])).await;
        insert_embedding(&pool, &mem_b.id, &v1024(&[0.0, 0.0, 1.0])).await;

        // Query close to [1,0,0] → mem_a should be most similar
        let results = MemoryRepo::search_by_vector(&pool, &v1024(&[0.9, 0.1, 0.0]))
            .await
            .expect("search_by_vector should succeed");

        assert!(!results.is_empty(), "should return at least one result");
        assert_eq!(
            results[0].id, mem_a.id,
            "most similar memory should be mem_a (x-axis)"
        );
    }

    /// 5.2 — a candidate whose similarity is above `SIMILARITY_THRESHOLD` is
    /// returned. An identical vector has cosine distance 0 ⇒ similarity 1.0.
    #[tokio::test]
    async fn test_search_by_vector_includes_similarity_above_threshold() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Identical", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        // Default threshold is 0.5 (seeded by migration); similarity is 1.0.
        assert_eq!(setting_f64(&pool, "SIMILARITY_THRESHOLD").await, 0.5);

        let results = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");

        assert_eq!(results.len(), 1, "similarity 1.0 ≥ 0.5 must be returned");
        assert_eq!(results[0].id, mem.id);
    }

    /// 5.2 — a candidate whose similarity is below the threshold is discarded.
    /// Cosine distance 0.8 ⇒ similarity 0.2 < 0.5.
    #[tokio::test]
    async fn test_search_by_vector_discards_similarity_below_threshold() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Orthogonal-ish", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        // Query at cosine similarity 0.2 (distance 0.8).
        let results = MemoryRepo::search_by_vector(&pool, &unit_vector_at_cosine(0.2))
            .await
            .expect("search should succeed");

        assert!(
            results.is_empty(),
            "similarity 0.2 < threshold 0.5 must be discarded"
        );
    }

    /// 5.2 — a high threshold leaves the result empty even for a card that
    /// would pass the default.
    #[tokio::test]
    async fn test_search_by_vector_high_threshold_returns_empty() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Fairly similar", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        let query = unit_vector_at_cosine(0.7); // similarity 0.7

        // With the default threshold the card is returned...
        let before = MemoryRepo::search_by_vector(&pool, &query)
            .await
            .expect("search should succeed");
        assert_eq!(before.len(), 1, "similarity 0.7 ≥ 0.5 is returned");

        // ...but raising the threshold above it empties the result.
        set_setting(&pool, "SIMILARITY_THRESHOLD", "0.95").await;
        let after = MemoryRepo::search_by_vector(&pool, &query)
            .await
            .expect("search should succeed");
        assert!(
            after.is_empty(),
            "similarity 0.7 < threshold 0.95 must be discarded"
        );
    }

    /// 5.3 (D3) — the threshold is applied to the SIMILARITY, not to the decayed
    /// score: an old card whose similarity is above the threshold stays in the
    /// result even though its `final` is far below a recent card's. The decay
    /// changes the ORDER, not the membership.
    #[tokio::test]
    async fn test_decay_reorders_but_does_not_drop_old_cards() {
        let pool = setup_pool().await;

        // A recent card with a modest similarity.
        let recent = MemoryRepo::create(&pool, "Recent", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &recent.id, &unit_vector_at_cosine(0.7)).await;

        // An old card (5 years) with a HIGHER similarity: its decayed score is
        // tiny, but its similarity (0.99) is well above the threshold.
        let old_created = (chrono::Utc::now() - chrono::Duration::days(5 * 365)).to_rfc3339();
        insert_memory_at(&pool, "old-card", "Old but relevant", 10, &old_created).await;
        insert_embedding(&pool, "old-card", &unit_vector_at_cosine(0.99)).await;

        let query = unit_vector_at_cosine(0.7);
        let results = MemoryRepo::search_by_vector(&pool, &query)
            .await
            .expect("search should succeed");

        assert_eq!(
            results.len(),
            2,
            "the old card must survive: the threshold filters by similarity, not by decayed score"
        );
        assert_eq!(
            results[0].id, recent.id,
            "the recent card ranks first because the old card's decayed score is smaller"
        );
        assert_eq!(
            results[1].id, "old-card",
            "the old card is present, just ordered last"
        );
    }

    /// Two candidates with the SAME similarity but different
    /// `metadata.last_message_at`: the one whose facts are more recent must
    /// rank first, because `MemoryRepo::search_by_vector` measures the age from
    /// `metadata.last_message_at`.
    #[tokio::test]
    async fn test_decay_orders_by_last_message_at_with_equal_similarity() {
        let pool = setup_pool().await;

        let now = chrono::Utc::now();
        let old_created = (now - chrono::Duration::days(5 * 365)).to_rfc3339();
        let new_created = now.to_rfc3339();
        let old_facts = (now - chrono::Duration::days(5 * 365)).to_rfc3339();
        let new_facts = now.to_rfc3339();

        // Same similarity (0.9) for both: only the decay can order them.
        // Its `created_at` is old, but its facts (`last_message_at`) are recent.
        insert_memory_with_metadata_at(
            &pool,
            "newer-facts",
            "Newer facts",
            10,
            &old_created,
            &serde_json::json!({ "last_message_at": new_facts }),
        )
        .await;
        insert_embedding(&pool, "newer-facts", &unit_vector_at_cosine(0.9)).await;

        // Its `created_at` is fresh, but its facts (`last_message_at`) are old.
        insert_memory_with_metadata_at(
            &pool,
            "older-facts",
            "Older facts",
            10,
            &new_created,
            &serde_json::json!({ "last_message_at": old_facts }),
        )
        .await;
        insert_embedding(&pool, "older-facts", &unit_vector_at_cosine(0.9)).await;

        let results = MemoryRepo::search_by_vector(&pool, &unit_vector_at_cosine(0.9))
            .await
            .expect("search should succeed");

        assert_eq!(
            results.len(),
            2,
            "both cards share the same similarity above the threshold"
        );
        assert_eq!(
            results[0].id, "newer-facts",
            "the card whose `last_message_at` is more recent must rank first"
        );
        assert_eq!(
            results[1].id, "older-facts",
            "the card whose `last_message_at` is older must rank last"
        );
    }

    /// Fallback (task 3.3): a card whose `metadata` has no `last_message_at`
    /// measures its age from `memory.created_at`. It is neither dropped nor an
    /// error: the change only adds an anchor, it never removes cards.
    #[tokio::test]
    async fn test_decay_falls_back_to_created_at_when_last_message_at_missing() {
        let pool = setup_pool().await;
        let now = chrono::Utc::now();

        // Fresh by `created_at`, metadata `{}` (no anchor) → no decay.
        let fresh = MemoryRepo::create(&pool, "Fresh", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &fresh.id, &unit_vector_at_cosine(0.9)).await;

        // Old by `created_at`, metadata `{}` (no anchor) → decays via `created_at`.
        let old_created = (now - chrono::Duration::days(5 * 365)).to_rfc3339();
        insert_memory_at(&pool, "old-no-anchor", "Old", 10, &old_created).await;
        insert_embedding(&pool, "old-no-anchor", &unit_vector_at_cosine(0.9)).await;

        let results = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");

        assert_eq!(
            results.len(),
            2,
            "a card without `last_message_at` must not be dropped for the absence of the key"
        );
        assert_eq!(
            results[0].id, fresh.id,
            "the fresh card ranks first: its age falls back to `created_at` with no decay"
        );
        assert_eq!(
            results[1].id, "old-no-anchor",
            "the old card falls back to `created_at` and ranks last"
        );
    }

    // ─── Characterization: the decay anchor is `last_message_at` ────────────
    //
    // `episodic-memory-occurred-at` changes, on purpose, where the temporal
    // decay measures the age from. The block-1 version of this test pinned the
    // OLD anchor (`memory.created_at`); the assertion has moved here on purpose
    // and is **exception 2 of 2** to this change's invariant (exception 1 is
    // dropping the date from `format_memory`, in `orchestrator::context_builder`).
    // The test is renamed to match the new behaviour. Any OTHER test that breaks
    // while implementing the change is a regression, not an update.
    //
    /// CHARACTERIZATION OF THE NEW DECAY ANCHOR: the age is measured from
    /// `metadata.last_message_at`. A card with a **lower** similarity but recent
    /// facts beats a card with a higher similarity whose facts are 5 years old.
    #[tokio::test]
    async fn characterization_decay_measures_age_from_last_message_at() {
        let pool = setup_pool().await;
        let now = chrono::Utc::now();
        let five_years_ago = (now - chrono::Duration::days(5 * 365)).to_rfc3339();
        let just_now = now.to_rfc3339();

        // Recent facts (`last_message_at` = now), but an old card
        // (`created_at` = 5 years ago) and a LOWER similarity (0.7).
        insert_memory_with_metadata_at(
            &pool,
            "recent-facts",
            "Recent facts, lower similarity",
            10,
            &five_years_ago,
            &serde_json::json!({ "last_message_at": just_now }),
        )
        .await;
        insert_embedding(&pool, "recent-facts", &unit_vector_at_cosine(0.7)).await;

        // Old facts (`last_message_at` = 5 years ago), a fresh card
        // (`created_at` = now) and a HIGHER similarity (0.9).
        insert_memory_with_metadata_at(
            &pool,
            "old-facts",
            "Old facts, higher similarity",
            10,
            &just_now,
            &serde_json::json!({ "last_message_at": five_years_ago }),
        )
        .await;
        insert_embedding(&pool, "old-facts", &unit_vector_at_cosine(0.9)).await;

        // Query on the x-axis, so the similarity equals the card vector's cosine.
        let results = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");

        assert_eq!(
            results.len(),
            2,
            "both cards clear the similarity threshold and survive the decay"
        );
        assert_eq!(
            results[0].id, "recent-facts",
            "NEW behaviour: the decay measures age from `metadata.last_message_at`, \
             so the card with recent facts wins despite its lower similarity"
        );
        assert_eq!(
            results[1].id, "old-facts",
            "NEW behaviour: the card whose facts are 5 years old is ranked last"
        );
    }

    /// 5.5 (D5) — `MEMORY_KNN_CANDIDATES` bounds the candidates but cannot
    /// empty the result while a candidate above the threshold exists.
    #[tokio::test]
    async fn test_knn_candidates_bounds_without_emptying() {
        let pool = setup_pool().await;

        // Three cards, all above the threshold, at decreasing similarity.
        for (id, sim) in [("c1", 0.9f32), ("c2", 0.8), ("c3", 0.7)] {
            insert_memory_at(&pool, id, id, 10, &chrono::Utc::now().to_rfc3339()).await;
            insert_embedding(&pool, id, &unit_vector_at_cosine(sim)).await;
        }

        // With k = 1 only the closest candidate is pulled, but the result is
        // NOT empty (the first candidate clears the threshold).
        set_setting(&pool, "MEMORY_KNN_CANDIDATES", "1").await;
        let one = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");
        assert_eq!(one.len(), 1, "k = 1 must yield exactly one candidate");
        assert_eq!(one[0].id, "c1", "the closest candidate wins");

        // Raising k admits more candidates.
        set_setting(&pool, "MEMORY_KNN_CANDIDATES", "2").await;
        let two = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");
        assert_eq!(two.len(), 2, "k = 2 must yield two candidates");
    }

    /// 6.2 (D7) — the knobs are read on every query, so changing
    /// `SIMILARITY_THRESHOLD` takes effect without a restart.
    #[tokio::test]
    async fn test_similarity_threshold_change_takes_effect_without_restart() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Borderline", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        let query = unit_vector_at_cosine(0.6); // similarity 0.6

        // Threshold 0.5 → the card is returned.
        set_setting(&pool, "SIMILARITY_THRESHOLD", "0.5").await;
        let first = MemoryRepo::search_by_vector(&pool, &query)
            .await
            .expect("search should succeed");
        assert_eq!(first.len(), 1);

        // Threshold 0.8, same pool, no restart → the card is filtered out.
        set_setting(&pool, "SIMILARITY_THRESHOLD", "0.8").await;
        let second = MemoryRepo::search_by_vector(&pool, &query)
            .await
            .expect("search should succeed");
        assert!(
            second.is_empty(),
            "the new threshold must take effect on the next query"
        );
    }

    /// Sanity check: an empty query embedding returns an empty result set
    /// (no crash on degenerate input).
    #[tokio::test]
    async fn test_search_by_vector_empty_embedding() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Some memory", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[0.5, 0.5])).await;

        let results = MemoryRepo::search_by_vector(&pool, &[])
            .await
            .expect("search_by_vector should handle empty query gracefully");

        // Degenerate case: an empty query vector produces no similarity above
        // any reasonable threshold → empty result set is acceptable.
        assert!(
            results.is_empty(),
            "empty query embedding should return no results"
        );
    }

    /// A matching dimension is scored normally and returned.
    #[tokio::test]
    async fn test_search_by_vector_matching_dimension_is_scored() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Right dimension", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        let results = MemoryRepo::search_by_vector(&pool, &v1024(&[1.0, 0.0, 0.0]))
            .await
            .expect("search should succeed");

        assert_eq!(
            results.len(),
            1,
            "an embedding with a matching dimension should be scored and returned"
        );
        assert_eq!(results[0].id, mem.id);
    }

    /// 3.4 — the JOIN by id returns the `memory` fields alongside the `vec0`
    /// distance, which is what makes `memory` the source of truth and
    /// `vec_memory` just the vector index.
    #[tokio::test]
    async fn test_join_returns_memory_fields_and_distance() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Joined content", 42, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0, 0.0])).await;

        let query = serde_json::to_string(&v1024(&[1.0, 0.0, 0.0])).unwrap();
        let row = sqlx::query(
            "SELECT m.id, m.content, m.tokens_count, v.distance \
             FROM memory m JOIN vec_memory v ON m.id = v.id \
             WHERE v.embedding MATCH vec_f32(?1) AND k = ?2 ORDER BY v.distance",
        )
        .bind(&query)
        .bind(10i64)
        .fetch_one(&pool)
        .await
        .expect("the JOIN by id must succeed (no `no column named id` error)");

        assert_eq!(row.get::<String, _>(0), mem.id);
        assert_eq!(row.get::<String, _>(1), "Joined content");
        assert_eq!(row.get::<i64, _>(2), 42);
        // An identical vector has cosine distance 0.
        let distance: f64 = row.get(3);
        assert!(distance.abs() < 1e-6, "identical vector → distance 0");
    }

    /// A query embedding whose dimension does not match the one declared by
    /// `vec0` makes the KNN query fail. This is the last surviving dimension
    /// mismatch: storage is now impossible (`vec0` rejects it structurally),
    /// and the query side surfaces `vec0`'s own error. The caller
    /// (`ContextBuilder`) turns a search error into an empty result with a
    /// warning, so nothing panics.
    ///
    /// CHANGED ON PURPOSE (invariant exception 2): the old implementation
    /// guarded this case explicitly with a `tracing::warn!`; that guard is
    /// retired with the requirement (block 5.1) and the error now propagates.
    #[tokio::test]
    async fn test_search_by_vector_mismatched_query_dimension_errors() {
        let pool = setup_pool().await;

        let mem = MemoryRepo::create(&pool, "Wrong dimension", 10, &serde_json::json!({}))
            .await
            .unwrap();
        insert_embedding(&pool, &mem.id, &v1024(&[1.0, 0.0])).await;

        // Query has dimension 3, which does not match the declared 1024.
        let result = MemoryRepo::search_by_vector(&pool, &[1.0, 0.0, 0.0]).await;

        assert!(
            result.is_err(),
            "a query embedding with a mismatched dimension must surface vec0's error, got {result:?}"
        );
    }
}
