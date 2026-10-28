# Change: Retirar el TODO local y dejar las tareas en OpenTodo

## Why

La fase 1 (`opentodo-integration`, archivada el 2026-10-27) dejó la herramienta `tasks` operando
contra OpenTodo **con fallback local**, de modo que desplegar no rompía nada si faltaba el token.
Ese despliegue ya ocurrió (v0.23.0 en `co1`), el usuario configuró el token y la E2E contra
producción está verificada. Con eso, el fallback ha dejado de ser una red de seguridad y se ha
convertido en el problema:

- **Dos fuentes de verdad.** La tabla `tasks` guarda 8 filas que el agente ya no lee ni escribe (con
  token, todas las operaciones van a OpenTodo) y la UI las sigue mostrando como si fueran las tareas
  del usuario.
- **Código muerto con mantenimiento.** Repositorio, handlers, rutas, ramas locales de la
  herramienta, semillas, export, `db_sizes`, dimensión `tasks` de `unified_search` y una vista del
  frontend que ya no se usa.
- **Una vista que miente.** `TaskView` (Kanban + lista) pinta estados GTD que ya no son la verdad: el
  usuario trabaja en `https://todo.territoriolinux.es`.

## What Changes

Este change es la **fase 2**: quita lo local y deja OpenTodo como único backend de tareas.

1. **Migración de datos, antes del release.** Las 8 filas de `tasks` se crean en OpenTodo con un
   script de una sola ejecución que lee la BD en solo lectura, resuelve el token del propio
   `settings`, es idempotente (clave derivada del id local) y tiene `--dry-run`. El recuento se
   verifica **antes** de desplegar y **el script se borra** al terminar, con el registro de lo hecho
   en `plans/PLAN-011.md`: el repositorio no gana una herramienta de migración permanente.
2. **Retirada del backend local**: migración `20261028000001_remove_local_tasks.sql` que hace
   `DROP TABLE IF EXISTS tasks_fts` y después `DROP TABLE IF EXISTS tasks`; se borran
   `src/db/repos/tasks.rs`, `src/handlers/tasks.rs`, `src/routes/tasks.rs` y su registro en
   `src/routes/mod.rs` y `src/handlers/mod.rs`, las ramas locales de `src/tools/tasks.rs`,
   `seed_tasks` de `src/bin/seed.rs`, la clave `tasks` de `src/routes/export.rs` y la entrada `tasks`
   de `StatsRepo::db_sizes`.
3. **`tasks` pasa a exigir OpenTodo**: sin token la herramienta devuelve un `ToolError` legible que
   pide configurarlo en Ajustes → Integraciones → OpenTodo. La exigencia de `profile_id` desaparece.
4. **Frontend**: se borran `TaskView`, `TaskModal`, `task.constants.ts`, `useTasks.ts`, sus tres
   ficheros de test, los métodos `listTasks`/`createTask`/`updateTask`/`deleteTask` del cliente, el
   tipo `Task`, el `lazy()` de `AppLayout`, el estado `tasksVisible`, su modal y el evento
   `tasks-changed` de `useMainChat`. En la misma plaza del header queda un **enlace externo** a
   OpenTodo (`target="_blank"` + `rel="noopener noreferrer"`).
5. **`unified_search`** pierde la dimensión `tasks` (quedan `messages`, `notes` y `events`).
6. **Specs y docs**: se retiran de `db/schema`, `db/repos`, `frontend/tasks`, `tools/tasks` y
   `tools/unified-search` los requisitos de lo que desaparece; se actualizan `README.es.md`,
   `AGENTS.md` §V y `plans/PLAN-011.md`.

## Impact

- **Specs afectadas**: `db/schema`, `db/repos`, `tools/tasks`, `tools/unified-search`,
  `frontend/tasks`.
- **Código afectado**: `src/db/repos/tasks.rs`, `src/handlers/tasks.rs`, `src/routes/tasks.rs`,
  `src/tools/tasks.rs`, `src/tools/unified_search.rs`, `src/routes/export.rs`,
  `src/db/repos/stats.rs`, `src/bin/seed.rs`, `src/lib.rs`, `src/routes/mod.rs`,
  `src/handlers/mod.rs`, `src/db/repos/mod.rs`, `src/tools/mod.rs`; en el frontend
  `AppLayout.tsx`, `useMainChat.ts`, `api/client.ts`, `types/index.ts` y los cinco ficheros de
  tareas; `README.es.md`.
- **Ruptura de contrato**: `GET/POST /api/tasks` y `PUT/DELETE /api/tasks/{id}` desaparecen. El
  único consumidor es el frontend empaquetado en la misma imagen; no hay clientes externos
  conocidos. Es la razón de que el release sea `minor` (v0.24.0) y no `patch`.
- **Irreversible en datos**: tras el `DROP TABLE` las 8 tareas solo existen en OpenTodo. De ahí el
  orden estricto: migrar → verificar → desplegar, con copia de la BD previa.
- **Fuera de alcance**: la fase 1 no se toca (ni el servicio `Opentodo`, ni los endpoints
  `/api/opentodo/*`, ni el fragmento `SKILL_PENDIENTES_PROMPT`, que ya habla solo de OpenTodo).
