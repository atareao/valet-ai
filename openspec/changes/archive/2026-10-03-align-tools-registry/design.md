# Design

## Context

Ver `proposal.md` — Why. El estado actual relevante:

- `ToolRegistry` (`src/tools/registry.rs`) envuelve un `HashMap<String, Arc<dyn Tool>>` inmutable
  tras la construcción. `definitions()`, `execute()` y `permission()` no consultan la BD.
- `src/orchestrator/agent.rs:723` construye la petición con `self.registry.definitions()` en cada
  iteración del bucle ReAct.
- La tabla `tools` (`name TEXT NOT NULL UNIQUE`) la gestiona `ToolsRepo`, usada solo por
  `src/handlers/tools.rs` (listar/toggle) y por `seed_defaults`.
- El orden de arranque en `Valet::new_with_orchestrator` es: `db::init_db` (migra + siembra) →
  construir registry. Es decir, **la semilla corre antes de que exista el registry**.
- `AppState` ya expone `tool_registry: Option<Arc<ToolRegistry>>`, así que el handler del toggle
  puede alcanzarlo.

## Goals / Non-Goals

**Goals**

- Una única fuente de verdad: el registry. La tabla `tools` es su reflejo persistido (descripción +
  enabled).
- `enabled=false` con efecto real en los dos extremos: no se ofrece al LLM y no se ejecuta.

**Non-Goals**

- No se introduce una capa nueva de configuración ni se toca el frontend.
- No se cambian los permisos por operación (H4) ni las tools de geo/tiempo (H5/H6).

## Decisions

### 1. El estado `enabled` vive en el registry con interior mutability

`ToolRegistry` gana un campo `disabled: Arc<RwLock<HashSet<String>>>` con `set_disabled(names)` e
`is_enabled(name)`. `definitions()` omite los nombres deshabilitados; `execute()` devuelve
`Err(ToolError::PermissionDenied(...))` antes de invocar la tool.

- **Por qué:** `definitions()` es sincrónico y se llama en cada iteración; hacerlo async y con pool
  acoplaría el registry a la BD y añadiría una consulta por turno. El conjunto en memoria mantiene el
  registry puro y barato.
- **Alternativas descartadas:** (a) leer `tools.enabled` en cada `definitions()` — rompe la firma
  síncrona y mete E/S en el camino caliente; (b) filtrar en `agent.rs` tras recibir las definiciones —
  deja `execute()` sin protección (defensa en profundidad fallida).
- **Error elegido:** se reutiliza `ToolError::PermissionDenied` con mensaje «tool is disabled» para no
  ampliar el enum ni sus `match`. El orquestador ya convierte cualquier `Err(ToolError)` en
  `ToolResult { success: false }`, así que una tool deshabilitada invocada por el modelo se degrada
  igual que cualquier otro error.

### 2. La tabla `tools` se reconcilia desde el registry

Nuevo `ToolsRepo::sync_from_registry(pool, defs: &[ToolDef])`, apoyado en `name UNIQUE`:

1. `DELETE FROM tools WHERE name NOT IN (...)` — baja de obsoletas (`geo`, `knowledge`).
2. `INSERT INTO tools (id, name, description, enabled) VALUES (...) ON CONFLICT(name) DO UPDATE SET
   description = excluded.description` — alta de ausentes con `enabled=1`, y actualización de
   descripción **sin pisar** `enabled`.

`ToolsRepo::disabled_names(pool)` devuelve los nombres con `enabled = 0` para cargar el registry.

- **Por qué:** el registry ya conoce nombre y descripción reales de cada tool; duplicarlos en una
  semilla manual fue justo lo que produjo `geo`/`knowledge`. Derivarlos elimina la clase de error.
- **Alternativas descartadas:** una migración con la lista correcta — vuelve a cablear la lista y se
  desincroniza al primer alta; mantener `seed_defaults` y parchear nombres — sigue habiendo dos
  fuentes.

### 3. Reordenar el arranque, sin doble fuente

`init_db` deja de sembrar `tools`. La reconciliación se hace **después** de construir el registry, en
`new_with_orchestrator`:

```text
registry = construir todas las tools (incl. notes, unified_search)
ToolsRepo::sync_from_registry(&pool, &registry.definitions())
let disabled = ToolsRepo::disabled_names(&pool)
registry.set_disabled(disabled)
```

`AppState::new_in_memory[_empty]` y `src/bin/seed.rs` se ajustan al mismo orden (construir registry →
reconciliar), o prescinden de la tabla si no la necesitan.

### 4. El toggle actualiza el registry en caliente

`handlers::tools::toggle_tool`, tras `ToolsRepo::toggle_enabled`, recalcula el conjunto de
deshabilitados y llama a `registry.set_disabled(...)` vía `state.tool_registry`. El cambio de
`enabled` en la BD y el estado en memoria quedan consistentes en el mismo request.

### 5. Renombrar `knowledge.rs` → `notes.rs`

`git mv src/tools/knowledge.rs src/tools/notes.rs` y actualizar `src/tools/mod.rs`
(`pub mod notes;`). El registro pasa a `crate::tools::notes::NotesTool`. No hay más usos del módulo
`knowledge` en producción; la semilla `("knowledge", ...)` desaparece con la reconciliación y los
tests de `tests/api/tools.rs` ya se reescriben.

- **Por qué:** el fichero solo define `NotesTool`; mantener el nombre viejo obliga a recordar una
  equivalencia inexistente. No colisiona con `src/db/repos/notes.rs` (directorios distintos).

### 6. Arreglar el ranking de `unified_search`

`search_table` selecciona `source, rank, snippet` pero hoy serializa solo `source` y `snippet`,
descartando `rank`; el `sort` posterior (`rank` ausente → `unwrap_or(0.0)`) no ordena nada. Se incluye
`"rank"` en el JSON de cada resultado (columna 1, `f64`) y se conserva el `sort` ascendente existente
(rank menor = más relevante), que pasa a ser efectivo.

- **Por qué:** al cablear la tool el comportamiento se vuelve visible al LLM; entregar resultados en
  orden de inserción es un defecto funcional, no cosmético.
- **Alternativas descartadas:** dejar el `sort` y ordenar solo dentro de cada tabla por SQL — la fusión
  de cuatro consultas seguiría sin orden global.

## Risks / Trade-offs

- **[La reconciliación borra filas con `enabled` personalizado de tools obsoletas]** → son tools que
  ya no se ofrecen; se asume la pérdida de ese estado, igual que en la poda anterior.
- **[Ventana de inconsistencia BD↔memoria si hay escrituras externas a la API]** → único escritor es
  la API de administración, que actualiza ambos; el arranque recarga desde BD.
- **[Tests que fijan el recuento de tools (7) y nombres (`geo`/`knowledge`)]** → se actualizan al
  nuevo contrato (12 nombres reales).
- **[`definitions()` cambia de firma observable para los tests del registry]** → se añaden tests
  unitarios de filtrado y rechazo de ejecución.
- **[El renombrado rompe rutas de `include!`/`mod` olvidadas]** → `grep -rn 'tools::knowledge\|mod knowledge'`
  antes y después; solo `mod.rs` lo referencia.
- **[El `rank` de FTS5 puede ser negativo según la configuración de bm25]** → el orden es
  numérico ascendente, independiente del signo; los tests comprueban monotonía, no valores fijos.

## Migration Plan

Sin migración de esquema: `tools.name` ya es `UNIQUE`. En el primer arranque la reconciliación
corrige la tabla. Rollback: revertir el código; el arranque vuelve a reconciliar (el registry sin
`notes`/`unified_search` las borraría de la tabla, comportamiento aceptado).

## Open Questions

Ninguna: las decisiones que afectaban a las specs (cablear ambas, derivar del registry, semántica de
`enabled`) se cerraron con el usuario antes de redactar este change.
