# Design — Guardas del bucle ReAct para llamadas a herramientas

## Contexto (estado actual)

`src/orchestrator/agent.rs`:
- `const MAX_TOOL_RETRIES: usize = 5;` (l.256).
- El bucle ReAct (`'react_loop`, l.1123) lee `self.config.max_iterations` (default `10`).
- Dentro de la rama `allowed` del guardrail (l.1372) se forma la clave
  `format!("{}::{}", tc.name, op)` (o `tc.name`), se incrementa un
  `HashMap<String, usize>` (`tool_call_counts`, declarado en l.839, por turno) y si supera
  `MAX_TOOL_RETRIES` se emite un mensaje fijo («…called 3 times…»), un `SSEEvent::ToolResult
  { success: false }` y `continue`.

Problema: la clave no mira argumentos ni resultado, así que «progreso» y «bucle» comparten contador.

## Las tres guardas

### Capa 1 — Huella (repetición idéntica)
- `fingerprint = (tool_name, operation, canonical_json(args))`, con `args` **antes** de inyectar
  `profile_id` (la inyección ocurre después, l.1420-1427) y claves ordenadas para el hash.
- Se cuenta el número de **ejecuciones** de esa huella en el turno. Se bloquea si
  `count + 1 > fingerprint_limit`.
- Default `2`: una llamada idéntica puede ejecutarse dos veces (cubre el reintento legítimo de un
  fallo transitorio) y la tercera se bloquea. **Independiente del resultado**: repetir una llamada
  que ya triunfó es inútil, y repetir una que falla es el bucle que queremos cortar.

### Capa 2 — Presupuesto por `tool::operation`
- Clave = `tool::operation` con `operation` si existe, si no el nombre del tool (idéntico a hoy).
- Dos contadores por clave: `successes` y `consecutive_failures`.
  - Éxito → `successes += 1`, `consecutive_failures = 0`.
  - Fallo → `consecutive_failures += 1`.
- Se bloquea cuando `successes >= success_limit` (default `50`) o
  `consecutive_failures >= failure_limit` (default `3`), **antes** de ejecutar.

### Capa 3 — Tope global del turno
- `total_executed` cuenta toda llamada ejecutada en el turno. Se bloquea al alcanzar
  `total_limit` (default `100`).

## Semántica y orden

- Los contadores viven **en memoria** y se **reinician en cada turno** (igual que el
  `tool_call_counts` actual, que se declara dentro del turno).
- Comprobación **antes** de ejecutar, en este orden y ganando el **primer** corte:
  tope global → huella → éxito → fallo. El motivo del corte se refleja en el mensaje.
- Actualización **después** de ejecutar, con el `success` real del `ToolResult`. Un error de
  ejecución (`ToolResult { success: false }` o un `Err`) cuenta como **fallo** para la capa 2.
- `max_iterations = 10` sigue siendo la cota externa del bucle; **no se toca**.

## Lectura de umbrales

- Módulo nuevo `src/orchestrator/tool_limits.rs`, patrón `src/generation.rs`:
  - `struct ToolLimits { fingerprint, failure, success, total: usize }` y
    `read_tool_limits(pool) -> ToolLimits`.
  - Lee las cuatro claves `REACT_TOOL_*` **en cada turno**; una clave **ausente** cae al default en
    silencio, y un valor **no parseable** (o un error de lectura de la BD) cae al default con un
    `tracing::warn!` — el mismo contrato que `src/generation.rs`.
- No se cachea: mismo contrato que los `GENERATION_*` (editar en la UI y que surta efecto sin
  reiniciar).

## Mensaje al modelo (exacto y por motivo)

Se corrige el texto fijo «3 times». Cada corte nombra su causa:
- huella: `Tool '{key}' has been called {n} times with identical arguments; no further identical calls are allowed. Change the arguments or use another approach.`
- fallo: `Tool '{key}' has failed {n} times in a row; no more retries are allowed. Inform the user and suggest alternatives.`
- éxito: `Tool '{key}' reached its per-turn limit of {n} calls.`
- global: `The turn reached its tool-call limit ({n}); no further tools will run.`

Se conserva el comportamiento de emitir `SSEEvent::ToolResult { name, success: false }` y de
continuar el bucle para que el LLM responda con alternativas, y se registra un `warn!` con nombre,
cuenta y motivo.

## Interfaz

- Sección «Límites del agente» en `SettingsDialog`, con cuatro campos numéricos (min 1) rotulados
  con las claves. Se persisten como cualquier ajuste (`GET`/`PUT /api/settings`); no requieren ruta
  ni contrato nuevos.

## Alternativas descartadas

- **Subir `MAX_TOOL_RETRIES` a secas:** empeora el modo «bucle» (ver `proposal.md`).
- **Un único contador por huella:** no permite lotes grandes con argumentos distintos.
- **Umbral por tokens o por tiempo:** no determinista y más difícil de razonar; los contadores son
  baratos y predecibles.
