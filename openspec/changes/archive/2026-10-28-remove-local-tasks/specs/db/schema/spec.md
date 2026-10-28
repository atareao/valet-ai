## REMOVED Requirements

### Requirement: db/schema SHALL define the tasks table with GTD statuses

**Reason**: las tareas viven en OpenTodo desde la fase 1 y la tabla local solo conserva 8 filas que
el agente ya no lee ni escribe. Este change migra esas filas a OpenTodo y retira la tabla y su
índice FTS.

**Migration**: las filas se crean en OpenTodo con un script de una sola ejecución (no versionado;
la constancia queda en `plans/PLAN-011.md`) **antes** de desplegar; la migración
`20261028000001_remove_local_tasks.sql` hace después `DROP TABLE IF EXISTS tasks_fts` y
`DROP TABLE IF EXISTS tasks`.

## ADDED Requirements

### Requirement: La migración SHALL retirar la tabla `tasks` y su índice FTS

La migración `20261028000001_remove_local_tasks.sql` SHALL ejecutar `DROP TABLE IF EXISTS tasks_fts`
**antes** de `DROP TABLE IF EXISTS tasks`, porque `tasks_fts` es una tabla FTS5 de contenido externo
que apunta a `tasks`. SHALL ser idempotente (`IF EXISTS`) y NO SHALL tocar ninguna otra tabla.

**Given** la base de datos con la migración aplicada
**When** se consulta `sqlite_master`
**Then** no existen `tasks` ni `tasks_fts`
**And** `events`, `notes`, `messages`, `reminders`, `memory`, `journal_entries` y `timeline_events`
siguen intactas

#### Scenario: Un despliegue repetido no falla

- **Given** una base de datos donde la migración ya se aplicó
- **When** se vuelve a ejecutar el arranque
- **Then** no devuelve error, porque ambos `DROP` llevan `IF EXISTS`
