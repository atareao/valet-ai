-- Seed the skill-router configuration and the eight per-skill prompt fragments.
--
-- Same idiom as `20261003000001_generation_settings.sql` and
-- `20260929000001_prompts.sql`: the upsert only overwrites empty/NULL values so
-- user customisations are preserved, and it is idempotent.
--
-- The values are read on every turn (see `read_router_config` and
-- `read_skill_fragments` in `src/orchestrator/skill_router.rs`), so editing
-- them takes effect without a restart. The router boots disabled.
--
-- Router knobs and defaults:
--   ROUTER_ENABLED       false
--   ROUTER_MODEL         typesafe/jev-1.13
--   ROUTER_THRESHOLD     0.3
--   ROUTER_TIMEOUT_MS    800
--   ROUTER_HISTORY_TURNS 2
--
-- The eight `SKILL_<ID>_PROMPT` keys match the `prompt_key` of the closed
-- catalog in `src/orchestrator/skills.rs` and carry only non-obvious
-- operating guidance; their heading is the skill's `prompt_heading`.

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_ENABLED', 'false', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_MODEL', 'typesafe/jev-1.13', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_THRESHOLD', '0.3', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_TIMEOUT_MS', '800', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_HISTORY_TURNS', '2', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_AGENDA_PROMPT', '# SKILL ACTIVA: AGENDA
- Si el usuario pregunta por su disponibilidad o por huecos libres, resuélvelo con `check_availability`; no lo deduzcas a ojo desde la lista de eventos.
- `update_event` y `delete_event` exigen el `id`: localízalo con `get_events` en el mismo turno y no lo inventes.
- La duración va en minutos y el fin se calcula solo («hora y media» son 90); un día completo usa `all_day`, porque `create_event` no admite `end`.
- Convierte las fechas relativas («mañana a las 9») a ISO 8601 absoluto antes de llamar, y usa `rrule` solo para series recurrentes.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_TAREAS_PROMPT', '# SKILL ACTIVA: TAREAS
- Una tarea es algo pendiente de hacer, no una hora de aviso: si el usuario pide que le avisen a una hora, es un recordatorio (`reminders`).
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve el estado GTD. No uses `calendar` para un pendiente.
- `update_task`, `complete_task` y `delete_task` exigen el `id`: obtenlo con `list_tasks` en el mismo turno y no lo inventes.
- `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_RECORDATORIOS_PROMPT', '# SKILL ACTIVA: RECORDATORIOS
- Un recordatorio es un aviso puntual a una hora; si es algo pendiente de hacer, es una tarea (`tasks`), y si es una cita, un evento con `reminder_minutes_before`.
- `set_reminder` exige una fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar.
- `dismiss_reminder` y `snooze_reminder` necesitan el `id`: localízalo con `list_reminders`; el pospuesto se expresa en `snoozed_minutes`.
- Para un aviso que se repite (cada día, cada semana) usa un evento con `rrule`, no un recordatorio.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_NOTAS_PROMPT', '# SKILL ACTIVA: NOTAS
- Una nota es un dato que el usuario quiere conservar; si implica una acción pendiente, es una tarea (`tasks`).
- Elige la categoría con criterio: `fact` para un dato objetivo, `idea` para una ocurrencia y `journal` para una reflexión o un diario.
- `delete_note` exige el `id`: localízalo con `list_notes` antes de borrar y no lo inventes.
- Las `tags` van separadas por comas, para que la nota se pueda recuperar después por tema.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_CLIMA_PROMPT', '# SKILL ACTIVA: CLIMA
- Secuencia: si el usuario nombra una ciudad, resuelve sus coordenadas con `geocode` y consulta `weather` en el mismo turno; no le pidas a él la latitud y la longitud.
- Si no nombra lugar, parte de su ubicación guardada con `get_current_location` en vez de dar el tiempo de una ciudad genérica.
- Para una previsión pasa `date` en formato YYYY-MM-DD (convierte «mañana» a fecha absoluta); sin `date`, `weather` devuelve el tiempo actual.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_LUGARES_PROMPT', '# SKILL ACTIVA: LUGARES
- «¿Dónde estoy?» o «¿qué hay a mi alrededor?» se resuelven con la ubicación guardada (`get_current_location`), no con `search_places`.
- Para buscar cerca del usuario, pásale a `search_places` las coordenadas obtenidas antes (`get_current_location` o `geocode`) y un `radius`; sin coordenadas la búsqueda pierde el entorno.
- No confundas el sentido: `geocode` convierte una dirección o un lugar en coordenadas y `reverse_geocode` convierte coordenadas en una dirección.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_BUSQUEDA_WEB_PROMPT', '# SKILL ACTIVA: BÚSQUEDA WEB
- Resérvala para información externa o cambiante (noticias, precios, documentación); lo que vive en los datos del usuario se busca con la skill de memoria o con la herramienta propia de su dominio.
- Reformula la petición como palabras clave, no como la frase completa del usuario.
- Apoya la respuesta en la URL de los resultados y no completes con conocimiento previo lo que la búsqueda no confirme.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_MEMORIA_PROMPT', '# SKILL ACTIVA: MEMORIA
- Úsala cuando no se sepa en qué dimensión vive el dato o cuando se pregunte por algo ya hablado; si el dominio está claro (eventos, tareas o notas), la herramienta propia de esa skill acierta más.
- `unified_search` recorre también los mensajes: es la vía para recordar conversaciones anteriores, no solo datos guardados.
- Cuando ya se conozca el dominio, acótalo con `dimensions` para reducir ruido en lugar de buscar en todas.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
