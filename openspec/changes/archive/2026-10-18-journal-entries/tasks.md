# Tasks

TDD estricto (rojo → verde → refactor), en el orden en que se implementará. Comandos de verificación
de este repositorio: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`,
`npx vitest run`, `npx tsc --noEmit`, `npm run lint`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests. **No se escribe una sola línea de código ni de tests antes de la aprobación.**

## 1. RED — migración y repositorio

- [x] 1.1 RED — test de esquema: `run_migrations()` crea `journal_entries` con sus columnas, con `mood`/`achievements`/`struggles`/`tags` nullable, y con el índice `(profile_id, date DESC)`. **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — test: la restricción `UNIQUE(profile_id, date)` rechaza la segunda entrada del mismo perfil y día, y admite el mismo día en perfiles distintos; la migración es idempotente.
- [x] 1.3 RED — test del modelo: `Mood` se serializa en `snake_case`; un valor fuera del conjunto (`"genial"`) no deserializa; una entrada con `mood` ausente es válida; los campos JSON `NULL` se leen como listas vacías. **Pegar la salida en rojo en el informe.**
- [x] 1.4 RED — tests de `JournalRepo`: `upsert` crea y reemplaza sin duplicar (una sola fila, `updated_at` cambia y `created_at` se conserva); `get` por `(profile_id, date)` devuelve la entrada o `None`; `list` por rango inclusivo, de más reciente a más antigua y con tope; `delete` borra y devuelve `false` para lo inexistente.
- [x] 1.5 RED — confirmar por CLI que solo fallan los tests nuevos y que el resto sigue en verde (`cargo test`).

## 2. GREEN — migración y repositorio

- [x] 2.1 GREEN — crear `migrations/20261018000001_journal_entries.sql` (idempotente, `CREATE TABLE IF NOT EXISTS` + índice `IF NOT EXISTS`) y verificar 1.1–1.2 en verde.
- [x] 2.2 GREEN — `src/models/journal.rs` (o el módulo de modelos donde vivan los suyos): `JournalEntry` y `enum Mood` con su serialización estricta; `src/db/repos/journal.rs`: `JournalRepo` con `upsert`/`get`/`list`/`delete`, más `pub mod journal` en `src/db/repos/mod.rs`. Verificar 1.3–1.4 en verde.
- [x] 2.3 `cargo check` y `cargo clippy -- -D warnings` sin warnings en el módulo nuevo.

## 3. RED — herramienta `journal` y enrutado

- [x] 3.1 RED — tests de `journal.get_entry`: con fecha devuelve la entrada, sin fecha devuelve la de hoy, y un día sin entrada informa sin fallar. **Pegar la salida en rojo en el informe.**
- [x] 3.2 RED — tests de `journal.write_entry`: crea la del día y la sobrescribe sin duplicar; una entrada sin `mood` se guarda igual.
- [x] 3.3 RED — tests de `journal.update_entry` (solo toca lo que le llega) y `journal.list_entries` (rango inclusivo, más reciente primero y con tope).
- [x] 3.4 RED — tests de `journal.delete_entry`: borra la del día y no falla si no existe.
- [x] 3.5 RED — tests de validación: fecha mal formada y `mood` fuera del conjunto devuelven `ToolError::InvalidArguments` sin tocar la base de datos.
- [x] 3.6 RED — tests de permiso: `NoConfirm` para `get_entry`/`write_entry`/`update_entry`/`list_entries` y `ExplicitApproval` para `delete_entry`.
- [x] 3.7 RED — test de definición: `journal` figura en `definitions()` con las cinco operaciones, esquema objeto, descripciones en español y documentación de los obligatorios por operación. **Pegar la salida en rojo en el informe.**
- [x] 3.8 RED — test de integridad de skills: `journal` pertenece a la skill `recuerdos` (y a ninguna otra), y las `criteria` de `recuerdos` incluyen el habla de la bitácora («bitácora», «diario», «cómo fue mi día»).
- [x] 3.9 RED — medir el enrutado **antes** de tocar los criterios con el arnés `src/bin/route-eval.rs` sobre una muestra de turnos que pidan leer o apuntar el diario, y dejar la salida en el informe.

## 4. GREEN — herramienta `journal` y enrutado

- [x] 4.1 GREEN — `src/tools/journal.rs` con `JournalTool` (cinco operaciones, validación estricta, permisos por operación, descripciones en español), más `pub mod journal` en `src/tools/mod.rs`; registrar la herramienta en `build_tool_registry` (`src/lib.rs`). Verificar 3.1–3.7 en verde.
- [x] 4.2 GREEN — `src/orchestrator/skills.rs`: `recuerdos` cubre `journal` y sus `criteria` ganan el habla de la bitácora. Verificar 3.8 en verde.
- [x] 4.3 Medir el enrutado **después** con `src/bin/route-eval.rs` y comprobar que la cobertura no empeora (comparar con 3.9) y que los turnos de bitácora seleccionan `recuerdos`. **Pegar la comparativa antes/después en el informe.**
- [x] 4.4 `cargo test` (suite completa) en verde y `cargo clippy -- -D warnings` sin warnings.

## 5. RED — endpoints

- [x] 5.1 RED — test de `GET /api/journal`: devuelve el rango de más reciente a más antigua y respeta el `limit`; sin sesión responde `401`. **Pegar la salida en rojo en el informe.**
- [x] 5.2 RED — test de `GET /api/journal/{date}`: `200` con la entrada o `404` si ese día no existe; una fecha mal formada responde `400`.
- [x] 5.3 RED — test de `PUT /api/journal/{date}`: crea la del día y reemplaza la del mismo día sin duplicar; `content` ausente responde `400`; un `mood` fuera del conjunto responde `400` sin escribir; una entrada sin ánimo responde `200` con `mood` a `null`. **Pegar la salida en rojo en el informe.**
- [x] 5.4 RED — test de `DELETE /api/journal/{date}`: borra y responde `204`; lo inexistente responde `404`.

## 6. GREEN — endpoints

- [x] 6.1 GREEN — `src/handlers/journal.rs` y `src/routes/journal.rs` (`GET /api/journal`, `GET`/`PUT`/`DELETE /api/journal/{date}` con sesión por middleware y resolución del perfil con `ProfilesRepo::get_or_create`); fusionar las rutas en `app_with_state` (`src/lib.rs`) y declarar el módulo en `src/handlers/mod.rs` y `src/routes/mod.rs`. Verificar 5.1–5.4 en verde.
- [x] 6.2 `cargo test` en verde y `cargo clippy -- -D warnings` sin warnings.

## 7. RED — interfaz (Bitácora)

- [x] 7.1 RED — test: el tipo `JournalEntry`/`Mood` y los métodos `listJournalEntries`/`getJournalEntry`/`upsertJournalEntry`/`deleteJournalEntry` del cliente apuntan a las rutas correctas. **Pegar la salida en rojo en el informe.**
- [x] 7.2 RED — test de la vista: lista de más reciente a más antigua mostrando fecha, ánimo, relato, etiquetas, logros y dificultades. **Pegar la salida en rojo en el informe.**
- [x] 7.3 RED — test: editar (ánimo, relato y etiquetas) llama a la API y refresca; borrar pide confirmación antes de llamar a la API.
- [x] 7.4 RED — tests de estados: carga, vacío y error.
- [x] 7.5 RED — test de integración en `AppLayout`: la entrada de Bitácora del header abre la vista, y esta se carga de forma diferida.

## 8. GREEN — interfaz (Bitácora)

- [x] 8.1 GREEN — `frontend/src/types/index.ts`: tipos `JournalEntry` y `Mood`. Verificar 7.1 en verde.
- [x] 8.2 GREEN — `frontend/src/api/client.ts`: los cuatro métodos.
- [x] 8.3 GREEN — la vista de Bitácora (componente nuevo) con listado, edición, borrado con confirmación y estados de carga/vacío/error. Verificar 7.2–7.4 en verde.
- [x] 8.4 GREEN — `frontend/src/components/AppLayout.tsx`: entrada de Bitácora en el header + vista bajo `React.lazy`/`Suspense`. Verificar 7.5 en verde.
- [x] 8.5 `npx vitest run` en verde y `npx tsc --noEmit` limpio.

## 9. Plantillas y ajustes (si hicieran falta)

- [x] 9.1 Comprobar si hace falta tocar alguna plantilla o ajuste (p. ej. una migración de siembra de prompts): si NO hace falta —lo esperado, es `journal-worker`—, dejar constancia en el informe de que se descartó y por qué. **No aplica**: no hay ninguna plantilla ni ajuste que tocar en este change; eso corresponde al siguiente, `journal-worker`.
- [x] 9.2 Si hiciera falta algún ajuste, implementarlo con su test y actualizar el delta de la capability correspondiente; si no, marcar esta tarea como no aplicable. **No aplica**: sin plantillas ni ajustes que tocar en este change.

## 10. Verificación y cierre

- [x] 10.1 `cargo test` (suite completa) en verde y `cargo clippy -- -D warnings` / `cargo fmt --check` limpios. Pegar la salida.
- [x] 10.2 `npx vitest run` completo en verde, `npx tsc --noEmit` limpio y `npm run lint` sin warnings nuevos. Pegar la salida.
- [x] 10.3 `openspec validate journal-entries --strict` en verde y `openspec list` sin cambios huérfanos. Pegar la salida.
- [x] 10.4 Revisión con `@rust-reviewer` (backend, herramienta y endpoints) y `@react-reviewer` (vista y navegación); aplicar los hallazgos. **Cumplida**: ambos revisores auditaron y sus hallazgos se aplicaron; ninguno bloqueante.
- [x] 10.5 Parar y presentar el resultado para la aprobación del usuario.
- [x] 10.6 `openspec archive journal-entries` (tras la aprobación) y merge del delta en `openspec/specs/`.
