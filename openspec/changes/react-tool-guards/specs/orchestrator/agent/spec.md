## REMOVED Requirements

### Requirement: Per-tool max retry limit of 3 in ReAct loop

Eliminado: sustituido por el requisito «Guardas del bucle ReAct para llamadas a herramientas». El
contador único por nombre de tool no distinguía progreso de bucle, y su umbral estaba desalineado
(spec `3`, constante `5`, mensaje `3`).

## ADDED Requirements

### Requirement: Guardas del bucle ReAct para llamadas a herramientas

El orquestador SHALL limitar las llamadas a herramientas en el bucle ReAct con **tres guardas
ortogonales** en lugar de un contador único por nombre de herramienta, de modo que el **progreso**
(muchas llamadas con argumentos distintos) no se confunda con un **bucle** (la misma llamada
repetida o un fallo repetido). Los umbrales SHALL leerse de `settings` **en cada turno**
(`REACT_TOOL_FINGERPRINT_LIMIT` default `2`, `REACT_TOOL_FAILURE_LIMIT` default `3`,
`REACT_TOOL_SUCCESS_LIMIT` default `50`, `REACT_TOOL_TOTAL_LIMIT` default `100`), cayendo al default
con un `warn!` si la clave falta o no es parseable. Los contadores SHALL vivir en memoria y
reiniciarse en cada turno. Las tres guardas son:

1. **Huella** — la repetición de la **misma** llamada (mismo nombre de tool, misma `operation` y
   mismos argumentos normalizados), con independencia de su resultado, SHALL bloquearse tras
   `REACT_TOOL_FINGERPRINT_LIMIT` ejecuciones.
2. **Presupuesto por `tool::operation`** — contabilizado aparte para **éxitos** y **fallos
   consecutivos**; SHALL bloquearse al alcanzar `REACT_TOOL_SUCCESS_LIMIT` éxitos o
   `REACT_TOOL_FAILURE_LIMIT` fallos consecutivos. Un éxito SHALL reiniciar el contador de fallos
   consecutivos.
3. **Tope global del turno** — SHALL bloquearse toda llamada al alcanzar `REACT_TOOL_TOTAL_LIMIT`
   llamadas ejecutadas en el turno.

El orden de comprobación SHALL ser: tope global, huella, éxito, fallo. La comprobación SHALL
hacerse **antes** de ejecutar la herramienta y la actualización de contadores **después**, según el
`success` real del resultado (un `ToolResult { success: false }` cuenta como fallo). Un corte NO
DEBE romper el bucle: se DEBE pasar al LLM un mensaje `tool` con el **motivo exacto** y continuar,
se DEBE emitir `SSEEvent::ToolResult { success: false }` y se DEBE registrar un `warn!` con el
nombre de la herramienta y el motivo.

**Given** un turno con `REACT_TOOL_SUCCESS_LIMIT = 50`, `REACT_TOOL_FAILURE_LIMIT = 3`, `REACT_TOOL_FINGERPRINT_LIMIT = 2` y `REACT_TOOL_TOTAL_LIMIT = 100`  
**When** el LLM llama a la misma herramienta con argumentos distintos y todas triunfan  
**Then** ninguna llamada se bloquea hasta el éxito número 50  

#### Scenario: Marcar N elementos distintos no se bloquea
**Given** el default `REACT_TOOL_SUCCESS_LIMIT = 50`
**When** el LLM llama a `email::mark_read` 10 veces, cada una con un `id` distinto, y todas triunfan
**Then** las 10 llamadas se ejecutan
**And** ninguna se bloquea por la guarda de éxito ni por la de huella

#### Scenario: La misma llamada repetida se corta por huella
**Given** `REACT_TOOL_FINGERPRINT_LIMIT = 2`
**When** el LLM llama a `geo::search` dos veces con **argumentos idénticos**
**Then** ambas se ejecutan
**When** el LLM la llama una tercera vez con los mismos argumentos
**Then** la tercera NO se ejecuta
**And** el mensaje al LLM nombra el motivo «huella» (no «3 times»)
**And** el ReAct loop continúa

#### Scenario: Los fallos consecutivos cortan antes que los éxitos
**Given** `REACT_TOOL_FAILURE_LIMIT = 3`
**When** el LLM llama a la misma `tool::operation` con argumentos distintos y falla 3 veces seguidas
**Then** la siguiente llamada al mismo `tool::operation` se bloquea
**And** el mensaje nombra el motivo «fallo»

#### Scenario: Un éxito reinicia el contador de fallos
**Given** `REACT_TOOL_FAILURE_LIMIT = 3`
**When** el tool falla 2 veces seguidas y luego triunfa
**Then** el contador de fallos consecutivos vuelve a 0
**And** dos fallos posteriores no bloquean todavía

#### Scenario: El tope global corta cualquier herramienta
**Given** `REACT_TOOL_TOTAL_LIMIT = 5`
**When** se han ejecutado 5 llamadas en el turno (de cualquier herramienta)
**Then** cualquier llamada posterior se bloquea
**And** el mensaje nombra el motivo «tope global»

#### Scenario: Los contadores se reinician en el turno siguiente
**Given** un turno que alcanzó un umbral y bloqueó una herramienta
**When** empieza el turno siguiente
**Then** los contadores arrancan a cero
**And** la misma herramienta vuelve a poder ejecutarse

#### Scenario: Un umbral no parseable cae al default
**Given** `REACT_TOOL_SUCCESS_LIMIT = "muchos"`
**When** el orquestador lee sus umbrales
**Then** `success_limit` es `50`
**And** se registra un `warn!`

## MODIFIED Requirements

### Requirement: Tool error logging

El orquestador SHALL registrar a nivel `error!` los errores de herramientas con nombre, argumentos y mensaje completo.

All tool execution errors must be logged with maximum detail including
tool name, arguments, and the full error message (Display + Debug).

#### Scenario: ToolError from registry.execute() is logged with error!
When a tool returns `Err(ToolError)`, the orchestrator must log:
- `tool_name` — the tool that failed
- `tool_args` — the JSON arguments passed to the tool
- `error` — the error Display text
- `error_debug` — the error Debug representation

#### Scenario: ToolResult success=false is logged with error!
When a tool returns `ToolResult { success: false, ... }`, the orchestrator
must log the tool name, arguments, and error message.

#### Scenario: Tool retry limit reached is logged with warn!
When one of the three ReAct tool guards (per-turn total, fingerprint, success budget or
consecutive-failure budget) blocks a tool call, the orchestrator must log a warning with the
tool name, the call count and the **guard that tripped**.
