-- Seed the `timeline` skill: enablement flag, extractor prompt and fragment.
--
-- Same idiom as `20261010000002_skill_running_prompt.sql`: idempotent, and it
-- only overwrites empty/NULL values so a user customisation is preserved.
--
-- Why each block:
--
--   1) `ROUTER_SKILL_TIMELINE_ENABLED='true'`. The reader already treats a
--      missing key as enabled, but seeding the key makes the intent explicit and
--      gives the UI a row to edit. An existing value (a user who disabled the
--      skill) is never overwritten.
--
--   2) `SKILL_TIMELINE_PROMPT`. The key matches the `prompt_key` of the
--      `timeline` skill in the closed catalog (`src/orchestrator/skills.rs`) and
--      its first line is the skill's `prompt_heading`. It carries the operating
--      guidance for the three `timeline_*` tools.
--
--   3) `timeline_prompt`. The system prompt of the chronological extractor the
--      worker reads; it never writes a fact without dating it from the source
--      message's `created_at`.

-- 1) The timeline skill is enabled by default; a user value is never overwritten.
INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_TIMELINE_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Operating guidance for the timeline skill. Only seeded when missing.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_TIMELINE_PROMPT', '# SKILL ACTIVA: TIMELINE
- «¿Qué hice ayer / el martes / esta semana?» se responde con `timeline_get_events`, acotando el rango con `start` y `end` en ISO 8601.
- Los hechos son atómicos y ya están fechados: no los deduzcas de la memoria episódica ni los reconstruyas por similitud.
- Si el usuario te cuenta algo que hizo y quiere que quede registrado, anótalo con `timeline_add_event`; omite el `timestamp` para usar la hora actual.
- Borra con `timeline_delete_event` solo cuando el usuario lo pida y te confirme cuál; localiza el `id` con `timeline_get_events` en el mismo turno.
- Presenta la información de forma cronológica y responde siempre en español.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) The extractor's system prompt. Only seeded when missing.
INSERT INTO settings (key, value, updated_at)
VALUES ('timeline_prompt', '# EXTRACTOR DE HECHOS CRONOLÓGICOS
Recibes un lote de mensajes numerados de una conversación. Cada línea empieza por su número, su fecha (`created_at`) y su rol. Extrae los hechos atómicos que el usuario ha vivido, hecho o decidido.

Reglas:
- Un hecho es una sola acción o acontecimiento del pasado: «Fui al gimnasio», «Compré filtros de café», «Cerré el PR #168».
- NO extraigas preguntas del asistente, ni opiniones sin hecho, ni planes futuros sin confirmar.
- `source` es el número de la línea de la que sale el hecho. Es obligatorio y debe ser uno de los números que aparecen entre corchetes. No lo inventes.
- NO devuelvas ninguna fecha: la fecha del hecho la toma el sistema del mensaje de origen.
- Elige la categoría de entre: sport, lifestyle, work, health, shopping, social, system.
- Ignora el ruido: saludos, agradecimientos, peticiones de formato y mensajes sin contenido factual.
- Si el lote no contiene ningún hecho, devuelve una lista vacía.

Formato de salida (solo JSON):
{"events": [{"source": 3, "category": "sport", "fact": "Fui a correr 8 km por la vía verde"}]}', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
