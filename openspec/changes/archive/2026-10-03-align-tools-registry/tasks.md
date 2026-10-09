# Tasks: align-tools-registry

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 0.2 Inventario: `grep -rn -iE "'geo'|'knowledge'|notes|unified_search" src/ | grep -v test` para localizar los puntos de cableado.

## Bloque 1 — RED (comportamiento observable con la API actual)
- [x] 1.1 `tests/api/tools.rs`: la lista de `GET /api/tools` contiene las 12 herramientas reales
  (`calendar`, `tasks`, `weather`, `geocode`, `reverse_geocode`, `search_places`, `web_search`,
  `reminders`, `get_current_time`, `get_current_location`, `notes`, `unified_search`) y NO contiene `geo` ni
  `knowledge`. Hoy devuelve 7 filas con nombres viejos → rojo.
- [x] 1.2 Añadir a `src/tools/unified_search.rs` un test que verifique que cada resultado incluye un
  `rank` numérico y que la secuencia de `rank` es ascendente (hoy el campo no existe → rojo).
- [x] 1.3 Los tests unitarios de `definitions()`/`execute()` filtrados y de `sync_from_registry` se
  escriben en el bloque 2 junto con la API nueva: no son observables con el código actual y un fallo
  de compilación bloquearía el resto de la suite. Se documenta en el informe de RED.
- [x] 1.4 Verificar por CLI: 1.1 y 1.2 en rojo y el resto en verde.

## Bloque 2 — GREEN (implementar)
- [x] 2.1 Renombrar `src/tools/knowledge.rs` → `src/tools/notes.rs` (`git mv`) y actualizar
  `src/tools/mod.rs` (`pub mod notes;`).
- [x] 2.2 `src/lib.rs`: registrar `NotesTool` (módulo `notes`) y `UnifiedSearchTool`.
- [x] 2.3 `ToolRegistry`: campo `disabled: Arc<RwLock<HashSet<String>>>`, `set_disabled`, `is_enabled`;
  `definitions()` omite deshabilitadas; `execute()` devuelve `Err(ToolError::PermissionDenied)` para
  deshabilitadas.
- [x] 2.4 `ToolsRepo`: `sync_from_registry(pool, &[ToolDef])` (upsert por `name`, preserva `enabled`,
  borra obsoletas, idempotente) y `disabled_names(pool)`.
- [x] 2.5 Arranque: quitar el sembrado de `init_db`; en `new_with_orchestrator`, tras construir el
  registry, reconciliar y cargar deshabilitados. Ajustar `AppState::new_in_memory[_empty]` y
  `src/bin/seed.rs` al mismo orden.
- [x] 2.6 `handlers::tools::toggle_tool`: tras el toggle, actualizar `registry.set_disabled` vía
  `state.tool_registry`.
- [x] 2.7 Tests: unitarios del registry (filtrado en `definitions()`, rechazo en `execute()`,
  reexpone al rehabilitar) y de `ToolsRepo::sync_from_registry` (alta, baja, preserva `enabled`,
  idempotente). Actualizar los tests existentes que fijan 7 filas/nombres viejos.
- [x] 2.8 `src/tools/unified_search.rs`: incluir `rank` (columna 1, `f64`) en el JSON de cada
  resultado para que el `sort` ascendente por relevancia sea efectivo.
- [x] 2.9 Verificar por CLI: `cargo test` (0 failed) y `cargo check`.

## Bloque 3 — Documentación
- [x] 3.1 `README.md` y `README.es.md`: la lista de herramientas pasa a 12 incluyendo `notes` y
  `unified_search`; corregir los nombres si procede.
- [x] 3.2 Spec deltas en `specs/tools/registry/`, `specs/tools/unified-search/`, `specs/db/repos/` y
  `specs/orchestrator/agent/`.

## Bloque 4 — REFACTOR y cierre
- [x] 4.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 4.2 `grep` sin residuos de `geo`/`knowledge` como nombres de tool.
- [x] 4.3 Revisión con `rust-reviewer`.
- [x] 4.4 PR a `development`; tras el merge, PR de archivado (`openspec archive align-tools-registry`).
