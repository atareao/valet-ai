-- Seed the twelve per-role generation knobs into the `settings` table.
--
-- Same idiom as `20261001000002_memory_settings.sql` and
-- `20260929000001_prompts.sql`: the upsert only overwrites empty/NULL values so
-- user customisations are preserved, and it is idempotent.
--
-- The values are read on every LLM call (see `src/generation.rs`), so editing
-- them in the UI takes effect without a restart.
--
-- Roles and defaults:
--   Chat        0.7 / ""    / 4096
--   Collapse    0.2 / off   / 1024
--   Memory      0.3 / off   / 1024
--   Semantic    0.1 / low   / 2048

-- Chat
INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_CHAT_TEMPERATURE', '0.7', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_CHAT_REASONING', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_CHAT_MAX_TOKENS', '4096', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

-- Collapse
INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_COLLAPSE_TEMPERATURE', '0.2', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_COLLAPSE_REASONING', 'off', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_COLLAPSE_MAX_TOKENS', '1024', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

-- Memory (fichas)
INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_MEMORY_TEMPERATURE', '0.3', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_MEMORY_REASONING', 'off', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_MEMORY_MAX_TOKENS', '1024', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

-- Semantic (consolidator / compression)
INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_SEMANTIC_TEMPERATURE', '0.1', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_SEMANTIC_REASONING', 'low', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_SEMANTIC_MAX_TOKENS', '2048', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';
