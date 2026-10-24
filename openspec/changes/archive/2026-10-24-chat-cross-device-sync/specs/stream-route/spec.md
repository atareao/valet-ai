## ADDED Requirements

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
