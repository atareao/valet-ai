# Spec Delta: stream-route

## Purpose

Rutas SSE para streaming de chat. Contiene el endpoint `POST /api/chat/stream` y el endpoint de
resolución de aprobaciones `POST /api/approval/{request_id}`.

## ADDED Requirements

### Requirement: El endpoint de aprobación SHALL resolver y señalar el turno en espera

`POST /api/approval/{request_id}` SHALL aceptar un cuerpo `{ "approved": bool }`, resolver la
solicitud pendiente y señalizar al turno que espera. SHALL devolver:
`200 { "status": "resolved", "approved": <valor pedido> }` al resolver; `404` si el id no existe;
`409` si ya estaba resuelta. Cuando no haya `Guardrails` configurado (tests), SHALL responder el stub
reflejando el `approved` del cuerpo, sin forzar `true`.

#### Scenario: Aprobar resuelve el turno en espera

**Given** una solicitud pendiente con id `req-1` y un turno esperando
**When** `POST /api/approval/req-1` con `{ "approved": true }`
**Then** la respuesta es `200` con `approved: true`
**And** el turno en espera se reanuda

#### Scenario: Denegar devuelve el valor pedido

**Given** una solicitud pendiente con id `req-1`
**When** `POST /api/approval/req-1` con `{ "approved": false }`
**Then** la respuesta es `200` con `approved: false`

#### Scenario: Id desconocido devuelve 404

**When** `POST /api/approval/no-existe` con `{ "approved": true }`
**Then** la respuesta es `404`

#### Scenario: Doble resolución mientras sigue registrada devuelve 409

**Given** una solicitud resuelta que aún no ha sido consumida por el turno en espera
**When** se resuelve de nuevo
**Then** la respuesta es `409`

#### Scenario: Resolver una solicitud ya consumida devuelve 404

**Given** una solicitud ya consumida por el turno y por tanto no registrada
**When** se resuelve de nuevo
**Then** la respuesta es `404`

#### Scenario: El stub sin guardrails refleja el cuerpo

**Given** un `AppState` sin `guardrails` (tests)
**When** `POST /api/approval/req-X` con `{ "approved": false }`
**Then** la respuesta es `200` con `approved: false`
