-- Chronological facts (`timeline_events`) and the widened `llm_requests.kind`.
--
-- Two independent chores in one migration file:
--
--   1) `timeline_events`. A new table holding atomic facts the user has lived,
--      done or decided, each already dated. `category` is a closed set enforced
--      by a CHECK (kept in sync with `TIMELINE_CATEGORIES` in
--      `src/models/timeline.rs`). `source_message_id` is a foreign key to
--      `messages(id)` with `ON DELETE SET NULL`: pruning a message must never
--      delete the fact it produced, it only severs the provenance link. The
--      three indexes serve the reads the tools make: newest-first by date, a
--      category filter, and lookup by origin message (partial, since most
--      facts have a source but the index only covers the non-NULL ones).
--
--   2) `llm_requests.kind`. The origin discriminator must learn a new value,
--      `'timeline'`, for the chronological extractor. SQLite cannot ALTER a
--      CHECK constraint in place, so the only correct way to widen it is the
--      documented 12-step table rebuild: create a new table with the same
--      columns and the widened CHECK, copy every row across, drop the old
--      table and rename. **No row is lost**: the `INSERT ... SELECT *`
--      preserves all rows whatever their `kind`, and the column order of the
--      new table matches the migrated one exactly (the original columns plus
--      `kind` in last position, where `ALTER TABLE ... ADD COLUMN` left it).
--
-- Both statements are written so that re-executing this file verbatim is safe:
-- `CREATE TABLE IF NOT EXISTS` guards the table and the `llm_requests_new`
-- scratch table is only a temporary name that disappears after the rename.
--
-- There are no indexes, triggers or views on `llm_requests` in this repo, so
-- there is nothing to recreate after the rename (checked across `migrations/`
-- and `src/`).

-- 1) The chronological facts table.
CREATE TABLE IF NOT EXISTS timeline_events (
    id                TEXT PRIMARY KEY,
    timestamp         TEXT NOT NULL,
    category          TEXT NOT NULL DEFAULT 'lifestyle'
                      CHECK (category IN ('sport','lifestyle','work','shopping','health','social','system')),
    fact              TEXT NOT NULL,
    source_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_timeline_events_timestamp
    ON timeline_events(timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_timeline_events_category
    ON timeline_events(category);

CREATE INDEX IF NOT EXISTS idx_timeline_events_source_message
    ON timeline_events(source_message_id)
    WHERE source_message_id IS NOT NULL;

-- 2) Widen `llm_requests.kind` by rebuilding the table.
CREATE TABLE IF NOT EXISTS llm_requests_new (
    id                TEXT PRIMARY KEY,
    model             TEXT NOT NULL,
    provider          TEXT,
    profile_id        TEXT REFERENCES profiles(id),
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens      INTEGER NOT NULL DEFAULT 0,
    cached_tokens     INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens  INTEGER NOT NULL DEFAULT 0,
    cost              REAL NOT NULL DEFAULT 0.0,
    is_byok           INTEGER NOT NULL DEFAULT 0,
    duration_ms       INTEGER,
    cache_hit         INTEGER NOT NULL DEFAULT 0,
    status            TEXT NOT NULL DEFAULT 'success'
                      CHECK(status IN ('success', 'error', 'timeout')),
    error_message     TEXT,
    tool_calls        TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    kind              TEXT NOT NULL DEFAULT 'chat'
                      CHECK (kind IN ('chat', 'router', 'archivist', 'consolidator', 'collapse', 'timeline'))
);

INSERT INTO llm_requests_new SELECT * FROM llm_requests;

DROP TABLE llm_requests;

ALTER TABLE llm_requests_new RENAME TO llm_requests;
