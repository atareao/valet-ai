# Spec Delta: db/schema

## ADDED Requirements

### Requirement: db/schema SHALL define the reminders table

La tabla `reminders` SHALL tener las columnas `id` (PK), `profile_id` (FK a `profiles(id)`), `text`,
`datetime`, `status` (enum `pending`/`dismissed`/`snoozed`, por defecto `pending`) y `created_at`.

**Given** la migración inicial aplicada
**When** se inspecciona el esquema
**Then** DEBE existir la tabla `reminders` con esas columnas y el CHECK de `status`

#### Scenario: El CHECK de status restringe los valores
**Given** la tabla `reminders`
**When** se intenta insertar `status` distinto de `pending`/`dismissed`/`snoozed`
**Then** la inserción DEBE fallar por el CHECK

### Requirement: db/schema SHALL define the tasks table with GTD statuses

La tabla `tasks` SHALL tener `id` (PK), `profile_id` (FK), `content`, `status` (enum GTD
`inbox`/`todo`/`doing`/`waiting`/`someday`/`done`, por defecto `inbox`), `priority`
(`low`/`medium`/`high`, por defecto `medium`), `project`, `due_date`, `scope`
(`shared`/`personal`, por defecto `shared`), `created_at` y `updated_at`. El estado final proviene de
la migración `20260926000001_gtd_statuses`, que recrea la tabla y mapea los estados antiguos.

**Given** las migraciones aplicadas
**When** se inspecciona `tasks`
**Then** DEBE tener el CHECK GTD y `DEFAULT 'inbox'`

#### Scenario: El CHECK GTD admite los seis estados
**Given** la tabla `tasks`
**When** se inserta con `status` en `inbox`/`todo`/`doing`/`waiting`/`someday`/`done`
**Then** la inserción DEBE aceptarse

#### Scenario: La migración GTD mapea estados antiguos
**Given** una fila previa con `status = 'completed'`
**When** se aplica `20260926000001_gtd_statuses`
**Then** su `status` DEBE quedar como `done`

### Requirement: db/schema SHALL define the notes table

La tabla `notes` SHALL tener `id` (PK), `profile_id` (FK), `content`, `category`
(`idea`/`journal`/`fact`/`todo`, por defecto `idea`), `tags`, `created_at` y `updated_at`, y SHALL
mantener los triggers FTS (`notes_ai`/`notes_ad`/`notes_au`) sobre `notes_fts`.

**Given** las migraciones aplicadas
**When** se inspecciona `notes`
**Then** DEBE tener el CHECK de `category` y los triggers FTS

#### Scenario: El CHECK de category restringe los valores
**Given** la tabla `notes`
**When** se inserta una `category` fuera del enum
**Then** la inserción DEBE fallar

#### Scenario: Los triggers mantienen el índice FTS
**Given** la tabla `notes` y `notes_fts`
**When** se inserta, actualiza o borra una nota
**Then** los triggers DEBEN sincronizar `notes_fts`

### Requirement: settings SHALL store the timezone and location keys

La tabla `settings` (clave/valor con `updated_at`) SHALL almacenar, entre otras, las claves
`timezone` (zona IANA para `get_current_time`), `latitude` y `longitude` (coordenadas para
`get_current_location`).

**Given** la tabla `settings`
**When** se leen `timezone`, `latitude` y `longitude`
**Then** DEBEN existir como claves con valor textual cuando estén configuradas

#### Scenario: Lectura de las claves de hora y ubicación
**Given** `settings` con `timezone`, `latitude` y `longitude`
**When** se consultan esas claves
**Then** DEBE devolverse su valor textual
