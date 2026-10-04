# Design

## Context

- Tablas definidas en `migrations/20260925000001_initial.sql`; `tasks` se recrea en
  `migrations/20260926000001_gtd_statuses.sql` con estados GTD y `DEFAULT 'inbox'`.
- Repos en `src/db/repos/{reminders,tasks,notes}.rs` (SQL crudo con `sqlx`).
- `settings` es una tabla clave/valor (`key`, `value`, `updated_at`); las claves `timezone`,
  `latitude` y `longitude` las consumen `current_time.rs` y `current_location.rs`.

## Goals / Non-Goals

- **Goal:** fijar el contrato observable de esos repos/tablas tal como está hoy.
- **Non-Goal:** cambiar el esquema, normalizar enums ni tocar migraciones.

## Decisions

- **ADDED, no MODIFIED.** Los requisitos nuevos se añaden a `db/repos`/`db/schema` sin tocar los
  existentes (evita el riesgo de perder escenarios en el archive).
- **Documentar la recreación GTD.** La spec refleja el estado final (`inbox`…`done`), mencionando la
  migración que lo introduce, no el estado intermedio `pending`/`completed`/`cancelled`.

## Risks / Trade-offs

- [Congelar un esquema mejorable] → Caracterizar no es aprobar; cualquier cambio va en otro change.

## Migration Plan

Sin migración. Solo specs.
