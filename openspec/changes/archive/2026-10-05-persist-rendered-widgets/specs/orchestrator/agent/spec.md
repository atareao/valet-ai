# Spec Delta: orchestrator/agent

## ADDED Requirements

### Requirement: El orquestador SHALL persistir los widgets renderizados con el mensaje del asistente

Cuando un turno emite uno o varios eventos `widget` (es decir, ejecuta `render_widget` con éxito), el
orquestador SHALL persistir esos widgets —con el **mismo `id`** del evento, su `name` y su `data`— en la
columna `widgets` del mensaje assistant del turno, de forma que el frontend pueda reconstruirlos al
recargar. Un turno sin widgets SHALL dejar `widgets` como `null`.

#### Scenario: Un turno con widget persiste la lista
**Given** un turno en el que el modelo invoca `render_widget` con éxito
**When** el orquestador persiste el mensaje assistant
**Then** ese mensaje SHALL tener `widgets` con el `id` del evento `widget` emitido, su `name` y su `data`

#### Scenario: Un turno sin widgets no persiste nada
**Given** un turno sin ninguna invocación a `render_widget`
**When** el orquestador persiste el mensaje assistant
**Then** `widgets` SHALL ser `null`
