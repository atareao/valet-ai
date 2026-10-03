# Tasks

## 1. Baseline

- [x] 1.1 Registrar baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 1.2 Reproducir el fallo con wiremock: `{"query":"Querida Jacinta restaurante","latitude":39.4676,"longitude":-0.3771,"radius":5000}` debe disparar hoy `places:searchNearby` con `includedTypes` (evidencia de la causa raíz).

## 2. RED

- [x] 2.1 Test: `execute({"query":"Querida Jacinta restaurante","latitude":39.4676,"longitude":-0.3771,"radius":5000})` DEBE disparar `places:searchText`, incluir `locationBias.circle` (center + radius) y NO incluir `includedTypes`. Hoy dispara `searchNearby` → rojo.
- [x] 2.2 Test: `execute({"query":"restaurantes en Madrid","latitude":40.4168,"longitude":-3.7038})` (sin `radius`) DEBE disparar `places:searchText` sin `locationBias` ni `includedTypes`.
- [x] 2.3 Test: `execute({"query":"cafe","latitude":40.4168,"longitude":-3.7038,"radius":500})` DEBE disparar `places:searchText` con `locationBias.circle` y sin `includedTypes`.
- [x] 2.4 Verificar por CLI: 2.1 y 2.3 en rojo (ambas con radius hoy van a searchNearby); 2.2 y el resto en verde.

## 3. GREEN

- [x] 3.1 `SearchPlacesTool::execute`: único camino `search_text`; cuando `radius` esté presente, pasar `locationBias.circle`.
- [x] 3.2 `search_text`: aceptar un `location_bias` opcional (center + radius) y añadirlo al body solo si existe.
- [x] 3.3 Retirar `search_nearby` y actualizar/eliminar sus tests obsoletos.
- [x] 3.4 Tests GREEN: 2.1–2.3 pasan. Verificar por CLI: `cargo test --no-fail-fast` (0 failed) y `cargo check`.

## 4. REFACTOR y cierre

- [x] 4.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 4.2 Revisión con `rust-reviewer`.
- [ ] 4.3 PR a `development`; tras el merge, PR de archivado (`openspec archive fix-search-places-radius-bias`).

## 5. Verificación de integración

- [ ] 5.1 Tras desplegar, repetir en producción una consulta de nombre con `radius` y confirmar que ya no devuelve 400 y que `search_places` responde con lugares.
