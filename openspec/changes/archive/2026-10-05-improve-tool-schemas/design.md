# Design

## Context

Los schemas de las tools viven en el propio código Rust: cada implementación de `Tool` devuelve su `description()` y su `parameters()` (JSON Schema escrito con `serde_json::json!`). No existe un fichero estático de definiciones que editar; cualquier cambio se hace en el módulo correspondiente y el `ToolRegistry::definitions()` los expone al modelo.

El registro de producción (`build_tool_registry`, `src/lib.rs`) incluye 13 tools: `weather`, `geocode`, `reverse_geocode`, `search_places`, `web_search`, `calendar`, `tasks`, `reminders`, `get_current_time`, `get_current_location`, `notes`, `unified_search` y `render_widget`.

Estado actual de lo que se va a tocar:
- `web_search` (`src/tools/web_search.rs:71-85`) y `search_places` (`src/tools/google_places.rs:276-302`) tienen texto en inglés.
- `search_places` declara `required: ["query","latitude","longitude"]` y su `execute` (`google_places.rs:315-322`) falla con `InvalidArguments` si faltan las coordenadas; `locationBias` se construye a partir de `(lat, lon, radius)`.
- `notes` ofrece la categoría `todo` (`src/tools/notes.rs:108,123`); la BD la admite en el CHECK (`migrations/20260925000001_initial.sql:126`).
- `render_widget` mantiene `data` opcional y lo normaliza a `{}` (`src/tools/widget.rs:19-24`); su descripción ya enumera las claves por widget (`DATA_SCHEMA_DESCRIPTION`, `widget.rs:12`).

La motivación y el alcance están en `proposal.md`; el comportamiento exigido, en `specs/`. Este documento sólo decide el "cómo".

## Goals / Non-Goals

**Goals:**
- Que el modelo invoque bien las tools: descripciones coherentes, obligatorios explícitos y menos pasos forzados.
- Relajar `search_places` sin romper la búsqueda con sesgo geográfico cuando sí hay coordenadas.
- Mantener la compatibilidad hacia atrás en lo posible (BD intacta, `render_widget.data` opcional).

**Non-Goals:**
- Descomponer las tools CRUD en funciones atómicas (fuera del alcance del function-calling actual).
- Migrar o reconstruir la tabla `notes` para endurecer su CHECK.
- Cambiar permisos, flujo de ejecución o el frontend.
- Introducir `if/then` de JSON Schema para los obligatorios condicionales.

## Decisions

### D1 — Idioma: español en `description()` y en las descripciones de parámetros

Todas las tools integradas exponen descripciones en español. Se traducen `web_search` y `search_places`. Alternativa descartada: unificar en inglés, porque el prompt de sistema, las specs y la mayoría de tools ya están en español; un híbrido perjudica el anclaje del modelo.

### D2 — `search_places` con coordenadas opcionales

`required` pasa a `["query"]`. En `execute`, `latitude`/`longitude` se leen como `Option<f64>`; `locationBias` se construye únicamente cuando hay ambas coordenadas y un `radius` válido (> 0). Sin coordenadas, se llama a `places:searchText` con `textQuery` y sin sesgo. Alternativas descartadas: (a) exigir `geocode` previo, que es justo la fricción que se elimina; (b) mantener las coordenadas obligatorias y solo documentarlas, que no arregla la invocación directa.

### D3 — `notes`: retirada "soft" de la categoría `todo`

Se elimina `todo` del enum y de la descripción de la tool, de modo que el modelo deja de ofrecerla; el solapamiento con `tasks` desaparece a efectos de tool-calling. **No se migra el esquema**: SQLite no permite modificar un `CHECK` con `ALTER TABLE`; habría que reconstruir la tabla `notes` (copiar, borrar, renombrar, recrear triggers FTS), con riesgo sobre datos existentes y beneficio nulo, porque el valor simplemente deja de ofrecerse. El CHECK sigue aceptando `todo` para filas legacy y `list_notes` lo sigue devolviendo si existiera. Alternativa descartada: migración de reconstrucción.

### D4 — `render_widget`: unión discriminada, `data` opcional

El esquema de `data` se refina con `oneOf` (o `anyOf`) con una alternativa por widget. `data` **sigue siendo opcional** y no entra en `required`. Alternativa descartada: hacer `data` obligatorio; rompería `normalize_widget_data` y los tests que cubren la invocación sin `data`, además de ser innecesario (cada widget tiene defaults).

**Riesgo**: el soporte de `oneOf`/`anyOf` varía según el proveedor de LLM. Mitigación: mantener el esquema documental ya existente (`DATA_SCHEMA_DESCRIPTION`) y, si un proveedor rechaza la unión, degradar al esquema plano actual sin cambiar el contrato de ejecución.

### D5 — Obligatorios por operación en la descripción, no en el schema

JSON Schema estándar de function-calling no soporta bien `required` condicional (`if/then` no es portable). Se documentan los obligatorios de cada acción en la descripción de `operation` (práctica que `calendar` ya sigue en `src/tools/calendar.rs:279-282`), y se extiende a `notes`, `tasks` y `reminders`.

## Risks / Trade-offs

- **Cambiar `required`/descripciones rompe tests que comparan strings.** Se actualizan en el mismo change (fase GREEN).
- **`oneOf`/`anyOf` no universal.** Documentado arriba con plan de degradación.
- **La spec `tools/notes` seguirá mencionando `todo` en su `## Purpose`** hasta que se archive; se corrige tras el archive (tarea).

## Migration Plan

1. RED: tests que describen los nuevos contratos y fallan.
2. GREEN: implementación mínima en los módulos de tools.
3. REFACTOR: `cargo fmt`, `cargo clippy -- -D warnings`, suite completa.
4. Verificación: `just check-all` y `openspec validate improve-tool-schemas`.
5. Archive: fusionar deltas en `openspec/specs/` y ajustar el `Purpose` de `tools/notes`.

## Open Questions

- Ninguna: el alcance y las decisiones se han cerrado con el usuario.
