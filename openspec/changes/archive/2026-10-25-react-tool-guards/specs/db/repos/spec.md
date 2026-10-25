## ADDED Requirements

### Requirement: Los límites de llamadas a herramientas SHALL vivir en settings y leerse en cada turno

Los cuatro umbrales de las guardas del bucle ReAct SHALL vivir en la tabla `settings` (como los
prompts, los mandos de memoria y los parámetros de generación) y SHALL leerse **en cada turno**, de
modo que cambiarlos surta efecto sin reiniciar. Una migración SHALL sembrarlos con sus defaults y
respetar cualquier valor ya fijado por el usuario (upsert idempotente que solo sobrescribe valores
vacíos/NULL).

| Clave | Significado | Default |
|---|---|---|
| `REACT_TOOL_FINGERPRINT_LIMIT` | ejecuciones permitidas de una llamada idéntica | `2` |
| `REACT_TOOL_FAILURE_LIMIT` | fallos consecutivos por `tool::operation` antes de cortar | `3` |
| `REACT_TOOL_SUCCESS_LIMIT` | llamadas exitosas por `tool::operation` antes de cortar | `50` |
| `REACT_TOOL_TOTAL_LIMIT` | llamadas ejecutadas en total en el turno | `100` |

**Given** la base de datos recién migrada  
**When** se consulta `settings`  
**Then** las cuatro claves existen con sus valores por defecto  

#### Scenario: Las cuatro claves se siembran con sus defaults
**Given** la base de datos recién migrada
**When** se consulta `settings`
**Then** `REACT_TOOL_FINGERPRINT_LIMIT` vale `2`
**And** `REACT_TOOL_FAILURE_LIMIT` vale `3`
**And** `REACT_TOOL_SUCCESS_LIMIT` vale `50`
**And** `REACT_TOOL_TOTAL_LIMIT` vale `100`

#### Scenario: La migración respeta un valor ya fijado
**Given** `settings.REACT_TOOL_SUCCESS_LIMIT = 200` antes de migrar
**When** se ejecuta la migración
**Then** el valor sigue siendo `200`

#### Scenario: Cambiar un umbral surte efecto sin reiniciar
**Given** `settings.REACT_TOOL_FAILURE_LIMIT = 3`
**When** se actualiza a `1` y se repite la lectura sin reiniciar
**Then** el umbral leído es `1`
