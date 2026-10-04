# Spec Delta: orchestrator/agent

## ADDED Requirements

### Requirement: La ejecución exitosa de `render_widget` SHALL emitir un evento SSE `widget`

Cuando el bucle ReAct ejecuta una tool call cuyo nombre es `render_widget` y la ejecución tiene éxito,
el orquestador SHALL emitir un `SSEEvent` de tipo `widget` con un `id` único no vacío, el `name` del
widget y el `data` recibido, antes del evento `done` del turno. El resultado de la herramienta SHALL
seguir inyectándose en el bucle como mensaje `tool`, de forma que el turno continúe.

**Given** un turno de streaming en curso
**When** el modelo invoca `render_widget` con éxito
**Then** el cliente recibe un evento `widget` y el turno termina con `done`

#### Scenario: La tool call produce un evento `widget`

**Given** una tool call a `render_widget` con `widget_name: "QuickForm"` y `data` válido
**When** el orquestador procesa la tool call con éxito
**Then** emite un `SSEEvent` de tipo `widget` con `name: "QuickForm"` y el mismo `data`
**And** el `id` del evento no está vacío

#### Scenario: Un widget inválido no emite evento

**Given** una tool call a `render_widget` con un `widget_name` no permitido
**When** el orquestador procesa la tool call y la ejecución falla
**Then** no se emite ningún evento de tipo `widget`
**And** el turno continúa sin romperse

#### Scenario: El turno sigue tras el widget

**Given** una tool call a `render_widget` ejecutada con éxito
**When** el orquestador prosigue el bucle ReAct
**Then** el resultado de la herramienta se añade como mensaje `tool`
**And** el turno termina con un evento `done`
