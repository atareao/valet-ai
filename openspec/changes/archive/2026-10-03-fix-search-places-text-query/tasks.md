# Tasks: fix-search-places-text-query

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 0.2 Inventario: `grep -rn "search_nearby\|search_text\|includedTypes\|textQuery" src/tools/google_places.rs` para fijar el estado actual.

## Bloque 1 — RED
- [x] 1.1 Test: `execute({"query":"restaurantes en Madrid","latitude":40.4168,"longitude":-3.7038})` (sin `radius`) DEBE disparar `places:searchText` con `textQuery`. Hoy dispara `searchNearby` → rojo.
- [x] 1.2 Test: `execute({"query":"cafe","latitude":40.4168,"longitude":-3.7038,"radius":500})` DEBE disparar `places:searchNearby` con `locationRestriction.circle`. Hoy también `searchNearby`, pero con `includedTypes` derivados de `query`; verificar el body correcto.
- [x] 1.3 Verificar por CLI: 1.1 en rojo por aserción y el resto en verde.

## Bloque 2 — GREEN
- [x] 2.1 `src/tools/google_places.rs`: en `SearchPlacesTool::execute`, despachar a `search_text(query, ...)` cuando `args` no tenga `radius`, y a `search_nearby(lat, lon, radius, types)` cuando sí lo tenga.
- [x] 2.2 Mantener la lectura de `google_places_api_key` (settings → Config/ENV) y el `maps_link` en ambas rutas.
- [x] 2.3 Tests GREEN: 1.1 y 1.2 pasan.
- [x] 2.4 Verificar por CLI: `cargo test --no-fail-fast` (0 failed), `cargo check`.

## Bloque 3 — REFACTOR y cierre
- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Revisión con `rust-reviewer`.
- [x] 3.3 PR a `development`; tras el merge, PR de archivado (`openspec archive fix-search-places-text-query`).
