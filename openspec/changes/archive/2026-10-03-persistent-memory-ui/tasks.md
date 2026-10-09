# Tasks: Interfaz de la memoria persistente (Capa C)

## Bloque 1 — Lógica compartida y errores
- [x] 1.1 RED/GREEN: extraer la lectura del presupuesto a `persistent_memory::read_budget(pool)`
  con la misma semántica (falta o no parseable ⇒ default 500, con warning) y hacer que el worker
  la use, sin cambiar su comportamiento.
- [x] 1.2 RED/GREEN: `AppError::Conflict(String)` → `409 Conflict`, con su test de respuesta.

## Bloque 2 — API HTTP del estado
- [x] 2.1 RED/GREEN: `GET /api/persistent-memory` (payload parseado, marca, tokens, presupuesto,
  techo, `is_empty`; ausencia ⇒ vacío sin crear fila).
- [x] 2.2 RED/GREEN: `PUT /api/persistent-memory` (validación de esquema + control de presupuesto
  y techo + `warning` + `updated_at` por hash de contenido, sin LLM).
- [x] 2.3 RED/GREEN: concurrencia optimista con `expected_updated_at` ⇒ `409`.
- [x] 2.4 RED/GREEN: `DELETE /api/persistent-memory` idempotente.
- [x] 2.5 Montar la ruta en `app_with_state` y exportar el handler en `routes`.

## Bloque 3 — Frontend: acceso a datos
- [x] 3.1 RED/GREEN: tipo `PersistentMemoryState` y métodos del cliente
  (`getPersistentMemory`, `updatePersistentMemory`, `clearPersistentMemory`).
- [x] 3.2 RED/GREEN: hook `usePersistentMemory` (carga, guardado, conflicto 409, aviso de tamaño,
  vaciado).

## Bloque 4 — Frontend: pantalla
- [x] 4.1 RED/GREEN: `PersistentMemoryPanel` (marca temporal, tokens/presupuesto, textarea con
  validación en cliente, guardar, vaciar con confirmación, avisos de conflicto y tamaño).
- [x] 4.2 RED/GREEN: campo del presupuesto integrado con `updateSettings`, sin tocar el estado.
- [x] 4.3 Montar la pestaña «Memoria persistente» en `SettingsDialog`.

## Bloque 5 — Verificación y limpieza
- [x] 5.1 `cargo fmt --check` limpio.
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` limpio.
- [x] 5.3 `cargo test` completo en verde, sin regresiones.
- [x] 5.4 `cd frontend && npx tsc --noEmit && npx vitest run` en verde.
- [x] 5.5 `cd frontend && npm run lint:ci` limpio.
- [x] 5.6 `openspec validate persistent-memory-ui --strict` válido.
