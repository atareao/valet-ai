# Proposal: El panel de estadísticas deja de fundir los modelos en un solo rol

## Why

El panel «📊 Stats» ya existe y muestra una tabla «Procesos de fondo» con **una fila por rol**
(`kind`). El problema es que **no tiene eje de modelo**, y eso esconde justo lo que se quiere
vigilar. La fila que la UI muestra hoy para el consolidador es literalmente
`89 llamadas · 242.522 tokens · 0 errores`, y dentro de esa única fila se funden **tres modelos**:

- `deepseek/deepseek-v4.1-flash`: **54 llamadas, 1 token de salida de media** — la avería del 9 y 10
  de octubre, respuestas vacías registradas como `status = 'success'`;
- `qwen/qwen3-235b-a22b-2507`: 33 llamadas, 654 tokens de salida de media (el parche sano);
- `mistralai/mistral-small-24b-instruct-2501`: 2 llamadas, 356 tokens.

La fusión tiene cuatro consecuencias medidas en producción:

- **Un modelo configurado en producción es invisible.** `SEMANTIC_MODEL=qwen/...` no aparece en
  ninguna parte de la UI: la tabla de roles lo funde con sus hermanos, y el panel «Cost by Model»
  filtra `WHERE kind='chat'`, donde qwen nunca se usa. El modelo que de verdad hace el trabajo de
  fondo no se ve por ningún lado.
- **La columna «Errors» no habría visto la avería.** Cuenta `status != 'success'`, y las 54
  respuestas vacías se registraron como éxito. El fallo que más ha costado es el que la tabla no
  sabe ver: `89 llamadas · 0 errores`.
- **La tabla suma toda la vida.** Un contador acumulado no responde «¿está funcionando ahora?».
- **Un rol con error alto pasa desapercibido.** El rol `timeline` con `mistral` acumula 7 errores en
  20 llamadas (35 %), invisible por la fusión de modelos y por la falta de ventana temporal.

## What Changes

- **MODIFICADO — `StatsRepo::background_summary` SHALL devolver una fila por pareja (origen,
  modelo)**, en lugar de una por origen. `BackgroundStats` SHALL ganar `model: Option<String>`. Los
  orígenes canónicos (`router`, `archivist`, `consolidator`, `collapse`, `timeline`) **sin ninguna
  fila** SHALL seguir apareciendo con los contadores a cero y `model: null`, para no perder la
  visibilidad de que un rol nunca corre (p. ej. `collapse` no tiene ni una fila histórica). Orden:
  por `kind` canónico y, dentro, por coste descendente.
- **AÑADIDO — `BackgroundStats` SHALL exponer `suspicious_calls: u64`.** Cuenta las filas con
  `status = 'success'` **y** `completion_tokens <= 1`, **solo** para los orígenes estructurados
  canónicos (nunca `chat`, donde una respuesta de un token puede ser legítima). Es la columna que
  habría cazado la avería el primer día. El umbral SHALL vivir en una constante con nombre.
- **AÑADIDO — `BackgroundStats` SHALL exponer `last_call_at: Option<String>`**: `MAX(created_at)` de
  cada pareja (origen, modelo); `None` si no hay filas.
- **MODIFICADO — `background_summary` SHALL aceptar un `days` opcional** y filtrar
  `created_at >= datetime('now', '-' || ?1 || ' days')`. El handler SHALL usar el patrón ya existente
  (`Query<DayParams>`, `params.days.unwrap_or(...)`) con **7** como valor por defecto. Sin parámetro
  explícito el comportamiento sigue siendo válido y no rompe a ningún cliente. El mismo `days` SHALL
  propagarse a `by_model`.
- **MODIFICADO — `StatsRepo::by_model` SHALL dejar de filtrar `WHERE kind='chat'`** y agrupar por
  modelo **agregando todos los orígenes**, de modo que un modelo que solo trabaja en roles de fondo
  (qwen) aparezca con su coste total. Conserva los campos actuales de `ModelStats` y recibe el mismo
  `days` opcional.
- **MODIFICADO — La UI SHALL mostrar una fila por (rol, modelo)** en la tarjeta «Procesos de fondo»,
  con las columnas actuales más «última llamada», marca visible de sospechosas y errores resaltados
  cuando `> 0`. El selector de ventana del dashboard (por defecto **7 días**) SHALL gobernar los tres
  paneles (diaria, roles y modelos), y el panel de modelos SHALL dejar de ser solo de chat, con
  título y etiqueta acordes.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `db/repos`: un requisito **modificado** (el desglose por modelo, que deja de excluir los orígenes
  de fondo) y otro **modificado** (la agregación de procesos de fondo, que pasa a una fila por
  (origen, modelo) con ventana temporal). Un requisito **añadido** (la marca de llamadas sospechosas
  con salida vacía).
- `frontend`: un requisito **modificado** (la tarjeta de procesos de fondo, fila por (rol, modelo)) y
  otro **modificado** (el panel de coste por modelo, que deja de ser solo de chat). Un requisito
  **añadido** (el selector de ventana que gobierna los tres paneles, con 7 días por defecto).

## Impact

- **Backend:** `src/db/repos/stats.rs` (`BackgroundStats`, `background_summary`, `by_model`) y
  `src/handlers/stats.rs` (propagación de `days` a las dos consultas de fondo).
- **Base de datos:** ninguna migración. Se consultan columnas ya existentes (`model`,
  `completion_tokens`, `created_at`, `status`).
- **Frontend:** la vista de estadísticas (`StatsDashboard`), su tarjeta de procesos de fondo, el
  panel de modelos y el selector de ventana del dashboard.
- **Spec:** delta en `db/repos` (2 MODIFIED + 1 ADDED) y delta en `frontend` (2 MODIFIED + 1 ADDED).
- **Sin cambios** en `docker-compose.prod.yml`.

## Non-goals

- **No se toca `StatsRepo::tools_summary`.** Su `WHERE kind = 'chat'` es correcto: las herramientas
  solo se usan en conversación. Lo mismo para la consulta de `tool_calls`. El filtro por chat es
  incorrecto para `by_model` y correcto para las herramientas; no se generaliza.
- **No se cambia `StatsRepo::summary`.** Sigue filtrando `WHERE kind='chat'`; la asimetría con el
  panel de modelos unificado queda registrada como decisión abierta en el `design.md`, no resuelta
  aquí.
- **No se rediseña el panel de estadísticas** ni se añaden vistas nuevas.
- **No se cambia la retención de datos** (`stats_retention_days`) ni la purga.
- **No se toca `docker-compose.prod.yml`.**
