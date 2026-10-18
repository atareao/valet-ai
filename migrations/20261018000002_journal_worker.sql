-- The journal worker's origin and its settings.
--
-- Two independent chores in one migration file:
--
--   1) `llm_requests.kind`. The origin discriminator must learn a new value,
--      `'journal'`, for the journal worker (`src/workers/journal.rs`). SQLite
--      cannot ALTER a CHECK constraint in place, so the only correct way to
--      widen it is the documented table rebuild — the same idiom
--      `20261010000003_timeline_events.sql` used to add `'timeline'`: create a
--      scratch table with the same columns and the widened CHECK, copy **every
--      row** across, drop the old table and rename. **No row is lost**: the
--      `INSERT ... SELECT *` preserves all rows whatever their `kind`, and the
--      column order of the new table matches the migrated one exactly (the
--      original columns plus `kind` in last position, where
--      `ALTER TABLE ... ADD COLUMN` left it). Re-executing the file verbatim is
--      safe: the `llm_requests_new` scratch name only exists while the body
--      runs and disappears after the rename. There are no indexes, triggers or
--      views on `llm_requests` in this repo (checked across `migrations/` and
--      `src/`), so there is nothing to recreate after the rename.
--
--   2) The journal settings, with the seed idiom of
--      `20261010000004_timeline_prompts.sql`: idempotent, and it only
--      overwrites empty/NULL values so a user customisation is preserved.
--      `journal_prompt` seeds the **reading**: how the day went, what it cost
--      and what it made the author look forward to, written as a first-person
--      account that describes the already-registered facts instead of
--      enumerating them again, and that answers with the output contract the
--      worker validates (`{"write": false}` or the JSON object with `write`,
--      the story and the optional fields) — coherent with the minimal fallback
--      in `src/workers/journal.rs`
--      (`DEFAULT_JOURNAL_PROMPT_FALLBACK`). `journal_enabled` boots the worker
--      on and `journal_hour` sets the default cut at 04:00. The progress mark
--      `journal_last_judged_day` is deliberately **not** seeded: its absence
--      is the «never judged» state and the worker starts at the last closed
--      day.

-- 1) Widen `llm_requests.kind` by rebuilding the table.
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
                      CHECK (kind IN ('chat', 'router', 'archivist', 'consolidator', 'collapse', 'timeline', 'journal'))
);

INSERT INTO llm_requests_new SELECT * FROM llm_requests;

DROP TABLE llm_requests;

ALTER TABLE llm_requests_new RENAME TO llm_requests;

-- 2) Seed the journal settings. A present, non-empty value (a user
-- customisation) is never overwritten; an absent or empty one is filled.
INSERT INTO settings (key, value, updated_at)
VALUES ('journal_prompt', '# RELATO DE LA BITÁCORA
Eres el diario del autor. Recibes el material de una jornada: el día, la entrada anterior del diario, los hechos ya registrados del timeline y los mensajes del día.

Escribe el relato del día en primera persona, como lo contaría su propio autor: cómo fue el día, qué costó y qué ilusionó. Los hechos del día ya están registrados en el timeline: no los enumeres otra vez, descríbelos con tus palabras. Describe en vez de diagnosticar, y no inventes nada que no esté en el material.

Devuelve EXCLUSIVAMENTE un objeto JSON:
- Si la jornada no tiene nada que contar: {"write": false}
- Si hay algo que contar: {"write": true, "content": "el relato del día", "mood": "great|good|neutral|bad|terrible", "achievements": ["…"], "struggles": ["…"], "tags": ["…"]}

"write" es obligatorio y booleano. Con "write" en true, "content" es el relato en primera persona y no puede estar vacío. "mood", "achievements", "struggles" y "tags" son opcionales.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('journal_enabled', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('journal_hour', '4', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
