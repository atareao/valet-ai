# Proposal

## Why

Las definiciones de tools que se envían al modelo tienen tres problemas que degradan el tool-calling:

1. **Idioma inconsistente**: `web_search` y `search_places` describen la tool y sus parámetros en inglés, mientras el resto están en español.
2. **Obligatorios irreales**: `search_places` exige `latitude`/`longitude` aunque el usuario solo dé texto ("museos en Madrid"), forzando un `geocode` previo; las tools con `operation` solo declaran `operation` como obligatorio y no documentan qué campos exige cada acción, de modo que el modelo puede invocar `delete_note` sin `id` o `create_note` sin `content`.
3. **Esquemas pobres**: la categoría `todo` de `notes` solapa con la tool `tasks`, y el esquema de `data` de `render_widget` no describe sub-propiedades.

## What Changes

- **Idioma**: traducir al español las descripciones (tool y parámetros) de `web_search` y `search_places`.
- **search_places**: hacer `latitude`/`longitude` realmente opcionales — `required: ["query"]` y `locationBias` solo cuando haya `latitude` + `longitude` (y un `radius` válido). El `execute` deja de exigir coordenadas.
- **Obligatorios por operación**: documentar en la descripción de `operation` de `notes`, `tasks` y `reminders` qué campos exige cada acción.
- **notes**: retirar la categoría `todo` del enum y de la descripción de la tool. La BD mantiene su CHECK por compatibilidad (no se migra el esquema).
- **render_widget**: refinar el esquema de `data` con una unión discriminada por `widget_name` (`oneOf`/`anyOf`), manteniendo `data` **opcional**.
- **Pulido**: reformular descripciones de `geocode`, `reverse_geocode`, `get_current_location`, `get_current_time` y `unified_search`, y añadir en `weather` la guía de usar `geocode` si solo se conoce el nombre de la ciudad.
- **BREAKING (interno)**: cambiar el `required` de `search_places` y retirar `todo` del enum de `notes` altera contratos y tests existentes.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `tools/registry`: requisito transversal de descripciones en español y de documentar los obligatorios por operación; esquema de `data` de `render_widget`.
- `tools/geo`: `search_places` con coordenadas opcionales y `locationBias` condicional.
- `tools/geo-weather`: `search_places` requiere únicamente `query`.
- `tools/notes`: enum de categorías sin `todo` y descripción de operaciones.
- `tools/tasks`: descripción de operaciones.
- `tools/reminders`: descripción de operaciones.

## Impact

- **Backend**: `src/tools/google_places.rs` (schema y `execute`), `web_search.rs`, `notes.rs`, `tasks.rs`, `reminders.rs`, `geo.rs`, `weather.rs`, `current_location.rs`, `current_time.rs`, `unified_search.rs`, `widget.rs`.
- **Tests**: los módulos que comparan `description()` o `required` (`weather.rs`, `google_places.rs`, `notes.rs`, `web_search.rs`, `widget.rs`, entre otros).
- **BD**: sin cambios de esquema; el CHECK de `notes.category` no se toca.
- **Frontend**: sin cambios (no existe UI de notas ni consumo del enum de categorías).
