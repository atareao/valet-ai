-- Seed the `running` skill: enablement flag and prompt fragment.
--
-- Same idiom as `20261009000002_skill_selection.sql` and
-- `20261007000001_skill_router.sql`: idempotent, and it only overwrites
-- empty/NULL values so a user customisation is preserved.
--
-- Why each block:
--
--   1) `ROUTER_SKILL_RUNNING_ENABLED='true'`. The reader already treats a
--      missing key as enabled, but seeding the key makes the intent explicit and
--      gives the UI a row to edit. An existing value (a user who disabled the
--      skill) is never overwritten.
--
--   2) `SKILL_RUNNING_PROMPT`. The key matches the `prompt_key` of the `running`
--      skill in the closed catalog (`src/orchestrator/skills.rs`) and its first
--      line is the skill's `prompt_heading`. The fragment carries only
--      non-obvious operating guidance for the four read-only Strava tools; the
--      values are read on every turn, so editing them takes effect without a
--      restart.

-- 1) The running skill is enabled by default; a user value is never overwritten.
INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_RUNNING_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Operating guidance for the running skill. Only seeded when missing.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_RUNNING_PROMPT', '# SKILL ACTIVA: RUNNING
- Empieza por `strava_recent_activities` para saber qué ha corrido el usuario (por fecha, página o deporte); usa `before`/`after` en ISO 8601 para acotar el periodo.
- Cuando haya que analizar una sesión concreta, `strava_activity_detail` da el resumen con vueltas y splits, y `strava_activity_streams` da las series (ritmo, frecuencia cardíaca, cadencia y altitud) para ver cómo evolucionó.
- Para totales de atleta —totales del año, últimas cuatro semanas y récords— usa `strava_athlete_stats`; no sumes a mano las actividades recientes.
- Expresa distancias en kilómetros y el ritmo en minutos por kilómetro (min/km); traduce la velocidad en m/s a ese formato antes de responder. Responde siempre en español.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
