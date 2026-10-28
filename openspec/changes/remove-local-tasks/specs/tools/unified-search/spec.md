## REMOVED Requirements

### Requirement: unified_search SHALL buscar en las dimensiones messages, notes, events y tasks

**Reason**: la dimensión `tasks` se apoyaba en `tasks_fts`, un índice que nunca se alimentó (no tiene
triggers) y que desaparece con la tabla `tasks`. Buscar en tareas corresponde ahora a la herramienta
`tasks` contra OpenTodo, que sí consulta el almacén real.

**Migration**: las tareas locales se migran a OpenTodo antes del release; `tasks_fts` se elimina con
el `DROP` de la migración `20261028000001_remove_local_tasks.sql`.

## ADDED Requirements

### Requirement: unified_search SHALL buscar en las dimensiones messages, notes y events

La herramienta SHALL consultar los índices FTS `messages_fts`, `notes_fts` y `events_fts`. Si no se
especifica `dimensions`, SHALL consultar las tres; si se especifica una, SHALL limitarse a ella. Un
valor de `dimensions` que no sea `messages`, `notes` o `events` SHALL tratarse como «sin filtro».

#### Scenario: Sin dimensión consulta las tres

- **Given** contenido indexado en mensajes, notas y eventos
- **When** se ejecuta `unified_search` solo con `query`
- **Then** los resultados pueden proceder de las tres dimensiones
- **And** cada resultado indica su origen en `source` (`message`, `note` o `event`)

#### Scenario: Con dimensión se limita a una

- **Given** contenido indexado en varias dimensiones
- **When** se ejecuta `unified_search` con `dimensions: "notes"`
- **Then** todos los resultados tienen `source = "note"`

#### Scenario: La dimensión `tasks` ya no existe

- **Given** una llamada con `dimensions: "tasks"`
- **When** se ejecuta `unified_search`
- **Then** no se consulta ninguna tabla FTS eliminada ni se produce un error de SQL
