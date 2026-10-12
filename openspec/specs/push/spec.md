# push Specification

## Purpose
El transporte WebPush de Valet: las claves VAPID se generan y persisten al primer uso, el navegador
registra y da de baja sus suscripciones, y el backend construye una petición cifrada conforme a
RFC 8030/8188/8291 y la envía con el cliente HTTP del proyecto, depurando las suscripciones
caducadas.

## Requirements

### Requirement: Las claves VAPID SHALL generarse al primer uso y almacenarse como ajustes sensibles

Las claves VAPID SHALL generarse la primera vez que se soliciten, cuando `push_vapid_public_key` o
`push_vapid_private_key` estén ausentes o vacías, y SHALL persistirse ambas en la tabla `settings`
dentro de una misma transacción mediante `INSERT ... ON CONFLICT(key) DO NOTHING` seguido de un
`SELECT` de las dos, de modo que dos peticiones concurrentes converjan en el mismo par. La clave
privada SHALL NO aparecer nunca en un log, ni en una salida de `Debug`, ni en ninguna respuesta
HTTP.

#### Scenario: La primera llamada genera y persiste el par

- **Given** una base de datos sin `push_vapid_public_key` ni `push_vapid_private_key`
- **When** se solicitan las claves VAPID por primera vez
- **Then** se genera un par de claves
- **And** ambas quedan persistidas en `settings`
- **And** la clave pública devuelta no está vacía

#### Scenario: La segunda llamada reutiliza el par existente

- **Given** una base de datos con las claves VAPID ya persistidas
- **When** se vuelven a solicitar las claves
- **Then** no se genera un par nuevo
- **And** la clave pública devuelta es la misma que la anterior

#### Scenario: Dos generaciones concurrentes convergen en un único par

- **Given** una base de datos sin claves VAPID
- **When** dos peticiones solicitan las claves a la vez
- **Then** ambas terminan con el mismo par de claves
- **And** la clave pública con la que se suscriba un cliente es la que luego se usa para firmar

### Requirement: `GET /api/push/vapid-key` SHALL devolver la clave pública VAPID en base64url

El endpoint SHALL responder `200` con el cuerpo `{"public_key": "..."}`, donde el valor es la clave
pública VAPID en base64url. Si las claves no se pueden cargar ni generar, SHALL responder `503` con
el error `push_unavailable` y SHALL NO responder nunca `200` con una clave vacía.

#### Scenario: La clave pública se devuelve con 200

- **Given** un backend con claves VAPID disponibles
- **When** se llama a `GET /api/push/vapid-key`
- **Then** responde `200`
- **And** el cuerpo es un JSON con `public_key` no vacía

#### Scenario: El fallo de generación o lectura se traduce a 503

- **Given** un backend que no puede cargar ni generar las claves VAPID
- **When** se llama a `GET /api/push/vapid-key`
- **Then** responde `503`
- **And** el cuerpo contiene el error `push_unavailable`
- **And** el cuerpo no contiene la clave privada

### Requirement: `POST /api/push/subscriptions` SHALL validar y registrar la suscripción del navegador

El endpoint SHALL aceptar un cuerpo `{"endpoint": "...", "keys": {"p256dh": "...", "auth": "..."}}`
y SHALL validar que `endpoint` sea una URL absoluta con esquema `https`, que `p256dh` decodifique
(base64url sin padding) a 65 bytes que formen un punto válido de la curva P-256, y que `auth`
decodifique a 16 bytes. Cualquier fallo de validación SHALL devolver `400` con `{"error": ...}`. El
`endpoint` SHALL almacenarse en su forma **normalizada** por el parser de URL, no en la cadena cruda,
de modo que lo aceptado en el alta se pueda volver a parsear en cada envío. Una suscripción válida
SHALL registrarse como *upsert* por `endpoint` y SHALL responder `201` con `{"id": "<uuidv4>"}`.
Volver a suscribir el mismo navegador SHALL actualizar `p256dh`, `auth`, `user_agent` y `updated_at`,
y SHALL conservar el `id` original.

#### Scenario: Una suscripción válida crea la fila y responde 201

- **Given** una suscripción con `endpoint` `https`, `p256dh` de 65 bytes y `auth` de 16 bytes
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `201`
- **And** el cuerpo contiene un `id` UUIDv4
- **And** existe una fila nueva en `push_subscriptions`

#### Scenario: Volver a suscribir el mismo endpoint conserva el id

- **Given** una suscripción ya registrada con su `endpoint`
- **When** se vuelve a publicar la misma `endpoint` con claves nuevas
- **Then** la respuesta trae el mismo `id` que la primera vez
- **And** la tabla sigue teniendo una sola fila para ese `endpoint`
- **And** `p256dh` y `auth` quedan actualizados

#### Scenario: Un endpoint que no es https se rechaza con 400

- **Given** una suscripción cuyo `endpoint` es `http://` o no es una URL absoluta
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `400`
- **And** el cuerpo contiene un error
- **And** no se inserta ninguna fila

#### Scenario: Un p256dh de longitud incorrecta se rechaza con 400

- **Given** una suscripción cuyo `p256dh` decodifica a un número de bytes distinto de 65
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `400`
- **And** el cuerpo contiene un error

#### Scenario: Un p256dh que no es un punto P-256 válido se rechaza con 400

- **Given** una suscripción cuyo `p256dh` decodifica a 65 bytes que no forman un punto SEC1 válido de
  la curva P-256
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `400`
- **And** el cuerpo contiene un error
- **And** no se inserta ninguna fila

#### Scenario: El endpoint se guarda normalizado

- **Given** una suscripción cuyo `endpoint` incluye caracteres que el parser de URL normaliza (por
  ejemplo un espacio en el path)
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `201`
- **And** el `endpoint` almacenado es la forma normalizada, no la cadena cruda

#### Scenario: Un auth de longitud incorrecta se rechaza con 400

- **Given** una suscripción cuyo `auth` decodifica a un número de bytes distinto de 16
- **When** se llama a `POST /api/push/subscriptions`
- **Then** responde `400`
- **And** el cuerpo contiene un error

### Requirement: `DELETE /api/push/subscriptions` SHALL eliminar la suscripción por endpoint y SHALL ser idempotente

El endpoint SHALL aceptar un cuerpo `{"endpoint": "..."}` y SHALL eliminar la suscripción
correspondiente. SHALL responder `204` siempre, exista o no la fila, y SHALL responder `400` cuando
falte `endpoint` en el cuerpo.

#### Scenario: Eliminar una suscripción existente

- **Given** una suscripción registrada con su `endpoint`
- **When** se llama a `DELETE /api/push/subscriptions` con ese `endpoint`
- **Then** responde `204`
- **And** la fila ya no existe

#### Scenario: Eliminar un endpoint desconocido también responde 204

- **Given** un `endpoint` que no corresponde a ninguna fila
- **When** se llama a `DELETE /api/push/subscriptions`
- **Then** responde `204`
- **And** no se produce ningún error

#### Scenario: Un cuerpo sin endpoint se rechaza con 400

- **Given** una petición sin `endpoint` en el cuerpo
- **When** se llama a `DELETE /api/push/subscriptions`
- **Then** responde `400`

### Requirement: El envío SHALL usar el cliente HTTP del proyecto

La petición de envío SHALL construirse con `web-push-native`, que devuelve un
`http::Request<Vec<u8>>`, y SHALL enviarse con el `reqwest::Client` ya presente en el proyecto,
configurado con rustls. SHALL NO introducirse un segundo stack TLS: ni OpenSSL, ni `native-tls`, ni
un cliente `isahc`.

#### Scenario: La petición resultante es un POST al endpoint

- **Given** una suscripción y un mensaje a enviar
- **When** se construye la petición de envío
- **Then** el método es `POST`
- **And** la URL destino es el `endpoint` de la suscripción

#### Scenario: No hay dependencia de openssl en el árbol

- **Given** el árbol de dependencias del proyecto
- **When** se inspecciona en busca de enlaces a `openssl` o `native-tls`
- **Then** no existe ninguna dependencia de `openssl` ni de `native-tls`
- **And** el cliente `isahc` no está presente

### Requirement: La petición construida SHALL seguir el formato RFC 8291 (aes128gcm) y RFC 8292 (VAPID)

La petición construida SHALL tener un cuerpo que empiece por `salt` (16 bytes), `rs` (4 bytes
big-endian), `idlen` (1 byte, valor 65) y la clave pública efímera (65 bytes, con primer byte
`0x04`), y SHALL incluir las cabeceras `TTL`, `Content-Encoding: aes128gcm`, `Content-Length` y
`Authorization` empezando por `vapid t=`. Todo ello SHALL ser verificable sin red.

#### Scenario: Las cabeceras están presentes y son correctas

- **Given** una suscripción y un mensaje a enviar
- **When** se construye la petición de envío
- **Then** incluye la cabecera `TTL`
- **And** incluye `Content-Encoding: aes128gcm`
- **And** incluye `Content-Length`
- **And** la cabecera `Authorization` empieza por `vapid t=`

#### Scenario: La cabecera del cuerpo respeta los offsets del RFC

- **Given** la petición de envío construida
- **When** se inspeccionan los primeros bytes del cuerpo
- **Then** los primeros 16 bytes son el `salt`
- **And** los 4 bytes siguientes son `rs` en big-endian
- **And** el byte siguiente es `idlen` con valor 65
- **And** los 65 bytes siguientes son la clave pública efímera, que empieza por `0x04`

#### Scenario: Sin payload no hay firma silenciosa

- **Given** un mensaje sin payload
- **When** se construye la petición de envío
- **Then** la cabecera `Authorization` solo existe si hay payload
- **And** el comportamiento real queda documentado en la prueba de contrato

### Requirement: El envío SHALL depurar las suscripciones caducadas

Si el Push Service responde `404` o `410`, la suscripción SHALL eliminarse de la base de datos. Otros
errores SHALL registrarse y SHALL NOT borrar la suscripción. El fallo de una suscripción SHALL NO
propagarse al resto: un envío a N suscripciones SHALL intentarse con todas.

#### Scenario: Un 410 borra la fila y no detiene el resto

- **Given** dos suscripciones, una de las cuales responde `410`
- **When** se envía el mensaje a ambas
- **Then** la suscripción que respondió `410` se borra
- **And** el envío a la otra se intenta igualmente

#### Scenario: Un 500 conserva la fila y registra el error

- **Given** una suscripción que responde `500`
- **When** se intenta el envío
- **Then** la suscripción NO se borra
- **And** el error se registra

#### Scenario: Un fallo no impide enviar a las demás

- **Given** varias suscripciones, una de las cuales falla
- **When** se envía el mensaje
- **Then** todas las demás reciben su envío
- **And** el fallo de la primera no cortocircuita el resto
