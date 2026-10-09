# Spec Delta: frontend

## Purpose

Interfaz de usuario de Valet: chat, agenda, tareas, estadísticas y ajustes, con estilos globales y
tipografía configurable.

## ADDED Requirements

### Requirement: El chat SHALL pedir confirmación para herramientas destructivas

Ante un evento SSE `approval_required`, el chat SHALL mostrar la petición con el nombre de la
herramienta y el motivo, y SHALL ofrecer las acciones Permitir y Denegar. Al decidir SHALL llamar a
`POST /api/approval/{request_id}` y SHALL mantener abierto el stream para continuar la respuesta. El
estado pendiente SHALL limpiarse al recibir `approval_result`, `done` o `error`.

#### Scenario: La petición de aprobación se muestra

**Given** el chat recibiendo un evento `approval_required` con `request_id`, `tool_name` y `reason`
**When** llega el evento
**Then** se muestra un diálogo con el nombre de la herramienta y el motivo
**And** el diálogo ofrece Permitir y Denegar

#### Scenario: Aprobar reanuda la respuesta

**Given** un diálogo de aprobación visible
**When** el usuario pulsa Permitir
**Then** se llama a `POST /api/approval/{request_id}` con `approved: true`
**And** el diálogo se cierra
**And** el stream continúa y la respuesta se completa con `done`

#### Scenario: Denegar cierra el diálogo y continúa

**Given** un diálogo de aprobación visible
**When** el usuario pulsa Denegar
**Then** se llama a `POST /api/approval/{request_id}` con `approved: false`
**And** el diálogo se cierra
**And** el stream continúa hasta `done`

#### Scenario: El estado pendiente se limpia

**Given** una aprobación resuelta
**When** llega `approval_result`, `done` o `error`
**Then** no queda ningún diálogo de aprobación visible
