# Proposal

## Why

Las tablas y repos de `reminders`, `tasks` y `notes` no tienen requisitos propios: solo aparecen
listadas en `StatsRepo::db_sizes` (`db/repos`). Su contrato (métodos, filtros, enums de estado) y el de
las claves de `settings` que usan `get_current_time`/`get_current_location` (`timezone`, `latitude`,
`longitude`) viven solo en el código y las migraciones, así que un cambio de esquema o de firma no lo
detectaría ninguna spec.

Phase 0 (Legacy): se documenta el estado **actual**.

## What Changes

- **`db/repos` (ADDED)**: requisitos para `RemindersRepo`, `TasksRepo` y `NotesRepo` (métodos y filtros).
- **`db/schema` (ADDED)**: requisitos para las tablas `reminders`, `tasks` y `notes` (columnas, enums y
  la recreación GTD de `tasks`), y para las claves `timezone`/`latitude`/`longitude` de `settings`.

## Capabilities

### Modified Capabilities

- `db/repos`, `db/schema` (se añaden requisitos; no se tocan los existentes).

## Impact

- Solo documentación (`openspec/specs/`). **Sin cambios de código ni de migraciones.**
- Se apoya en la cobertura de las tools (`characterize-legacy-tools`, ya archivada).
