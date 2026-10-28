# Fase 0 — Caracterización del TODO local (as-is)

Escrito el 2026-10-28, **antes** de tocar una línea de código, como exige la Fase 0 del protocolo del
repositorio. Su objetivo es fijar qué hace hoy el TODO local, qué lo documenta y qué lo prueba, para
que la retirada sea verificable y no aparezca a mitad del change una pieza sin inventariar.

## Inventario

| Pieza | Dónde | Spec publicada | Tests que la fijan |
|---|---|---|---|
| Tabla `tasks` (CHECK GTD de 6 estados, `DEFAULT 'inbox'`) | `migrations/20260925000001_initial.sql:100`, recreada por `migrations/20260926000001_gtd_statuses.sql` | `openspec/specs/db/schema/spec.md` → «db/schema SHALL define the tasks table with GTD statuses» (con escenario del mapeo de estados antiguos) | Ninguno directo: la migración se ejerce en el arranque de la BD en toda la suite |
| Índice FTS `tasks_fts` (`content=tasks`) | `migrations/20260925000001_initial.sql:68` | **ninguna** | **ninguno** |
| `TasksRepo` (`create`, `find_by_id`, `list`, `update`, `complete`, `cancel`, `delete`) | `src/db/repos/tasks.rs` (365 líneas) | `openspec/specs/db/repos/spec.md` → «TasksRepo SHALL provide CRUD for tasks» | 7 `#[tokio::test]` en el propio fichero |
| API `GET/POST /api/tasks` y `PUT/DELETE /api/tasks/{id}` | `src/routes/tasks.rs` (17 líneas), `src/handlers/tasks.rs` (115) | **ninguna**: los endpoints nunca se especificaron | `tests/api/tasks.rs` (287 líneas): listado vacío, creación con defaults (`inbox`/`medium`/`shared`), listado tras crear, filtros por `status`/`priority`/`project`, actualización, borrado y 404 |
| Backend local de la herramienta (las seis operaciones sobre `TasksRepo`, con `profile_id` obligatorio) | `src/tools/tasks.rs:749-880` | `openspec/specs/tools/tasks/spec.md` → «`tasks` SHALL operar sobre OpenTodo y conservar el backend local solo como fallback» | 39 tests en el propio fichero (doble backend, argumentos, permisos) |
| `unified_search` con dimensión `tasks` | `src/tools/unified_search.rs:63`, `:68`, `:116` (más una inserción de ejemplo en `:182`) | `openspec/specs/tools/unified-search/spec.md` | tests del propio fichero |
| Export `/api/export` con la clave `tasks` | `src/routes/export.rs:14` y `:34` | **ninguna** | `tests/api/export.rs:29`: `assert!(data.get("tasks").is_some(), "missing tasks")` |
| `StatsRepo::db_sizes` con la entrada `tasks` | `src/db/repos/stats.rs:204` | `openspec/specs/db/repos/spec.md` → «StatsRepo SHALL provide database table sizes» | tests del propio fichero |
| Semillas `seed_tasks` | `src/bin/seed.rs:83-112` | **ninguna** | — |
| `TaskView` (Kanban + lista), `TaskModal`, `task.constants.ts`, `useTasks` | `frontend/src/components/`, `frontend/src/hooks/` | `openspec/specs/frontend/tasks/spec.md` (4 requisitos) | `frontend/src/test/TaskView.test.tsx`, `TaskModal.test.tsx`, `useTasks.test.ts` |
| Integración en `AppLayout` (botón, `lazy`, estado, modal) | `frontend/src/components/AppLayout.tsx:24-26`, `:91`, `:219-232` | `openspec/specs/frontend/tasks/spec.md` → «TaskView integrated in AppLayout» | `AppLayout.test.tsx`, `AppLayout.journal.test.tsx`, `AppLayout.notes.test.tsx` y `src/test/AppLayout.{notifications,logout}.test.tsx`, todos con `vi.mock` de `TaskView` |
| Evento `tasks-changed` | `frontend/src/hooks/useMainChat.ts:291-294` | `openspec/specs/frontend/tasks/spec.md` | no tiene test propio; lo consume `useTasks.ts` |
| Cliente HTTP y tipo del frontend (`listTasks`, `createTask`, `updateTask`, `deleteTask`, `interface Task`) | `frontend/src/api/client.ts:163-179`, `frontend/src/types/index.ts:203` | `openspec/specs/frontend/tasks/spec.md` → «Task type and API client» | `useTasks.test.ts` |

## Datos reales a migrar (BD de producción, leída en solo lectura el 2026-10-28)

8 filas, todas con `scope = 'shared'`, `priority = 'medium'` y `due_date = NULL`:

| `content` | `status` | `project` |
|---|---|---|
| Recibir de Miguel (Preemer) el documento para introducir en la app | `waiting` | `Preemer` |
| Conector de publicación para WordPress (REST API) — permitir publicar respuestas a comentarios desde el asistente | `inbox` | — |
| Responder a Enrique (comentario en atareao.es sobre el podcast del WAF casero): recomendarle VPS alternativo a IONOS tras la subida de renovación — pendiente mientras se resuelve el conector de publicación de WordPress | `inbox` | — |
| Continuar con Shuul | `inbox` | — |
| Continuar con podtrack | `inbox` | — |
| Continuar con kesto | `inbox` | — |
| Continuar con Valet | `inbox` | — |
| Continuar con id3cli | `inbox` | — |

## Hallazgos (as-is; no se arreglan aquí)

1. **`seed_tasks` ya está roto.** Inserta `status = 'pending'` y `status = 'completed'`, valores que
   el CHECK de `20260926000001_gtd_statuses` rechaza desde el 2026-09-26. Nadie lo notó porque las
   semillas no se ejecutan en el arranque. Se retira con el resto; no se arregla.
2. **`tasks_fts` es un índice vacío.** Se crea en la migración inicial y nadie lo alimenta: no hay
   triggers en `src/db/fts.rs` (solo crea los de `messages` y `notes`), ni escrituras en el
   repositorio ni en los handlers. `unified_search` con `dimensions: ["tasks"]` devuelve siempre
   `Found 0 results`. Retirarlo no pierde datos.
3. **`/api/tasks` no tiene spec publicada.** Su contrato vive solo en `tests/api/tasks.rs`; al
   desaparecer el endpoint, ese fichero es la única huella del contrato.
4. **`profile_id` en la herramienta.** El orquestador lo inyecta en *todas* las llamadas
   (`src/orchestrator/agent.rs:1581`), así que retirar la exigencia en `tasks` no obliga a tocar el
   orquestador: un argumento de más se ignora.

## Conclusión de la Fase 0

La conducta local está caracterizada por spec **y** tests en todas las piezas menos tres:
`/api/tasks`, el export y las semillas. **No se publica una spec nueva** para esas tres: lo
caracterizado se retira en este mismo change y una spec publicada que muere en el archivo sería
ruido permanente. Este inventario queda como artefacto de la Fase 0, y la retirada se verifica con
la lista de `tasks.md` más una búsqueda global (`grep -rn "TasksRepo\|api/tasks\|TaskView\|useTasks\|tasks_fts"`)
que debe quedar sin referencias fuera de `openspec/changes/archive/` y de este mismo change.
