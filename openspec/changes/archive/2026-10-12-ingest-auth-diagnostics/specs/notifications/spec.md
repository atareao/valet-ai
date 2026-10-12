## MODIFIED Requirements

### Requirement: La ingesta SHALL exigir un token de aplicación válido

`POST /api/notifications` SHALL aceptar el cuerpo de ingesta, SHALL estar exenta de la sesión OIDC y
SHALL exigir un token Bearer de aplicación perteneciente a una clave de ingesta no revocada
(capability `ingest-keys`). Sin un token válido —ausente, malformado, desconocido o de una clave
revocada— la ruta SHALL responder `401` con `Content-Type: application/json` y cuerpo
`{"error":"unauthorized"}`, SHALL registrar el motivo de la denegación y SHALL NOT insertar ninguna
fila. Hacia fuera, la respuesta SHALL ser idéntica en los cuatro casos.

#### Scenario: Una clave válida ingesta con 201

- **Given** una clave de ingesta activa y su token
- **When** se hace `POST /api/notifications` con `Authorization: Bearer <token>` y un cuerpo válido
- **Then** la respuesta es `201`
- **And** el cuerpo es `{"id": "<uuid>"}`

#### Scenario: Sin cabecera de autorización no se ingesta

- **Given** una petición `POST /api/notifications` sin cabecera `Authorization`
- **When** se procesa la petición
- **Then** la respuesta es `401`
- **And** la tabla `notifications` no gana ninguna fila

#### Scenario: Una clave revocada no ingesta

- **Given** una clave de ingesta ya revocada y su token
- **When** se hace `POST /api/notifications` con ese token
- **Then** la respuesta es `401`

#### Scenario: Un token malformado no ingesta

- **Given** una cabecera `Authorization` sin el esquema `Bearer` o con un valor arbitrario
- **When** se hace `POST /api/notifications`
- **Then** la respuesta es `401`

#### Scenario: El 401 de la ingesta lleva cuerpo JSON

- **Given** una petición `POST /api/notifications` sin cabecera `Authorization`
- **When** se procesa la petición
- **Then** la respuesta es `401`
- **And** el `Content-Type` es `application/json` y el cuerpo es `{"error":"unauthorized"}`

#### Scenario: El motivo de la denegación queda registrado sin distinguir la clave revocada de la desconocida

- **Given** una petición cuyo token no corresponde a ninguna clave y otra cuyo token es de una clave revocada
- **When** ambas se procesan
- **Then** las dos respuestas son un `401` con el mismo cuerpo `{"error":"unauthorized"}`
- **And** el log registra el motivo (cabecera ausente, esquema incorrecto o clave desconocida o revocada) sin exponer el token
