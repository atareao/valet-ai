-- Skill selection (skill-selection): the skill is the selectable unit, so the
-- per-tool enablement and the global router threshold are retired.
--
-- Same idiom as `20261008000001_skill_router_tuning.sql` and
-- `20260929000001_prompts.sql`: the settings blocks are idempotent and never
-- overwrite a value the user has customised. The router knobs are read on every
-- turn, so editing them takes effect without a restart.
--
-- Why each block:
--
--   1) Seed `ROUTER_SKILL_<ID>_ENABLED='true'` for the six skills. The reader
--      already treats a missing key as enabled, but seeding the key makes the
--      intent explicit and gives the UI a row to edit. An existing value (a
--      user who disabled a skill) is never overwritten.
--
--   2) Materialise the effective per-skill threshold. The global
--      `ROUTER_THRESHOLD` disappears: a skill with no `ROUTER_THRESHOLD_<ID>`
--      falls back to its compiled catalog threshold. If the global had been
--      customised away from the seeded `0.10`, its value is copied to the five
--      routed domains that have no override yet (`agenda`, `pendientes`,
--      `recuerdos`, `entorno`, `web`), so their behaviour does not change when
--      the global key goes away. `widgets` already carries its own `0.20` and is
--      left untouched. When the global is still `0.10` (our default) no rows are
--      created: the compiled fallback already yields `0.10`. A non-numeric
--      global creates no rows either: it would only seed garbage.
--
--   3) Drop the retired global key. Its value was materialised in (2).
--
--   4) Drop the `enabled` column of `tools`: the per-tool toggle is gone and the
--      routing filter now lives at the skill level. `ALTER TABLE ... DROP
--      COLUMN` needs SQLite >= 3.35 (the bundled libsqlite3 is far newer). No
--      row of `tools` is lost: only the column goes.

-- 1) Every skill is enabled by default; a user value is never overwritten.
INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_AGENDA_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_PENDIENTES_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_RECUERDOS_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_ENTORNO_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_WEB_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_WIDGETS_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Materialise a customised global threshold into the domains without an
--    override. The source is the single `ROUTER_THRESHOLD` row; the five
--    domains come from an inline values list. Rows are only created when the
--    skill has no `ROUTER_THRESHOLD_<ID>` yet (never overwritten), and only
--    when the global is numeric (`GLOB '[0-9]*'`): an unreadable global must
--    not spawn garbage rows the reader would reject anyway.
INSERT INTO settings (key, value, updated_at)
SELECT 'ROUTER_THRESHOLD_' || upper(s.skill_id), g.value, datetime('now')
FROM settings AS g
CROSS JOIN (
    SELECT 'agenda' AS skill_id
    UNION ALL SELECT 'pendientes'
    UNION ALL SELECT 'recuerdos'
    UNION ALL SELECT 'entorno'
    UNION ALL SELECT 'web'
) AS s
WHERE g.key = 'ROUTER_THRESHOLD'
  AND g.value <> '0.10'
  AND g.value GLOB '[0-9]*'
  AND NOT EXISTS (
      SELECT 1
      FROM settings AS existing
      WHERE existing.key = 'ROUTER_THRESHOLD_' || upper(s.skill_id)
  );

-- 3) Retire the global threshold (its value was materialised above).
DELETE FROM settings WHERE key = 'ROUTER_THRESHOLD';

-- 4) Retire the per-tool enablement column.
ALTER TABLE tools DROP COLUMN enabled;
