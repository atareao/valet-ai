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

### Requirement: El stream SSE SHALL declararse no transformable ante proxies intermedios

La respuesta del endpoint de streaming SHALL incluir `Cache-Control: no-cache, no-transform` y
`X-Accel-Buffering: no`, de modo que un proxy intermedio (Traefik, nginx) no comprima ni acumule el
cuerpo y los eventos sigan llegando de forma incremental. Las cabeceras ya presentes
(`Content-Type: text/event-stream` y `cache-control: no-cache`) SHALL mantenerse.

#### Scenario: Un cliente que anuncia compresión recibe la respuesta sin transformar
**Given** un cliente que envía `Accept-Encoding: gzip, deflate, br`
**When** pide `/api/chat/stream`
**Then** la respuesta lleva `Content-Type: text/event-stream`
**And** `Cache-Control` incluye `no-transform`
**And** no lleva `Content-Encoding`

#### Scenario: La respuesta declara el no-bufferizado para proxies
**Given** una petición a `/api/chat/stream`
**When** se inspeccionan sus cabeceras
**Then** incluye `X-Accel-Buffering: no`
**And** incluye `Cache-Control: no-cache, no-transform`

#### Scenario: Los eventos siguen llegando de forma incremental
**Given** un cliente que consume el stream hasta el final
**When** el modelo produce varios fragmentos
**Then** los eventos `chunk` se leen repartidos en el tiempo
**And** no se acumulan en un único bloque final

### Requirement: GET /api/chat/events SHALL difundir el estado del turno y los mensajes nuevos

`GET /api/chat/events?since=<cursor>` SHALL devolver un stream SSE de larga duración. Cada evento SHALL viajar como un objeto JSON con un campo `type` que lo discrimina. Al abrirse, el endpoint SHALL emitir, en este orden: (1) un evento `{"type":"snapshot","busy":<bool>}` con el estado del turno en ese instante; (2) si se informa `since`, los mensajes persistidos **estrictamente posteriores** a ese cursor, en orden ascendente, cada uno como evento `{"type":"message_created","message":<mensaje>,"cursor":"<cursor>"}`; y (3) a partir de ahí, el flujo en vivo. El mensaje SHALL llevar la fila completa, con la misma forma que devuelve `GET /api/messages`, y el campo `cursor` SHALL ser el cursor **opaco** de ese mensaje, de modo que un cliente pueda reanudar sin conocer su formato. El inicio y el fin de turno SHALL emitirse como `{"type":"turn_started"}` y `{"type":"turn_finished"}`. Cada mensaje que el orquestador persista SHALL difundirse a **todos** los clientes suscritos, incluido el que originó el turno. El endpoint SHALL quedar bajo la misma sesión autenticada que el resto de la API y SHALL admitir `since` como único parámetro.

#### Scenario: Un cliente recibe el mensaje escrito en otro
**Given** dos clientes suscritos a `/api/chat/events`
**When** uno de ellos envía un mensaje por `POST /api/chat/stream` y el turno completa
**Then** ambos reciben el evento `message_created` con la fila del mensaje de usuario
**And** ambos reciben el evento `message_created` con la fila del mensaje del asistente

#### Scenario: El estado inicial informa de un turno en curso
**Given** un turno en curso
**When** un cliente abre `/api/chat/events` sin `since`
**Then** el primer evento que recibe es `snapshot` con `busy: true`

#### Scenario: Sin turno en curso el estado inicial dice que está libre
**Given** ningún turno en curso
**When** un cliente abre `/api/chat/events`
**Then** el primer evento que recibe es `snapshot` con `busy: false`

#### Scenario: El replay recupera lo perdido y solo lo perdido
**Given** un cliente que conoce el cursor del último mensaje que vio
**And** se han persistido dos mensajes posteriores a ese cursor
**When** abre `/api/chat/events?since=<cursor>`
**Then** recibe exactamente dos eventos `message_created`, en orden ascendente
**And** ninguno es anterior o igual al cursor

#### Scenario: Sin mensajes posteriores el replay está vacío
**Given** un cursor igual al del último mensaje persistido
**When** abre `/api/chat/events?since=<cursor>`
**Then** no recibe ningún evento `message_created`
**And** sí recibe el `snapshot`

#### Scenario: Cada mensaje difundido trae su cursor
**Given** un cliente suscrito
**When** recibe un evento `message_created`
**Then** el evento incluye un campo `cursor` no vacío
**And** con ese valor como `since` no vuelve a recibir ese mismo mensaje

#### Scenario: Un cursor ilegible se rechaza sin abrir el stream
**When** se pide `/api/chat/events?since=no-es-un-cursor`
**Then** la respuesta es `400` con un mensaje de error legible
**And** no se abre ningún stream de eventos

### Requirement: Un suscriptor que se queda atrás SHALL avisarse en vez de perder mensajes en silencio

Si un cliente suscrito no consume los eventos a la velocidad a la que se publican y la cola del bus se desborda, el servidor SHALL emitir un evento `{"type":"resync"}` y cerrar el stream, en lugar de descartar eventos en silencio. El cliente SHALL reaccionar reabriendo la suscripción con el último cursor que conozca. `resync` SHALL NOT emitirse en el flujo normal de un cliente que va al día.

#### Scenario: Un suscriptor lento recibe el aviso y el stream se cierra
**Given** un cliente suscrito que deja de consumir el stream
**When** se publican más eventos de los que caben en la cola del bus
**Then** el cliente recibe un evento `resync`
**And** el stream se cierra

#### Scenario: Un cliente que va al día nunca ve el aviso
**Given** un cliente suscrito que consume cada evento al recibirlo
**When** se publican varios mensajes
**Then** recibe los eventos `message_created` correspondientes
**And** no recibe ningún `resync`

### Requirement: El stream de eventos SHALL sobrevivir a los proxies intermedios y a la inactividad

La respuesta de `GET /api/chat/events` SHALL incluir `Cache-Control: no-cache, no-transform` y `X-Accel-Buffering: no`, de modo que un proxy intermedio (Traefik, nginx) no comprima ni acumule el cuerpo y los eventos lleguen de forma incremental. El stream SHALL además emitir un comentario de latido cuando pasen 15 segundos sin actividad, de modo que ningún proxy cierre la conexión por inactividad; el latido SHALL ser un comentario SSE y SHALL NOT contar como evento para el cliente.

#### Scenario: Un cliente que anuncia compresión recibe la respuesta sin transformar
**Given** un cliente que envía `Accept-Encoding: gzip, deflate, br`
**When** pide `/api/chat/events`
**Then** la respuesta lleva `Content-Type: text/event-stream`
**And** `Cache-Control` incluye `no-transform`
**And** no lleva `Content-Encoding`

#### Scenario: Una conexión ociosa no se cae y sigue viva
**Given** un cliente suscrito sin turnos en curso
**When** transcurren 40 segundos sin ningún evento de chat
**Then** el cliente sigue conectado
**And** ha recibido al menos dos latidos

#### Scenario: Los latidos no confunden al cliente
**Given** un cliente que consume el stream
**When** recibe latidos entre eventos
**Then** ningún latido se interpreta como un tipo de evento del chat

### Requirement: POST /api/chat/stream SHALL admitir un identificador de mensaje propuesto por el cliente

El cuerpo de `POST /api/chat/stream` SHALL admitir un campo opcional `client_message_id`. Cuando llegue una cadena no vacía de 64 caracteres o menos que no corresponda a ningún mensaje ya persistido, el servidor SHALL usar ese valor como identificador del mensaje de usuario. En cualquier otro caso —ausente, vacío, demasiado largo o ya usado— el servidor SHALL generar un identificador propio y SHALL NOT fallar el turno por ello. El evento `done` SHALL seguir informando el identificador realmente persistido en `user_message_id`.

#### Scenario: El id propuesto se respeta
**Given** un cuerpo con `client_message_id: "cli-abc"`
**When** `POST /api/chat/stream` con ese cuerpo
**Then** el mensaje de usuario se persiste con id `cli-abc`
**And** el evento `done` lo informa como `user_message_id`

#### Scenario: Sin id propuesto el servidor genera uno
**Given** un cuerpo sin `client_message_id`
**When** `POST /api/chat/stream`
**Then** el mensaje de usuario se persiste con un id generado por el servidor
**And** el evento `done` lo informa como `user_message_id`

#### Scenario: Un id ya usado no rompe el turno
**Given** un `client_message_id` que ya corresponde a un mensaje persistido
**When** `POST /api/chat/stream` con ese cuerpo
**Then** el turno procede con normalidad
**And** el mensaje de usuario se persiste con un id distinto generado por el servidor

#### Scenario: Un id inválido no rompe el turno
**Given** un `client_message_id` vacío o de más de 64 caracteres
**When** `POST /api/chat/stream` con ese cuerpo
**Then** el turno procede con normalidad
**And** el mensaje de usuario se persiste con un id generado por el servidor

### Requirement: El chat SHALL admitir un único turno a la vez

`POST /api/chat/stream` SHALL rechazar la petición con `409` y un cuerpo `{ "error": <texto>, "busy": true }` cuando ya haya un turno en curso, sin persistir ningún mensaje de usuario y sin arrancar ningún procesamiento. El turno admitido SHALL liberar el cerrojo al terminar, **tanto si completa como si falla**, y SHALL publicar el inicio y el fin del turno en el bus de eventos. `POST /api/approval/{request_id}` SHALL NOT requerir el cerrojo: resuelve una aprobación del turno en curso.

#### Scenario: El segundo envío simultáneo recibe un aviso claro
**Given** un turno en curso
**When** un segundo cliente envía `POST /api/chat/stream`
**Then** la respuesta es `409` con `busy: true` y un texto legible
**And** no se persiste ningún mensaje de usuario para el segundo envío
**And** el primer turno no se interrumpe

#### Scenario: El resto de clientes se entera de que hay un turno en curso
**Given** dos clientes suscritos a `/api/chat/events`
**When** uno de ellos empieza un turno y éste termina
**Then** ambos reciben un evento `turn_started`
**And** ambos reciben un evento `turn_finished`

#### Scenario: Un turno que falla libera el cerrojo
**Given** un turno que termina con error del orquestador
**When** un cliente envía otro mensaje
**Then** la petición NO recibe `409`
**And** el nuevo turno arranca con normalidad

#### Scenario: Resolver una aprobación no queda bloqueado por el cerrojo
**Given** un turno en curso que está esperando una aprobación
**When** se resuelve la aprobación con `POST /api/approval/{request_id}`
**Then** la respuesta es `200`
**And** el turno en curso continúa

---
