## MODIFIED Requirements

### Requirement: POST /api/chat/stream SHALL admitir un identificador de mensaje propuesto por el cliente

El cuerpo de `POST /api/chat/stream` SHALL admitir un campo opcional `client_message_id`. Cuando llegue
una cadena no vacía de 64 caracteres o menos que no corresponda a ningún mensaje ya persistido, el
servidor SHALL usar ese valor como identificador del mensaje de usuario. En cualquier otro caso
—ausente, vacío, demasiado largo o ya usado— el servidor SHALL generar un identificador propio y SHALL
NOT fallar el turno por ello. El evento `done` SHALL seguir informando el identificador realmente
persistido en `user_message_id`.

El evento `done` SHALL llevar además, en un campo `content`, el contenido **íntegro** del mensaje del
asistente tal y como ha quedado persistido —sin recortes— (ver `orchestrator/agent`). Ese valor es la
**base autoritativa** contra la que están calculados los `span` de los widgets proyectados del mensaje, de
modo que el cliente componga el texto sobre el mismo texto que indexan los tramos y no sobre el suyo
acumulado. El campo SHALL estar presente aunque el contenido sea una cadena vacía, y un cliente que lo
ignore SHALL seguir funcionando (conserva el texto streameado y pinta los widgets debajo).

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

#### Scenario: El `done` lleva el contenido íntegro del asistente

**Given** un turno en el que el servidor proyectó un widget
**When** el turno termina
**Then** el evento `done` SHALL incluir un campo `content`
**And** ese `content` SHALL coincidir con el contenido persistido del mensaje del asistente
**And** SHALL contener el texto íntegro del asistente, incluido el bloque que el widget ocupa

#### Scenario: Un cliente que ignora `content` sigue funcionando

**Given** un cliente que no lee el campo `content` del evento `done`
**When** el turno termina con un widget proyectado
**Then** el cliente conserva el texto que streameó
**And** no se produce ningún error por el campo nuevo
