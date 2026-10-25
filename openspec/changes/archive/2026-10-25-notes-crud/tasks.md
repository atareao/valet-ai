# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **OK explícito** del usuario antes de escribir código o tests.

## 1. RED — tool `notes`

- [x] 1.1 Test que falla: el enum de `operation` ofrece `update_note` además de las tres actuales.
- [x] 1.2 Test que falla: `update_note` cambia el `content` de una nota y lo persiste.
- [x] 1.3 Test que falla: `update_note` exige `id` no vacío y **al menos uno** de `content`/`category`/`tags` (si no, `InvalidArguments`).
- [x] 1.4 Test que falla: `update_note` rechaza una `category` fuera de {`idea`,`journal`,`fact`} y un `content` vacío.
- [x] 1.5 Test que falla: `tags: ""` vacía las etiquetas de la nota.
- [x] 1.6 Test que falla: `update_note` y `delete_note` **no actúan** sobre una nota de otro perfil (error «nota no encontrada», sin escribir).
- [x] 1.7 Test que falla: `permission()` sigue devolviendo `NoConfirm` para `update_note`.

## 2. RED — API `/api/notes`

- [x] 2.1 Test que falla: `GET /api/notes` devuelve las notas del perfil ordenadas de más reciente a más antigua.
- [x] 2.2 Test que falla: `GET /api/notes?category=journal` filtra por categoría; una categoría inválida responde `400`.
- [x] 2.3 Test que falla: `GET /api/notes?limit=1` devuelve una sola nota y un `limit` fuera de rango (`0`, `-1`, `abc`, `501`) responde `400`.
- [x] 2.4 Test que falla: `POST /api/notes` crea y responde `201` con la nota; `content` vacío o categoría inválida responden `400`.
- [x] 2.5 Test que falla: `GET /api/notes/{id}` devuelve la nota o `404`.
- [x] 2.6 Test que falla: `PUT /api/notes/{id}` actualiza de forma **parcial** (un campo ausente no se pierde) y devuelve la nota; sin campos o con categoría inválida responde `400`.
- [x] 2.7 Test que falla: `DELETE /api/notes/{id}` responde `204` y la nota desaparece.
- [x] 2.8 Test que falla: `GET`/`PUT`/`DELETE` sobre una nota **de otro perfil** responden `404`.
- [x] 2.9 Test que falla: sin sesión, `/api/notes` responde `401`.

## 3. RED — interfaz

- [x] 3.1 Test que falla: `useNotes` carga el listado y expone `create`/`update`/`remove`, recargando tras cada mutación.
- [x] 3.2 Test que falla: `NotesView` lista las notas, y el formulario crea y edita; el borrado pide confirmación.

## 4. GREEN — backend

- [x] 4.1 `src/tools/notes.rs`: `update_note` (validaciones del diseño) y comprobación de pertenencia al perfil en `update_note`/`delete_note`; enum y descripción de `parameters()` actualizados.
- [x] 4.2 `src/handlers/notes.rs` (nuevo): los cinco handlers con `ProfilesRepo::get_or_create`, validación de entrada y `AppError` (`NotFound`/`BadRequest`), según el contrato del `design.md`.
- [x] 4.3 `src/routes/notes.rs` (nuevo) + merge en `src/lib.rs`: `/api/notes` y `/api/notes/{id}`.

## 5. GREEN — frontend

- [x] 5.1 `src/types/index.ts`: `Note`, `NoteInput`, `NoteCategory`.
- [x] 5.2 `src/api/client.ts`: `listNotes`, `createNote`, `updateNote`, `deleteNote`.
- [x] 5.3 `src/hooks/useNotes.ts`: listado + mutaciones con recarga y rechazo de errores de mutación.
- [x] 5.4 `src/components/NotesView.tsx`: lista, alta, edición en línea y borrado con confirmación.
- [x] 5.5 `src/components/AppLayout.tsx`: botón «Notas» en la cabecera que abre la vista (modal, `lazy`).

## 6. Verificación y cierre

- [x] 6.1 `cargo test` en verde.
- [x] 6.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 6.3 `cargo fmt --check` en verde.
- [x] 6.4 `npx tsc --noEmit` y `npx vitest run` en verde.
- [x] 6.5 `openspec validate notes-crud --strict` en verde.
- [x] 6.6 Revisión con `@rust-reviewer` (backend) y `@react-reviewer` (UI); hallazgos aplicados.
- [x] 6.7 `openspec archive notes-crud` y registro del cierre en `AGENTS.md` §V si procede.
