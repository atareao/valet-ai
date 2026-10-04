# Proposal

## Why

Cinco tools integradas no tienen spec viva: `reminders`, `tasks` y `notes` solo aparecen de pasada en
otros requisitos (registro o permisos), y `get_current_time` / `get_current_location` carecen de
cualquier requisito en `openspec/specs/`. Su comportamiento (operaciones, valores por defecto,
permisos, mensajes) vive solo en el código y sus tests, así que un cambio silencioso no lo detectaría
ninguna spec. Además, el delta archivado de `time-location` documenta un fallback **UTC** que el
código y sus tests contradicen (usan **Europe/Madrid**).

Esta es una tarea **Phase 0 (Legacy)**: especificar el comportamiento **actual** (caracterización), sin
cambiarlo.

## What Changes

- **Nueva capacidad `tools/reminders`**: operaciones `set_reminder`, `list_reminders`,
  `dismiss_reminder`, `snooze_reminder`; permiso `Notify`.
- **Nueva capacidad `tools/tasks`**: operaciones `list_tasks`, `add_task`, `update_task`,
  `complete_task`, `delete_task`; permiso `ExplicitApproval` solo para `delete_task`.
- **Nueva capacidad `tools/notes`**: operaciones `create_note`, `list_notes`, `delete_note`; permiso
  `NoConfirm` (se documenta la asimetría con `tasks`: `delete_note` no pide aprobación).
- **Nueva capacidad `tools/time-location`**: `get_current_time` (timezone de settings, fallback
  **Europe/Madrid**) y `get_current_location` (reverse geocoding de lat/lon vía Nominatim); corrige el
  UTC del delta archivado.

## Capabilities

### Added Capabilities

- `tools/reminders`, `tools/tasks`, `tools/notes`, `tools/time-location`.

## Impact

- Solo documentación (`openspec/specs/`). **Sin cambios de código.**
- Fuera de alcance (follow-up): requisitos propios para `RemindersRepo`/`TasksRepo`/`NotesRepo` en
  `db/repos` y para las tablas `reminders`/`tasks`/`notes` y las claves `timezone`/`latitude`/
  `longitude` en `db/schema`.
