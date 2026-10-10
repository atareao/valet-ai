-- Seed the three per-role generation knobs for the `timeline` extractor role.
--
-- Same idiom as `20261010000004_timeline_prompts.sql`: the upsert only
-- overwrites empty/NULL values so user customisations are preserved, and it is
-- idempotent. The timeline extractor turns a batch of messages into dated
-- atomic facts; it favours determinism (low temperature) over creativity.
--
-- The values are read on every LLM call (see `src/generation.rs`), so editing
-- them in the UI takes effect without a restart.
--
-- Role and defaults:
--   Timeline    0.2 / off   / 2048

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_TIMELINE_TEMPERATURE', '0.2', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_TIMELINE_REASONING', 'off', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('GENERATION_TIMELINE_MAX_TOKENS', '2048', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
