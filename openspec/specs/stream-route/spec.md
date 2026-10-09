# stream-route Specification

## Purpose
Rutas SSE para streaming de chat. Contiene el endpoint `POST /api/chat/stream`.

## Requirements

### Requirement: profile_id must resolve from DB, not be hardcoded

El handler SHALL resolver el `profile_id` desde la tabla `profiles` (vía `ProfilesRepo::get_or_create`) y SHALL NOT usar un ID hardcodeado.
**Given** una base de datos sin perfil con id="profile-1"  
**When** se llama a `POST /api/chat/stream`  
**Then** el profile_id inyectado en los tool calls debe ser un ID real existente en la tabla `profiles`

#### Scenario: Hardcoded "profile-1" no existe en producción
**Given** una base de datos recién migrada (sin seed)  
**When** `ProfilesRepo::get_or_create(pool)` devuelve un profile con id UUID  
**Then** ese UUID debe ser el profile_id que se pase al orquestador  
**And** no debe usarse `"profile-1"` como string hardcodeado

#### Scenario: El handler resuelve el profile correctamente
**Given** un `AppState` con base de datos y orquestador  
**When** se envía una petición `POST /api/chat/stream`  
**Then** el stream debe funcionar sin errores de FK  
**And** debe usar el ID devuelto por `ProfilesRepo::get_or_create()`

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
