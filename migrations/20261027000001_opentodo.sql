-- Migración de la integración con OpenTodo (`opentodo-integration`).
--
-- Aditiva e idempotente, con el mismo idioma que las migraciones hermanas
-- (`20261026000001_zennotes_skill.sql`, `20261012000003_pendientes_notifications_prompt.sql`):
-- solo se rellena un valor ausente, vacío o NULL, así que una personalización
-- del usuario se conserva.
--
-- Why each block:
--
--   1) `opentodo_base_url`: la URL de la instancia de OpenTodo, con su valor por
--      defecto. Es un ajuste normal (no sensible): viaja y se edita por
--      `GET`/`PUT /api/settings`. Espeja el respaldo `OPENTODO_BASE_URL`.
--
--   2) `opentodo_token`: se siembra **vacío** a propósito; el usuario lo pega en
--      Ajustes → Integraciones → OpenTodo, de modo que el token nunca acaba en el
--      repositorio ni en el historial de migraciones. Es sensible
--      (`SENSITIVE_KEYS`), así que su única puerta de escritura es
--      `PUT /api/opentodo/token`. Espeja `OPENTODO_TOKEN`.
--
--   3) Promoción del fragmento `SKILL_PENDIENTES_PROMPT`: las tareas ya no viven
--      en la tabla local sino en OpenTodo, y el fragmento tiene que decírselo al
--      agente (vistas, prioridad 1–4, `someday`, etiquetas con `set_label`,
--      borrado definitivo) sin perder que los recordatorios son otra cosa. Solo
--      se promueve el valor **byte a byte** sembrado por `20261012000003`: una
--      edición del usuario se queda intacta. El texto nuevo conserva
--      `# SKILL ACTIVA: PENDIENTES` como primera línea (el `prompt_heading` que
--      usa el ensamblador de fragmentos para no duplicarlo).
--
--   4) Relleno del fragmento cuando la clave falta, está vacía o es NULL
--      (re-ejecutable vía `ON CONFLICT`), sin tocar un valor no vacío.

-- 1) URL base de OpenTodo. Solo se siembra cuando falta.
INSERT INTO settings (key, value, updated_at)
VALUES ('opentodo_base_url', 'https://todo.territoriolinux.es', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Token de OpenTodo: vacío a propósito; lo rellena el usuario en la UI.
INSERT INTO settings (key, value, updated_at)
VALUES ('opentodo_token', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) Solo se promueve el valor exacto sembrado por 20261012000003; un fragmento
--    editado por el usuario se conserva.
UPDATE settings SET value = '# SKILL ACTIVA: PENDIENTES
- Las tareas viven en **OpenTodo**, no en Valet: gestiónalas con la herramienta `tasks` y no mantengas una lista paralela.
- Distingue la forma: algo por hacer es una tarea (OpenTodo), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`). Los recordatorios de Valet siguen siendo otra cosa distinta de las tareas de OpenTodo: no los mezcles.
- Elige la vista con `view`: `inbox` (Bandeja de entrada, sin proyecto), `today` (Hoy, vence hoy o antes), `upcoming` (Próximo, vence después), `anytime` (En cualquier momento, sin fecha), `someday` (Algún día, `start: "someday"`), `doing` (En marcha) y `waiting` (En espera). Las vistas las calcula Valet sobre la lista completa: el servidor no filtra.
- La prioridad va de 1 a 4 y se escribe siempre explícita: `critical`→1, `high`→2, `medium`→3, `low`→4. Si no se dice nada, `low`.
- Para aparcar algo sin fecha usa `start: "someday"`; para darle fecha, `due` (ISO o lenguaje natural, que resuelve el servidor). Para agrupar, el proyecto.
- `doing`, `waiting`, `shared` y `personal` son **etiquetas**, no campos: se fijan con `set_label` indicando grupo y valor (`state`: `doing`|`waiting`, `scope`: `shared`|`personal`). Nunca mandes ids de etiqueta.
- Un aviso entrante (`notifications`) tampoco es un recordatorio: los recordatorios los pone el usuario, los avisos los envían otras aplicaciones (formularios, monitorización, scripts). Revísalos con `list_unread` y márcalos como leídos con `mark_read` cuando el usuario los atienda.
- `update_task`, `complete_task`, `delete_task` y `set_label` exigen el `id`: obtenlo con `list_tasks` o `search_tasks` en el mismo turno y no lo inventes.
- `delete_task` exige aprobación explícita; borra con lápida y `purge: true` es definitivo e irreversible.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.', updated_at = datetime('now')
 WHERE key = 'SKILL_PENDIENTES_PROMPT' AND value = '# SKILL ACTIVA: PENDIENTES
- Distingue la forma: algo por hacer es una tarea (`tasks`), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`).
- Un aviso entrante (`notifications`) tampoco es un recordatorio: los recordatorios los pone el usuario, los avisos los envían otras aplicaciones (formularios, monitorización, scripts). Revísalos con `list_unread` y márcalos como leídos con `mark_read` cuando el usuario los atienda.
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve su estado GTD. `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.
- `update_task`, `complete_task`, `delete_task`, `dismiss_reminder` y `snooze_reminder` exigen el `id`: obtenlo con `list_tasks` o `list_reminders` en el mismo turno y no lo inventes.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.';

-- 4) Relleno del fragmento ausente, vacío o NULL (re-ejecutable).
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_PENDIENTES_PROMPT', '# SKILL ACTIVA: PENDIENTES
- Las tareas viven en **OpenTodo**, no en Valet: gestiónalas con la herramienta `tasks` y no mantengas una lista paralela.
- Distingue la forma: algo por hacer es una tarea (OpenTodo), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`). Los recordatorios de Valet siguen siendo otra cosa distinta de las tareas de OpenTodo: no los mezcles.
- Elige la vista con `view`: `inbox` (Bandeja de entrada, sin proyecto), `today` (Hoy, vence hoy o antes), `upcoming` (Próximo, vence después), `anytime` (En cualquier momento, sin fecha), `someday` (Algún día, `start: "someday"`), `doing` (En marcha) y `waiting` (En espera). Las vistas las calcula Valet sobre la lista completa: el servidor no filtra.
- La prioridad va de 1 a 4 y se escribe siempre explícita: `critical`→1, `high`→2, `medium`→3, `low`→4. Si no se dice nada, `low`.
- Para aparcar algo sin fecha usa `start: "someday"`; para darle fecha, `due` (ISO o lenguaje natural, que resuelve el servidor). Para agrupar, el proyecto.
- `doing`, `waiting`, `shared` y `personal` son **etiquetas**, no campos: se fijan con `set_label` indicando grupo y valor (`state`: `doing`|`waiting`, `scope`: `shared`|`personal`). Nunca mandes ids de etiqueta.
- Un aviso entrante (`notifications`) tampoco es un recordatorio: los recordatorios los pone el usuario, los avisos los envían otras aplicaciones (formularios, monitorización, scripts). Revísalos con `list_unread` y márcalos como leídos con `mark_read` cuando el usuario los atienda.
- `update_task`, `complete_task`, `delete_task` y `set_label` exigen el `id`: obtenlo con `list_tasks` o `search_tasks` en el mismo turno y no lo inventes.
- `delete_task` exige aprobación explícita; borra con lápida y `purge: true` es definitivo e irreversible.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
