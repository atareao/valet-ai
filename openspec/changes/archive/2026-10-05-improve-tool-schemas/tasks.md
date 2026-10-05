# Tasks

## 1. RED — tests que describen los nuevos contratos (deben fallar)

- [x] 1.1 `src/tools/google_places.rs`: test de schema (`required == ["query"]`) y test de `execute` con solo `{"query":"museos en Madrid"}` que espera éxito sin `locationBias`; verificar que fallan.
- [x] 1.2 `src/tools/google_places.rs`: test de `locationBias.circle` con `latitude`+`longitude`+`radius` y test de ausencia de sesgo sin coordenadas; verificar que fallan.
- [x] 1.3 `src/tools/notes.rs`: test de que el enum de `category` no contiene `todo` y de que la descripción de `operation` menciona `content`/`id`; verificar que fallan.
- [x] 1.4 `src/tools/tasks.rs` y `src/tools/reminders.rs`: tests de la descripción de `operation` con los obligatorios de cada acción; verificar que fallan.
- [x] 1.5 `src/tools/web_search.rs` y `src/tools/google_places.rs`: tests de que `description()` y las descripciones de parámetros están en español; verificar que fallan.
- [x] 1.6 `src/tools/widget.rs`: test de que `data` no está en `required` y de que su esquema usa `oneOf`/`anyOf`; verificar que falla.
- [x] 1.7 `src/tools/weather.rs`, `geo.rs`, `current_location.rs`, `current_time.rs` y `unified_search.rs`: tests de las descripciones reformuladas; verificar que fallan.

## 2. GREEN — implementación mínima

- [x] 2.1 `google_places.rs`: `required: ["query"]`; leer `latitude`/`longitude` como `Option<f64>` y construir `locationBias` solo con lat+lon y `radius` válido; hacer pasar 1.1 y 1.2.
- [x] 2.2 Traducir al español la tool y los parámetros de `web_search` y `search_places`; hacer pasar 1.5.
- [x] 2.3 `notes.rs`: quitar `todo` del enum y de la descripción; documentar los obligatorios en `operation`; hacer pasar 1.3.
- [x] 2.4 `tasks.rs` y `reminders.rs`: documentar los obligatorios en `operation`; hacer pasar 1.4.
- [x] 2.5 `widget.rs`: añadir `oneOf`/`anyOf` discriminado por `widget_name` al esquema de `data`, manteniendo `data` opcional; hacer pasar 1.6.
- [x] 2.6 Reformular descripciones de `weather` (con guía a `geocode`), `geocode`, `reverse_geocode`, `get_current_location`, `get_current_time` y `unified_search`; hacer pasar 1.7.
- [x] 2.7 Actualizar los tests existentes que comparan `description()` o `required` y dejar la suite en verde.

## 3. REFACTOR y verificación

- [x] 3.1 `cargo fmt` y `cargo clippy -- -D warnings` sin warnings.
- [x] 3.2 `cargo test` 100% verde.
- [x] 3.3 `just check-all` verde (Rust + frontend).
- [x] 3.4 `openspec validate improve-tool-schemas` sin errores.
- [x] 3.5 Revisión del código con `rust-reviewer` e incorporar hallazgos críticos/altos.
- [x] 3.6 Archivar el change y, en la spec fusionada, corregir el `## Purpose` de `tools/notes` para que no mencione `todo`.
