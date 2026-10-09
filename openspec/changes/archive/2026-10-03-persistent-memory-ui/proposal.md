# Proposal: Interfaz de la memoria persistente (Capa C)

## Why

La Capa C ya se consolida, se mide y se inyecta, pero es invisible e ineditable: no hay forma de
ver qué sabe Valet del usuario, corregir un hecho, borrar una regla ni vaciar el estado sin tocar
la base a mano. Una memoria que no se puede inspeccionar ni corregir no es confiable; hace falta
una UI de ver/edición, y el presupuesto de tokens —hoy solo editable por configuración— encaja en
el mismo sitio.

## What Changes

- **D1 — Lectura por HTTP.** `GET /api/persistent-memory` devuelve el `payload` ya parseado (o
  `null`), `updated_at`, `token_count`, `budget_tokens`, `ceiling_tokens` e `is_empty`. La
  ausencia de fila se lee como estado vacío y no crea fila.
- **D2 — Escritura validada.** `PUT /api/persistent-memory` valida el esquema versionado de la
  Capa C, mide el estado y aplica el mismo presupuesto y techo absoluto que la consolidación: por
  encima del techo se rechaza conservando lo anterior; por encima del presupuesto y bajo el techo
  se guarda con aviso. Sin LLM en el guardado manual. `updated_at` lo resuelve Rust por hash de
  contenido, como en la consolidación.
- **D3 — Concurrencia optimista.** `expected_updated_at` opcional: si no coincide con la marca
  vigente —incluida la diferencia entre «sin fila» y «con fila»—, `409 Conflict` y nada se
  escribe. Evita pisar una consolidación del worker ocurrida entre la carga y el guardado.
- **D4 — Vaciado.** `DELETE /api/persistent-memory` elimina la fila `global_state`; idempotente.
- **D5 — Pantalla.** Nueva pestaña «Memoria persistente» en el diálogo de ajustes: estado, marca
  temporal, tokens frente al presupuesto, editor JSON en crudo con validación en cliente (el
  servidor sigue siendo la autoridad), guardado con manejo de conflicto y aviso de tamaño, y
  acción de vaciado con confirmación.
- **D6 — Presupuesto a la vista.** La misma pestaña muestra y edita
  `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` reutilizando el endpoint de settings; cambiar el
  presupuesto no toca el estado persistente.

### Fuera de alcance

- Editor estructurado (pares clave/valor) y edición por secciones: se eligió JSON en crudo.
- Historización/versionado del estado: solo se ve y edita la foto actual.
- Compresión con LLM en el guardado manual.
- Tocar `docker-compose.prod.yml`.

## Capabilities

### New Capabilities

- `persistent-memory-ui`: la superficie HTTP y de interfaz para ver, editar y vaciar el estado
  persistente, y para ver y editar su presupuesto de tokens.

### Modified Capabilities

- (ninguna)

## Impact

- **Specs**: `persistent-memory-ui` (nueva).
- **Código**: `src/routes/persistent_memory.rs` y su handler; `src/errors.rs` (variante
  `Conflict` → 409); `src/persistent_memory.rs` (helper `read_budget` compartido) y el worker
  `src/workers/episodic_memory.rs`, que pasa a usarlo; `src/lib.rs` y `src/routes/mod.rs` (ruta
  nueva); tests `tests/api/persistent_memory.rs`.
- **Frontend**: `types.ts`, `api/client.ts`, `hooks/usePersistentMemory.ts`,
  `components/PersistentMemoryPanel.tsx` (+ test), `components/SettingsDialog.tsx`.
- **Sin migración** (la tabla y las claves ya existen). **Sin tocar** `docker-compose.prod.yml`.
