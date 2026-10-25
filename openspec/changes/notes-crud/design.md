# Design — Gestionar las notas desde la interfaz y poder actualizarlas

## Contexto (estado actual)

- **Tabla** `notes` (`migrations/20260925000001_initial.sql`): `id`, `profile_id`, `content`,
  `category` (`CHECK` admite `idea|journal|fact|todo`), `tags` (`TEXT`, NULL-able), `created_at`,
  `updated_at`.
- **`NotesRepo`**: `create`, `find_by_id`, `list(profile_id, category)`, `update(id, content,
  category, tags)` (COALESCE parcial) y `delete(id)`. **`update` y `delete` operan por `id` sin
  filtrar por perfil**, y no informan de filas afectadas.
- **Tool** `notes` (`src/tools/notes.rs`): enum `["create_note", "list_notes", "delete_note"]`,
  `Permission::NoConfirm` para todo, `profile_id` por defecto `default`. `delete_note` borra por
  `id` sin comprobar el perfil.
- **Router/UI**: cada dominio tiene `src/routes/<d>.rs` con `pub fn routes()` fusionado en
  `src/lib.rs` y handlers en `src/handlers/<d>.rs` que resuelven el perfil con
  `ProfilesRepo::get_or_create(&state.db)`. En el frontend, `AppLayout.tsx` abre vistas-modales
  diferidas (p. ej. `JournalView` + `useJournal` + `api.client.ts` + `types`).

## Contrato de la API

| Método y ruta | Éxito | Errores |
|---|---|---|
| `GET /api/notes?category=&limit=` | `200` array (más reciente primero) | `400` categoría/limit inválidos |
| `POST /api/notes` | `201` con la nota creada | `400` content vacío / categoría inválida |
| `GET /api/notes/{id}` | `200` con la nota | `404` inexistente o de otro perfil |
| `PUT /api/notes/{id}` | `200` con la nota actualizada | `400` sin campos / categoría inválida / content vacío · `404` |
| `DELETE /api/notes/{id}` | `204` | `404` inexistente o de otro perfil |

- Cuerpo de cada nota: `id`, `profile_id`, `content`, `category`, `tags` (`string` o `null`),
  `created_at`, `updated_at`.
- `GET /api/notes`: `category` opcional ∈ {`idea`,`journal`,`fact`}; `limit` por defecto **100** y
  máximo **500**; un `limit` no entero/positivo o por encima del techo → `400`. Orden
  `created_at DESC` (el que ya usa `NotesRepo::list`); el `limit` se aplica **recortando** el vector
  que devuelve el repo, sin tocarlo.
- `POST`/`PUT`: `content` obligatorio no vacío (recortado) en el alta y, si viene, en la
  actualización; `category` por defecto `idea` en el alta y validada ∈ {`idea`,`journal`,`fact`};
  `tags` opcional.
- **Actualización parcial:** en `PUT`, un campo **ausente** deja el valor anterior; `content` y
  `category` no admiten vacío; `tags: ""` **vacía** las etiquetas (la cadena vacía es un valor, no
  una ausencia) y `tags: null`/ausente las deja como estaban. Es el comportamiento que ya tiene
  `NotesRepo::update` con `COALESCE`.
- Se reutiliza `AppError` (`NotFound`/`BadRequest`) y los códigos del resto de la API (`201` en el
  alta, `204` en el borrado), como `handlers/tasks.rs`. En el `PUT` se sigue el patrón
  `find_by_id` → verificar perfil → `update` → `find_by_id` para devolver la nota actualizada.

## Pertenencia al perfil (seguridad)

El perfil se resuelve de la sesión (`ProfilesRepo::get_or_create`), nunca del cuerpo. Como
`NotesRepo::find_by_id`/`update`/`delete` **no** filtran por perfil, el handler (y la tool) SHALL
usar `find_by_id` y **comprobar que `note.profile_id` coincide** con el de la sesión antes de tocar
nada. Una nota inexistente **o de otro perfil** responde `404` (nunca `403`): no se filtra la
existencia de la nota de otro perfil. La tool, por su parte, responde con un error de «nota no
encontrada».

## Tool `notes`: `update_note`

- Args: `id` (obligatorio, no vacío) y, opcionalmente, `content`, `category`, `tags`; debe venir
  **al menos uno** de esos tres.
- `category` se valida contra {`idea`,`journal`,`fact`}. El enum `operation` pasa a
  `["create_note","list_notes","update_note","delete_note"]` y su descripción documenta los
  obligatorios.
- `content: ""` (vacío) se rechaza: una nota sin texto no tiene sentido.
- `update_note` y `delete_note` comprueban la pertenencia al perfil (`profile_id` de los args, que
  inyecta el orquestador).

## Interfaz

- **Punto de entrada:** un `Button` más en la cabecera de `AppLayout.tsx`, junto al de Bitácora
  (icono + `aria-label`/`title` «Notas»), que abre la vista como modal, con `lazy()`.
- **`NotesView.tsx`** (contenido del modal) sobre **`useNotes.ts`** (listado + `create`/`update`/
  `remove`, recargando tras cada mutación, como `useJournal`):
  - Lista de notas con su categoría como `Tag`, los `tags` y las fechas (partiendo la cadena ISO,
    nunca `new Date()`, como `JournalView`).
  - Alta y edición con un formulario en línea (contenido, categoría, etiquetas), y borrado con
    `Popconfirm`.
  - La categoría se elige de un desplegable con las tres permitidas y etiquetas legibles
    (Idea / Diario / Dato). Una nota **legada** con categoría `todo` se muestra igual, pero guardar
    obliga a elegir una de las tres.

## Alternativas descartadas

- **Tags como array en la API:** exigiría normalizar en la frontera y deshacerlo al guardar; la
  columna es una cadena y la tool ya usa el mismo formato.
- **Migrar el `CHECK` para quitar `todo`:** SQLite obliga a reconstruir la tabla (crear, copiar,
  borrar, renombrar) para cambiar un `CHECK`; no compensa. La validación en la API cubre el objetivo.
- **Pestaña superior en lugar de modal:** las vistas del proyecto son modales desde la cabecera;
  una pestaña rompería además el guardián de pestañas de `SettingsDialog`.
- **Cambiar el permiso de la tool a `Confirm`:** fuera de alcance; la asimetría con
  `tasks.delete_task` ya está anotada en su propia spec.
