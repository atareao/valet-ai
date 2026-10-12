-- Add the `notifications` operating guidance to the `pendientes` fragment.
--
-- The `notifications` tool (operations `list_unread` / `mark_read`) joined the
-- `pendientes` skill in the `notification-ingest` change, but the seeded
-- fragment (`SKILL_PENDIENTES_PROMPT`) only talks about `tasks` and
-- `reminders`. Without a line about it, the routed agent sees the tool in the
-- catalog but never learns when to reach for it, so an incoming notice gets
-- mistaken for a user-made reminder.
--
-- Same idiom as `20261010000004_timeline_prompts.sql`: idempotent, and it never
-- overwrites a value the user has customised. This is a new migration instead
-- of editing `20261008000001_skill_router_tuning.sql`: that file is already
-- applied on every existing database, so the sqlx `Migrator` would never re-run
-- it and the line would never reach the fragment.
--
-- Why each block:
--
--   1) Promote the fragment to the new text, but only when it is still
--      byte-for-byte the value seeded by `20261008000001`. A user edit is left
--      untouched. The new value keeps `# SKILL ACTIVA: PENDIENTES` as its first
--      line (the skill's `prompt_heading`), so the fragment assembler's
--      anti-duplication rule still detects it.
--
--   2) Backfill an absent, empty or NULL value with the full new fragment
--      (re-run safe via `ON CONFLICT`), without touching a non-empty value.

-- 1) Only the exact seeded value is promoted; an edited fragment is preserved.
UPDATE settings SET value = '# SKILL ACTIVA: PENDIENTES
- Distingue la forma: algo por hacer es una tarea (`tasks`), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`).
- Un aviso entrante (`notifications`) tampoco es un recordatorio: los recordatorios los pone el usuario, los avisos los envían otras aplicaciones (formularios, monitorización, scripts). Revísalos con `list_unread` y márcalos como leídos con `mark_read` cuando el usuario los atienda.
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve su estado GTD. `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.
- `update_task`, `complete_task`, `delete_task`, `dismiss_reminder` y `snooze_reminder` exigen el `id`: obtenlo con `list_tasks` o `list_reminders` en el mismo turno y no lo inventes.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.', updated_at = datetime('now')
 WHERE key = 'SKILL_PENDIENTES_PROMPT' AND value = '# SKILL ACTIVA: PENDIENTES
- Distingue la forma: algo por hacer es una tarea (`tasks`), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`).
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve su estado GTD. `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.
- `update_task`, `complete_task`, `delete_task`, `dismiss_reminder` y `snooze_reminder` exigen el `id`: obtenlo con `list_tasks` o `list_reminders` en el mismo turno y no lo inventes.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.';

-- 2) Seed the full fragment when the key is absent, empty or NULL (re-run safe).
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_PENDIENTES_PROMPT', '# SKILL ACTIVA: PENDIENTES
- Distingue la forma: algo por hacer es una tarea (`tasks`), un aviso puntual a una hora es un recordatorio (`reminders`) y una cita con aviso previo es un evento (`calendar`, con `reminder_minutes_before`).
- Un aviso entrante (`notifications`) tampoco es un recordatorio: los recordatorios los pone el usuario, los avisos los envían otras aplicaciones (formularios, monitorización, scripts). Revísalos con `list_unread` y márcalos como leídos con `mark_read` cuando el usuario los atienda.
- Las tareas nacen en `inbox`; `complete_task` las pasa a `done` y `update_task` mueve su estado GTD. `due_date` es una fecha límite informativa: no dispara ningún aviso por sí sola.
- `update_task`, `complete_task`, `delete_task`, `dismiss_reminder` y `snooze_reminder` exigen el `id`: obtenlo con `list_tasks` o `list_reminders` en el mismo turno y no lo inventes.
- `set_reminder` exige fecha y hora absolutas: convierte «en media hora» o «mañana» a ISO 8601 antes de llamar. Para un aviso que se repite, usa un evento con `rrule`, no un recordatorio.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
