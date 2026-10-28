# Tasks — remove-local-tasks

## 0. Fase 0 (spec de caracterización)

- [x] 0.1 Inventario as-is de todo lo que se retira, con spec y tests que lo fijan, en
      `characterization.md`. — Escrito el 2026-10-28; incluye los datos reales a migrar y los cuatro
      hallazgos (semillas rotas, `tasks_fts` sin triggers, `/api/tasks` sin spec, `profile_id`
      inyectado siempre).
- [x] 0.2 Comprobar la línea base: `cargo test`, `npx vitest run`, `openspec validate --all --strict`
      en verde **antes** de tocar nada. — 1947 tests backend y 644 frontend en verde; 62 specs
      válidas.

## 1. Migración de datos (antes del release)

- [x] 1.1 RED — script de una ejecución (`/tmp/opencode/migrate_local_tasks_to_opentodo.py`, fuera del
      repo) con `--dry-run` sobre una copia de la BD de producción: imprime las 8 filas mapeadas, no
      escribe nada y no imprime el token. — Verificado contra un snapshot del 2026-10-28: 8 filas, 2
      etiquetas (`shared`, `waiting`) y 1 proyecto (`Preemer`) por crear, y OpenTodo con las mismas 4
      tareas de ejemplo antes y después del `--dry-run`.
- [x] 1.2 GREEN — creación real: etiquetas (`shared`, `waiting`), proyecto (`Preemer`), `POST
      /api/v1/tasks` con `Idempotency-Key` y `PATCH` de etiquetas con el array completo. — 8 creadas,
      0 errores, 2 etiquetas y 1 proyecto creados.
- [x] 1.3 Verificar en OpenTodo: 8 tareas nuevas, títulos exactos, `waiting` en la de Miguel,
      `Preemer` creado como proyecto, prioridad 3, las siete «Continuar con …» sin proyecto y ninguna
      tarea duplicada al reejecutar con `--dry-run`. — OpenTodo pasa de 4 a 12 tareas; las 8 llegan
      con `priority 3`, `shared` en todas, `waiting` solo en la de Miguel, `Preemer` como proyecto y
      sin `start` ni `completed_at`. La segunda ejecución del `--dry-run` dice `8 ya estaban, 0
      pendientes`.
- [x] 1.4 Borrar el script y dejar constancia de la ejecución (fecha, recuento y resultado) en
      `plans/PLAN-011.md`: no queda ninguna herramienta de migración en el repositorio. — Script y
      snapshots (local y en `co1`) borrados el 2026-10-28.

## 2. Backend: retirada del TODO local

- [x] 2.1 RED — tests que fijan el comportamiento nuevo: `tasks` sin token devuelve `ToolError`
      mencionando la configuración; `unified_search` trata `dimensions: ["tasks"]` como «sin filtro»;
      el export no tiene clave `tasks`; `db_sizes` no lista `tasks`. — Los cuatro fallaron por
      aserción antes de implementar.
- [x] 2.2 GREEN — migración `20261028000001_remove_local_tasks.sql` (`DROP tasks_fts`, `DROP tasks`).
- [x] 2.3 GREEN — borrar `src/db/repos/tasks.rs`, `src/handlers/tasks.rs`, `src/routes/tasks.rs` y sus
      registros en `mod.rs` y `lib.rs` (más el target `[[test]] tasks` de `Cargo.toml`).
- [x] 2.4 GREEN — quitar las ramas locales de `src/tools/tasks.rs` y sustituirlas por el error de
      «OpenTodo no está configurado»; limpiar imports (`sqlx`, `Uuid`) y los parámetros muertos
      (`content`, `status`, `due_date`, `scope`).
- [x] 2.5 GREEN — `src/routes/export.rs` (clave y whitelist), `src/db/repos/stats.rs` (`db_sizes`),
      `src/bin/seed.rs` (`seed_tasks`), `src/tools/unified_search.rs` (dimensión).
- [x] 2.6 GREEN — borrar `tests/api/tasks.rs` y adaptar `tests/api/export.rs` (sin `tasks`).
- [x] 2.7 GREEN — tests de la migración que pide el delta de `db/schema`:
      `test_migrations_drop_local_tasks_tables` y `test_remove_local_tasks_migration_is_idempotent`
      en `tests/db/migrations.rs` (añadidos tras la auditoría, que detectó el escenario sin cobertura).

## 3. Frontend

- [x] 3.1 RED — test del header: el botón de tareas es un enlace a OpenTodo con `target="_blank"` y
      `rel="noopener noreferrer"`, sin modal ni `lazy()`.
- [x] 3.2 GREEN — `AppLayout.tsx` (enlace con `aria-label` y `title`), `useMainChat.ts` (sin
      `tasks-changed`), `client.ts` y `types/index.ts` (sin `Task` ni los cuatro métodos).
- [x] 3.3 GREEN — borrar `TaskView.tsx`, `TaskModal.tsx`, `task.constants.ts`, `useTasks.ts` y sus tres
      tests; retirar los cinco `vi.mock` de `TaskView` y el test que montaba la vista.
- [x] 3.4 GREEN — test del escenario «un turno del agente ya no refresca una vista inexistente» en
      `useMainChat.test.ts` (añadido tras la auditoría).

## 4. Refactor y verificación

- [x] 4.1 `grep -rn "TasksRepo\|api/tasks\|TaskView\|useTasks\|tasks_fts"` sin resultados fuera de las
      migraciones históricas y del propio change. — Solo queda `CREATE VIRTUAL TABLE … tasks_fts` en
      `migrations/20260925000001_initial.sql`, que es una migración ya aplicada y no se toca.
- [x] 4.2 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
      `npx tsc --noEmit`, `npm run lint:ci`, `npx vitest run`. — **1925 tests backend** (1 ignored
      preexistente) y **635 frontend**, todo en verde; `just check-all` limpio, incluido el
      presupuesto de bundle inicial (47,77 kB de 70).
- [x] 4.3 `openspec validate --all --strict` y archivar el change; ajustar a mano el `## Purpose` de
      `openspec/specs/frontend/tasks/spec.md`. — Archivado el 2026-10-28 (`+4 / ~1 / −8` requisitos);
      62 specs válidas. Ajustados también los propósitos de `db/schema`, `tools/tasks` y
      `frontend/tasks`, y las referencias a `TaskView` que quedaban en `frontend/notifications` y
      `frontend-lint`.

## 5. Cierre

- [ ] 5.1 Release `minor` → **v0.24.0**, CHANGELOG, PR a `main`, tag, imagen.
- [ ] 5.2 Desplegar en `co1` con copia previa de la BD y verificar `/api/health`, `/api/tasks` → 404,
      herramienta con token y enlace del header.
- [ ] 5.3 `AGENTS.md` §V (estado 16 con la fase 2 cerrada) y §VI (plan completado);
      `plans/PLAN-011.md` → `plans/PLAN-011-COMPLETED.md` con las tareas 8–10 cerradas.
