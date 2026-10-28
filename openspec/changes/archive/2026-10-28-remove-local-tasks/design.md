# Design: Retirar el TODO local y dejar las tareas en OpenTodo

## 1. Orden estricto de las operaciones

El borrado de la tabla es irreversible, así que el change se ejecuta en este orden y no en otro:

1. Escribir y probar el script de migración (`--dry-run` contra una copia de la BD).
2. Ejecutarlo contra la BD de producción **en solo lectura** para crear las tareas en OpenTodo.
3. Verificar en OpenTodo que están las 8 tareas y que no falta ningún campo.
4. Publicar el release (imagen nueva) y recrear el stack: la migración hace el `DROP`.
5. Verificar que `/api/tasks` ya no existe, que la herramienta funciona con token y que el header
   enlaza a OpenTodo.

Invertir 2 y 4 destruiría las 8 filas sin haberlas copiado. El script es, por tanto, **anterior** al
release, no parte de él.

## 2. Script de migración (una sola ejecución)

Script de una sola ejecución, escrito en `/tmp/opencode/migrate_local_tasks_to_opentodo.py` (fuera
del repositorio): Python 3 con biblioteca estándar (`sqlite3`, `urllib`), sin dependencias. Se
**borra al terminar la migración**, y lo ejecutado queda registrado en `plans/PLAN-011.md`.
Argumentos:

- `--db RUTA` — base de datos de Valet (por defecto la del volumen de compose:
  `/var/lib/docker/volumes/valet_valet_data/_data/valet.db`).
- `--dry-run` — imprime el mapeo y no escribe en OpenTodo.
- `--no-create-project` — si un `project` local no existe en OpenTodo, deja la tarea sin proyecto y
  avisa, en vez de crearlo.

Lee `opentodo_base_url` y `opentodo_token` de `settings` **de esa misma BD**, con
`sqlite3.connect("file:...?mode=ro", uri=True)` para no tocar el fichero. El token **nunca** se
imprime ni se escribe en logs: si hay que citarlo, se muestra `len(token)` y un `redact` de 4
caracteres. Autenticación: `Authorization: Bearer <token>` (el mismo contrato que
`src/services/opentodo.rs`).

### Mapeo de campos

| Valet (`tasks`) | OpenTodo | Nota |
|---|---|---|
| `content` | `title` | entero; `description` vacío |
| `priority: low` | `priority: 4` | `low → 4`, `medium → 3`, `high → 2` (la tabla local no tenía `critical`); siempre explícita porque el servidor asume 4 |
| `due_date` | `due: {date}` | `NULL` en las 8 filas actuales; si algún día trae hora, se manda `due: {date, time}` |
| `status: inbox` / `todo` | sin proyecto, abierta | —
| `status: doing` | etiqueta `doing` | —
| `status: waiting` | etiqueta `waiting` | 1 fila actual |
| `status: someday` | `start: "someday"` | —
| `status: done` | `completed_at` | ver §2.2 |
| `scope: shared` / `personal` | etiqueta homónima | las 8 son `shared` |
| `project` (texto libre) | `project_id` | resolución por nombre exacto contra `GET /api/v1/projects` |

### 2.1 Etiquetas y proyectos

Antes de crear tareas, el script hace `GET /api/v1/projects` y `GET /api/v1/labels` una sola vez. Si
falta una etiqueta (`shared`, `personal`, `doing`, `waiting`), la crea con `POST /api/v1/labels` en
el espacio por defecto (sin `workspace_id`, igual que hace la herramienta). Si ya existe una con ese
nombre, la **adopta**. Un proyecto que no exista se crea con `POST /api/v1/projects` salvo
`--no-create-project`. Decidido el 2026-10-28: `Preemer` **se crea** en OpenTodo (la información se
preserva). Las siete tareas «Continuar con …» **no** se agrupan en proyectos: se migran como tareas
sueltas en la bandeja de entrada, idénticas a como están hoy, y organizarlas será decisión del
usuario dentro de OpenTodo.

Las etiquetas se aplican con un `PATCH /api/v1/tasks/{id}` posterior que envía **el array completo**
(la lección de la fase 1: `labels` reemplaza, no fusiona).

### 2.2 Tareas completadas

Hoy no hay ninguna fila `done`, pero el script no puede ignorarlas. Creará la tarea y luego intentará
`PATCH /api/v1/tasks/{id}` con `completed_at` en milisegundos. Si el servidor lo rechaza, el script
**aborta** con el mensaje de error y no se despliega: no se inventa un endpoint de completado que no
se ha verificado contra la instancia real.

### 2.3 Idempotencia

`Idempotency-Key: valet-migrate-<uuid local>` en cada `POST /api/v1/tasks`. Un `409` significa «ya
aplicado» (comportamiento verificado en la fase 1: el servidor no devuelve la tarea original), y el
script lo contabiliza como migrada sin reintentar. Una segunda ejecución, por tanto, no duplica nada:
debe terminar diciendo `8 ya migradas, 0 pendientes`. Para poder decir eso sin el id remoto, el
script compara **títulos** de `GET /api/v1/tasks` antes de crear y omite los que ya estén.

### 2.4 Salida

Una tabla `id local → título → nuevo id remoto (o «ya estaba»)`, el recuento final y el número de
etiquetas/proyectos creados. Código de salida distinto de cero ante cualquier error HTTP, para que
pueda encadenarse en el despliegue.

## 3. Retirada del backend local

- Migración `20261028000001_remove_local_tasks.sql`: `DROP TABLE IF EXISTS tasks_fts;` y después
  `DROP TABLE IF EXISTS tasks;`. El orden importa: `tasks_fts` es una tabla FTS5 de contenido
  externo que apunta a `tasks`.
- Borrar `src/db/repos/tasks.rs`, `src/handlers/tasks.rs`, `src/routes/tasks.rs` y sus `pub mod`
  (`src/db/repos/mod.rs:16`, `src/handlers/mod.rs:13`, `src/routes/mod.rs:20`), el `merge` de
  `src/lib.rs:578` y el `pub mod tasks;` de `src/tools/mod.rs:17` **solo si** ya no hay nada que
  exportar desde `src/tools/tasks.rs` (la herramienta sigue existiendo, así que ese `mod` se queda).
- `src/db/repos/tasks.rs` era el único consumidor de `TasksRepo`; no queda huérfano.
- `src/routes/export.rs`: quitar `tasks` de `EXPORTABLE_TABLES` y del `json!` de `export_all_tables`.
- `src/db/repos/stats.rs:204`: quitar `"tasks"` de la lista de tablas de `db_sizes`.
- `src/bin/seed.rs`: borrar `seed_tasks`, su llamada en `main` y el `println` asociado.
- `src/tools/unified_search.rs`: quitar la dimensión `tasks` de las dos ramas del `match` y del enum
  `dimensions` del esquema JSON; el test que inserta en `tasks` se adapta a `notes`.

## 4. Herramienta `tasks` sin backend local

`src/tools/tasks.rs` conserva la firma, las operaciones, las vistas calculadas, `set_label`, los
límites y los permisos; desaparecen las ramas `else` que hablaban con `TasksRepo` y la función
`is_configured` deja de tener sentido como bifurcación: si no hay token, se devuelve

`ToolError::Execution("OpenTodo no está configurado: pega el token en Ajustes → Integraciones → OpenTodo")`.

El tool deja de mirar `profile_id` (el orquestador lo sigue inyectando a todas las herramientas; un
argumento de más se ignora) y `src/tools/tasks.rs` deja de importar `sqlx` y `Uuid`, lo que se
comprueba con `cargo clippy --all-targets -- -D warnings`.

## 5. Frontend

- Borrar `frontend/src/components/TaskView.tsx`, `TaskModal.tsx`, `task.constants.ts`,
  `frontend/src/hooks/useTasks.ts` y los tests `TaskView.test.tsx`, `TaskModal.test.tsx`,
  `useTasks.test.ts`.
- `AppLayout.tsx`: fuera el `lazy()` de `TaskView`, `tasksVisible`, el modal y el import del icono
  solo si deja de usarse (el icono se queda, ahora vive en el enlace). El botón pasa a

  ```tsx
  <Button
    type="text"
    href={OPENTODO_URL}
    target="_blank"
    rel="noopener noreferrer"
    icon={<CheckSquareOutlined />}
    style={{ color: "rgba(255,255,255,0.65)" }}
  />
  ```

  con `const OPENTODO_URL = "https://todo.territoriolinux.es";` en el propio `AppLayout.tsx`.
  Trade-off asumido: si algún día se cambia `opentodo_base_url` en Ajustes, el enlace del header no
  lo sigue. Se prefiere eso a un `GET /api/opentodo/status` en cada arranque solo para pintar un
  botón.
- `useMainChat.ts`: fuera el bloque de `tasks-changed`.
- `api/client.ts` y `types/index.ts`: fuera los cuatro métodos y el tipo `Task`.
- Los cinco `vi.mock("../components/TaskView")` de los tests de `AppLayout` se retiran junto con el
  fichero que mockean.

## 6. Specs

| Spec | Operación |
|---|---|
| `db/schema` | REMOVED «db/schema SHALL define the tasks table with GTD statuses»; ADDED el requisito de la migración que retira tabla y FTS |
| `db/repos` | REMOVED «TasksRepo SHALL provide CRUD for tasks»; MODIFIED «StatsRepo SHALL provide database table sizes» (sin `tasks`) |
| `tools/tasks` | REMOVED el requisito del fallback; ADDED «`tasks` SHALL operar exclusivamente sobre OpenTodo» |
| `tools/unified-search` | REMOVED el requisito de las cuatro dimensiones; ADDED el de tres |
| `frontend/tasks` | REMOVED los requisitos del tipo, los hooks, `TaskView` y su integración; ADDED el del enlace externo |

`openspec archive` no reescribe el `## Purpose` del fichero destino: tras archivar hay que ajustar a
mano el propósito de `openspec/specs/frontend/tasks/spec.md`, que hoy describe «dos vistas, Kanban y
Lista».

## 7. Release y despliegue

- `vampus upgrade --minor` → **v0.24.0** (en 0.x, quitar endpoints y UI es ruptura, y la convención
  del repo es `minor`). `git-cliff -o CHANGELOG.md --tag v0.24.0`.
- Rama `release/v0.24.0` → PR a `main`, tag y `image.yml`.
- En `co1`: copia de la BD a `/root/`, `docker compose pull && up -d` desde
  `~/docker/dockge/stacks/valet`, y verificación con `/api/health`.
- Rollback: restaurar la copia y volver a `:v0.23.0`. Las 8 tareas seguirían en OpenTodo y una
  reejecución del script no las duplicaría.

## 8. Riesgos

| Riesgo | Mitigación |
|---|---|
| Se despliega antes de migrar y se pierden las 8 filas | Orden fijado en §1; la migración es del operador, no del arranque |
| El script crea duplicados al reintentar | `Idempotency-Key` derivado del id local + comparación por título |
| `Preemer` no existe en OpenTodo | `POST /api/v1/projects` por defecto; `--no-create-project` para lo contrario |
| Una tarea `done` no se puede completar por API | el script aborta y se decide entonces; no se inventa un endpoint |
| El token deja de estar configurado tras el change | la herramienta sigue existiendo y devuelve un error legible con la ruta exacta de configuración |
| Enlaces externos abiertos con `window.opener` | `rel="noopener noreferrer"` |
| Un despliegue de desarrollo con datos locales | la migración los borra igual; es el comportamiento pactado (el TODO local desaparece) |
