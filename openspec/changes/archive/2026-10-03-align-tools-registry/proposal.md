# Change: Alinear el registry de tools con `enabled` y con la tabla `tools`

## Why

El sistema de herramientas tiene **tres capas que no cuadran**: el registry ofrece 10 tools, la tabla
`tools` guarda 7 filas con nombres que no existen en el registry (`geo`, `knowledge`), y dos tools
implementadas y testeadas (`notes`, `unified_search`) **nunca se registran**. Además, el toggle
`enabled` de la UI escribe en la BD pero el orquestador envía **todas** las definiciones al LLM, así
que activar o desactivar una herramienta no tiene ningún efecto. Se necesita una única fuente de
verdad.

## What Changes

- **Cablear las fantasma.** Se registran `notes` (fichero `knowledge.rs`) y `unified_search`, que ya
  tienen implementación y tests pero no llegaban al LLM.
- **Nombre alineado.** Se renombra `src/tools/knowledge.rs` → `src/tools/notes.rs`: el fichero solo
  contiene `NotesTool` y «knowledge» era un resto del nombre antiguo.
- **Ranking de `unified_search` arreglado.** La tool descarta la puntuación `rank` al serializar, así
  que el `sort` posterior compara siempre 0.0 y el orden final es el de inserción de las tablas. Se
  incluye `rank` en cada resultado y se ordena por relevancia de verdad.
- **`enabled` real.** `ToolRegistry::definitions()` SHALL omitir las tools deshabilitadas y
  `ToolRegistry::execute()` SHALL rechazar su ejecución, no solo ocultarlas.
- **Tabla `tools` derivada del registry.** Al arrancar, la tabla se reconcilia con las tools
  registradas: se insertan las ausentes, se borran las obsoletas (`geo`, `knowledge`) y se preserva
  el `enabled` de las existentes. Corrige de paso los nombres desalineados.
- **Cambio en caliente.** El toggle de la API de administración actualiza el estado en memoria, de
  forma que las definiciones siguientes ya reflejan el cambio.
- **Tests y documentación** alineados con el comportamiento nuevo.

## Capabilities

### New Capabilities
- `tools/registry`: catálogo de herramientas — qué tools existen, cuáles se exponen al LLM y cuándo
  se permite su ejecución.
- `tools/unified-search`: búsqueda léxica transversal — sobre qué dimensiones busca y cómo ordena y
  presenta los resultados.

### Modified Capabilities
- `db/repos`: `ToolsRepo` pasa a reconciliar la tabla `tools` con el registry (hoy solo siembra si
  está vacía).
- `orchestrator/agent`: la petición al LLM ofrece únicamente las herramientas habilitadas.

## Impact

- Código: `src/tools/registry.rs`, `src/lib.rs`, `src/handlers/tools.rs`, `src/db/repos/tools.rs`,
  `src/db/mod.rs`, `src/tools/mod.rs` (registro de `notes`/`unified_search`), `src/orchestrator/agent.rs`,
  `src/tools/knowledge.rs` → `src/tools/notes.rs` (renombrado) y `src/tools/unified_search.rs`
  (ranking).
- Datos: la tabla `tools` cambia de contenido en el primer arranque (borra `geo`/`knowledge`, añade
  las restantes). No requiere migración destructiva; la reconciliación es idempotente.
- Sin cambios de frontend: la UI ya lee `GET /api/tools` y togglea por `id`.
- Sin tocar `docker-compose.prod.yml` ni el despliegue.
- Tests afectados: `src/db/repos/tools.rs`, `tests/api/tools.rs` y los que fijan el número de tools
  registradas.

### Fuera de alcance

- Permisos por operación (H4), `search_places` conforme a spec (H5), timeout de `weather` (H6): en
  changes aparte.
