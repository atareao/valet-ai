-- Seed the four ReAct tool-call guard limits into the `settings` table.
--
-- Same idiom as `20261003000001_generation_settings.sql`: the upsert only
-- overwrites empty/NULL values so user customisations are preserved, and it is
-- idempotent.
--
-- The values are read on every turn (see `src/orchestrator/tool_limits.rs`), so
-- editing them in the UI takes effect without a restart.
--
-- Defaults:
--   Fingerprint  2    executions allowed of an identical call
--   Failure      3    consecutive failures per `tool::operation`
--   Success      50   successful calls per `tool::operation`
--   Total        100  tool calls allowed in the whole turn

INSERT INTO settings (key, value, updated_at)
VALUES ('REACT_TOOL_FINGERPRINT_LIMIT', '2', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('REACT_TOOL_FAILURE_LIMIT', '3', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('REACT_TOOL_SUCCESS_LIMIT', '50', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('REACT_TOOL_TOTAL_LIMIT', '100', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '';
