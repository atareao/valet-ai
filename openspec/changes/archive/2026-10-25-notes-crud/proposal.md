# Change: Gestionar las notas desde la interfaz y poder actualizarlas

## Why

Las notas existen desde hace tiempo —tabla `notes`, `NotesRepo`, tool `notes` y dimensión propia en
`unified_search`— pero solo se manejan **conversando con Valet**:

1. **No hay interfaz.** No existe ruta REST ni componente en el frontend. No se pueden *ver* ni
   *editar*: ni listarlas en una pantalla, ni corregir una nota.
2. **La tool no sabe actualizar.** Su enum es `["create_note", "list_notes", "delete_note"]`, así
   que corregir una nota obliga a **borrarla y volver a crearla**, aunque `NotesRepo::update` ya
   existe y no lo usa nadie.
3. **El borrado no comprueba el perfil.** `delete_note` borra **por `id` sin mirar el `profile_id`**
   (mientras `list_notes` sí filtra por perfil). Cualquiera que conozca un id puede borrar la nota de
   otro perfil. `update_note` no debe heredar ese agujero.

## What Changes

- La tool `notes` SHALL ganar la operación **`update_note`** (`id` obligatorio y, opcionalmente,
  `content`, `category` y `tags`; al menos uno de los tres), y SHALL **comprobar la pertenencia al
  perfil** al actualizar y al borrar.
- La API SHALL exponer **`/api/notes`** (listado con filtro por categoría y alta) y
  **`/api/notes/{id}`** (lectura, actualización parcial y borrado), con sesión OIDC y validación de
  la categoría contra el conjunto cerrado.
- La interfaz SHALL añadir una **vista de Notas** (ver, crear, editar y borrar) abierta desde la
  barra superior, siguiendo el patrón de la Bitácora (`JournalView` + `useJournal`).

## Impact

- **Backend:** `src/tools/notes.rs`; nuevos `src/routes/notes.rs` y `src/handlers/notes.rs`; merge
  del router en `src/lib.rs`.
- **Frontend:** nuevos `src/components/NotesView.tsx` y `src/hooks/useNotes.ts`; métodos en
  `src/api/client.ts`; tipos en `src/types/index.ts`; botón en `src/components/AppLayout.tsx`.
- **Specs:** `tools/notes` (1 eliminado, 1 añadido, 1 modificado), `notes-api` (nuevo),
  `notes-ui` (nuevo).
- **Sin migración:** la tabla `notes` ya existe y no cambia. **No** se toca
  `docker-compose.prod.yml`.

## Decisiones de alcance

- La vista incluye **crear y borrar**, no solo ver y editar: sin alta desde la interfaz, la pantalla
  solo mostraría lo que ya metiste por chat, y sin borrado un error de tecleo quedaría atrapado.
- **Categorías:** `idea`, `journal`, `fact` (las mismas que ofrece la tool). El `CHECK` de la tabla
  todavía admite `todo` por legado: **no se migra** y la validación vive en la API.
- **`tags`** se expone como **cadena** (separada por comas), igual que en la tool y en la columna,
  en vez de normalizarla a array. Los `tags` pasan a ser editables y vaciables.

## Non-goals

- NO se toca el `CHECK` de la tabla `notes` (evita reconstruir la tabla en SQLite).
- NO se añade paginación por `offset` ni «cargar más»: el listado se acota con un `limit` con techo.
- NO se toca `unified_search` (ya indexa las notas) ni el router de skills (`recuerdos` ya incluye
  la tool `notes`); el cambio es interno a la tool.
- NO se cambia el permiso de la tool: sigue `NoConfirm` en todas las operaciones.
