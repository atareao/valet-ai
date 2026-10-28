## REMOVED Requirements

### Requirement: TasksRepo SHALL provide CRUD for tasks

**Reason**: el repositorio local de tareas desaparece con la tabla `tasks`. El almacén de tareas es
OpenTodo, y el acceso pasa por `src/services/opentodo.rs` más la herramienta `tasks`; no queda
ningún consumidor de `TasksRepo`.

**Migration**: las 8 filas existentes se migran a OpenTodo antes del release con un script de una
sola ejecución (no versionado; la constancia queda en `plans/PLAN-011.md`). El código de
`src/db/repos/tasks.rs` se borra en este change.

## MODIFIED Requirements

### Requirement: StatsRepo SHALL provide database table sizes

El repositorio SHALL exponer `StatsRepo::db_sizes(pool)` devolviendo el número de filas de cada tabla.

**Given** una base de datos con tablas pobladas
**When** se llama a `StatsRepo::db_sizes(pool)`
**Then** devuelve `Vec<TableSize>` con nombre de tabla y row count para:
events, llm_requests, memory, message_embeddings, messages, notes, profiles, reminders, settings,
timeline_events, tools

#### Scenario: Tablas con datos
**Given** 10 messages, 2 profiles, 5 memories
**When** `StatsRepo::db_sizes(pool)`
**Then** messages=10, profiles=2, memories=5, resto=0

#### Scenario: El timeline sale en el recuento de tablas
**Given** una base de datos con 7 hechos en `timeline_events`
**When** `StatsRepo::db_sizes(pool)`
**Then** el resultado incluye una entrada con `table = "timeline_events"` y `rows = 7`

#### Scenario: Una tabla del timeline vacía también aparece
**Given** una base de datos recién migrada, con `timeline_events` sin filas
**When** `StatsRepo::db_sizes(pool)`
**Then** el resultado incluye `table = "timeline_events"` con `rows = 0`

#### Scenario: `tasks` ya no aparece en el recuento
**Given** una base de datos con el change aplicado
**When** `StatsRepo::db_sizes(pool)`
**Then** ninguna entrada tiene `table = "tasks"`
