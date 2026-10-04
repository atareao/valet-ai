# Spec Delta: db/repos

## ADDED Requirements

### Requirement: RemindersRepo SHALL provide CRUD for reminders

`RemindersRepo` SHALL exponer `create`, `find_by_id`, `list`, `dismiss`, `snooze` y `delete` sobre la
tabla `reminders`. `list` SHALL aceptar `profile_id` y un `status` opcional; `dismiss` y `snooze` SHALL
actualizar el estado y/o el `datetime`.

**Given** un pool SQLite
**When** se invoca `RemindersRepo`
**Then** DEBE operar sobre la tabla `reminders` y devolver `Reminder`/`Option<Reminder>`

#### Scenario: create inserta un recordatorio
**Given** un `Reminder` con id, profile_id, text, datetime y status
**When** se llama a `RemindersRepo::create`
**Then** DEBE insertarse la fila correspondiente

#### Scenario: list filtra por estado opcional
**Given** recordatorios en estados distintos
**When** se llama a `RemindersRepo::list` con `status = "pending"`
**Then** DEBE devolver solo los `pending`

#### Scenario: dismiss y snooze actualizan el estado
**Given** un recordatorio existente
**When** se llama a `RemindersRepo::dismiss` o `snooze`
**Then** DEBE actualizarse su `status` (`dismissed`/`snoozed`) y, en `snooze`, su `datetime`

### Requirement: TasksRepo SHALL provide CRUD for tasks

`TasksRepo` SHALL exponer `create`, `find_by_id`, `list`, `update`, `complete`, `cancel` y `delete`
sobre la tabla `tasks`. `list` SHALL aceptar filtros opcionales (`status`, `priority`, `project`,
`scope`) además de `profile_id`.

**Given** un pool SQLite
**When** se invoca `TasksRepo`
**Then** DEBE operar sobre la tabla `tasks` y devolver `Task`/`Option<Task>`

#### Scenario: create inserta una tarea
**Given** un `Task` con los campos requeridos
**When** se llama a `TasksRepo::create`
**Then** DEBE insertarse la fila

#### Scenario: complete marca la tarea como done
**Given** una tarea existente
**When** se llama a `TasksRepo::complete`
**Then** DEBE actualizarse su `status` a `done`

#### Scenario: update modifica campos seleccionados
**Given** una tarea existente
**When** se llama a `TasksRepo::update` con nuevos valores
**Then** DEBE persistir los cambios y actualizar `updated_at`

#### Scenario: delete elimina la tarea
**Given** una tarea existente
**When** se llama a `TasksRepo::delete`
**Then** DEBE eliminarse la fila

### Requirement: NotesRepo SHALL provide CRUD for notes

`NotesRepo` SHALL exponer `create`, `find_by_id`, `list`, `update` y `delete` sobre la tabla `notes`.
`list` SHALL aceptar `profile_id` y una `category` opcional.

**Given** un pool SQLite
**When** se invoca `NotesRepo`
**Then** DEBE operar sobre la tabla `notes` y devolver `Note`/`Option<Note>`

#### Scenario: create inserta una nota
**Given** una `Note` con content y category
**When** se llama a `NotesRepo::create`
**Then** DEBE insertarse la fila

#### Scenario: list filtra por categoría opcional
**Given** notas de varias categorías
**When** se llama a `NotesRepo::list` con una `category`
**Then** DEBE devolver solo las de esa categoría

#### Scenario: delete elimina la nota
**Given** una nota existente
**When** se llama a `NotesRepo::delete`
**Then** DEBE eliminarse la fila
