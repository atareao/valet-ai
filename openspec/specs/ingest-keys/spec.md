# ingest-keys Specification

## Purpose
Las claves de ingesta de Valet: credenciales de aplicación que autorizan `POST /api/notifications`
sin sesión OIDC. Cada clave se identifica por su nombre, se autentica por el hash SHA-256 de su
token y se revoca —sin borrarse— cuando deja de usarse. El token se muestra una única vez y solo se
persiste su hash y su prefijo, de modo que un volcado de la base de datos o del listado nunca
revela una credencial utilizable.

## Requirements

### Requirement: La tabla `ingest_keys` SHALL persistir solo el hash y el prefijo del token

La tabla `ingest_keys` SHALL tener las columnas `id` (`TEXT` PK), `name` (`TEXT` NOT NULL
**UNIQUE**), `key_hash` (`TEXT` NOT NULL **UNIQUE**, el SHA-256 en hexadecimal del token), `prefix`
(`TEXT` NOT NULL, los **primeros 12 caracteres** del token, para identificarlo en la lista sin
guardarlo), `created_at` (`TEXT` NOT NULL DEFAULT `(datetime('now'))`), `last_used_at` (`TEXT`
NULL) y `revoked_at` (`TEXT` NULL). Ninguna columna SHALL contener el token completo, y el token
SHALL NOT aparecer en ningún log, ni en una salida de `Debug`, ni en el listado de claves.

#### Scenario: Una clave nueva persiste hash y prefijo, nunca el token

- **Given** una clave creada con un token `val_...` conocido
- **When** se lee su fila de `ingest_keys`
- **Then** `key_hash` es el SHA-256 hexadecimal del token y `prefix` son sus 12 primeros caracteres
- **And** ninguna columna contiene el token completo

#### Scenario: El listado no filtra el token

- **Given** una clave recién creada y su token devuelto una vez
- **When** se hace `GET /api/ingest-keys`
- **Then** la respuesta no contiene el token en ningún campo

#### Scenario: La salida de depuración no filtra el token

- **Given** la representación `Debug` de una clave de ingesta
- **When** se formatea
- **Then** no aparece el token completo

### Requirement: El token SHALL tener un formato fijo y SHALL mostrarse una sola vez

El token de una clave de ingesta SHALL ser `val_` seguido de 32 bytes aleatorios codificados en
base64url sin padding. El token SHALL mostrarse una única vez, en la respuesta que crea la clave, y
SHALL NOT poder releerse en ninguna petición posterior; solo SHALL persistirse su SHA-256 y su
prefijo.

#### Scenario: El token tiene el prefijo y la forma esperados

- **Given** una clave creada
- **When** se inspecciona el token devuelto
- **Then** empieza por `val_`
- **And** el resto es base64url sin padding

#### Scenario: El prefijo son los 12 primeros caracteres

- **Given** un token devuelto al crear una clave
- **When** se persiste la clave
- **Then** su `prefix` coincide con los 12 primeros caracteres del token

#### Scenario: El token no se relee después

- **Given** una clave creada y su token conocido
- **When** se hace `GET /api/ingest-keys`
- **Then** la respuesta no incluye el token de esa clave

### Requirement: El middleware de ingesta SHALL autenticar por hash y registrar `last_used_at`

Un middleware aplicado **solo** a `POST /api/notifications` SHALL leer `Authorization: Bearer
<token>` y SHALL responder `401` cuando la cabecera falta, está malformada, o el token es
desconocido o pertenece a una clave **revocada**. Cuando el token es válido, el middleware SHALL
actualizar `last_used_at` de la clave y SHALL dejar pasar la petición. La búsqueda SHALL hacerse por
el hash (columna UNIQUE), sin comparar secretos en claro byte a byte, porque el token no se guarda.
Los registros asociados a la ingesta SHALL incluir solo el `id` y el `name` de la clave, y SHALL NOT
incluir el token.

#### Scenario: Un token válido actualiza `last_used_at` y pasa

- **Given** una clave activa con `last_used_at` NULL
- **When** se hace `POST /api/notifications` con su token
- **Then** la petición llega al handler y responde `201`
- **And** `last_used_at` de la clave deja de ser NULL

#### Scenario: Un token desconocido o malformado se rechaza

- **Given** una cabecera `Authorization` ausente, sin el esquema `Bearer`, o con un token que no
  corresponde a ninguna clave
- **When** se hace `POST /api/notifications`
- **Then** la respuesta es `401`

#### Scenario: Un token revocado se rechaza

- **Given** una clave cuyo `revoked_at` está fijado
- **When** se hace `POST /api/notifications` con su token
- **Then** la respuesta es `401`
- **And** `last_used_at` no se actualiza

#### Scenario: Los registros de ingesta no contienen el token

- **Given** una ingesta autenticada con una clave
- **When** se emiten sus registros
- **Then** incluyen el `id` y el `name` de la clave
- **And** no incluyen el token

### Requirement: `POST /api/ingest-keys` SHALL crear una clave y devolver su token una vez

`POST /api/ingest-keys` SHALL requerir sesión OIDC y SHALL aceptar el cuerpo `{"name": "..."}`,
donde `name` es obligatorio, no vacío tras `trim` y `≤ 64` bytes. Un nombre duplicado SHALL
responder `409`. En caso de éxito SHALL responder `201` con `{"id": ..., "name": ..., "prefix": ...,
"token": "<token completo, única vez>"}`.

#### Scenario: Crear una clave devuelve su token una vez

- **Given** un nombre válido y no usado
- **When** se hace `POST /api/ingest-keys` con `{"name": "<nombre>"}`
- **Then** la respuesta es `201`
- **And** el cuerpo incluye `id`, `name`, `prefix` y `token`

#### Scenario: Un nombre en blanco se rechaza

- **Given** un cuerpo sin `name`, o con `name` que solo contiene espacios
- **When** se hace `POST /api/ingest-keys`
- **Then** la respuesta es `400`

#### Scenario: Un nombre duplicado se rechaza

- **Given** una clave ya existente llamada `monitor`
- **When** se crea otra clave con `name = "monitor"`
- **Then** la respuesta es `409`

#### Scenario: Un nombre demasiado largo se rechaza

- **Given** un `name` de más de 64 bytes
- **When** se hace `POST /api/ingest-keys`
- **Then** la respuesta es `400`

### Requirement: `GET /api/ingest-keys` SHALL listar las claves sin exponer ningún token

`GET /api/ingest-keys` SHALL requerir sesión OIDC y SHALL responder `200` con la lista de claves,
incluidas las revocadas, exponiendo de cada una `id`, `name`, `prefix`, `created_at`, `last_used_at`
y `revoked_at`, y SHALL NOT incluir ningún token.

#### Scenario: Sin claves la lista es vacía

- **Given** la tabla `ingest_keys` vacía
- **When** se hace `GET /api/ingest-keys`
- **Then** la respuesta es `200` con una lista vacía

#### Scenario: La lista incluye las claves y sus metadatos

- **Given** una clave activa y una revocada
- **When** se hace `GET /api/ingest-keys`
- **Then** la respuesta es `200` con una entrada por cada clave
- **And** cada entrada expone `id`, `name`, `prefix`, `created_at`, `last_used_at` y `revoked_at`
- **And** ninguna entrada incluye un token

### Requirement: `DELETE /api/ingest-keys/{id}` SHALL revocar la clave de forma idempotente

`DELETE /api/ingest-keys/{id}` SHALL requerir sesión OIDC y SHALL **revocar** la clave fijando
`revoked_at`, sin borrar la fila. La ruta SHALL responder `204` siempre, exista o no el id, y SHALL
ser idempotente: repetirla SHALL NOT cambiar `revoked_at`. Presentar después el token de una clave
revocada a la ingesta SHALL responder `401`.

#### Scenario: Revocar una clave conserva la fila

- **Given** una clave activa con `revoked_at` NULL
- **When** se hace `DELETE /api/ingest-keys/{id}`
- **Then** la respuesta es `204`
- **And** la fila sigue existiendo con `revoked_at` distinto de NULL

#### Scenario: Revocar dos veces no altera `revoked_at`

- **Given** una clave ya revocada con un `revoked_at` conocido
- **When** se repite `DELETE /api/ingest-keys/{id}`
- **Then** la respuesta es `204`
- **And** `revoked_at` no cambia

#### Scenario: Revocar un id inexistente también es 204

- **Given** un id que no corresponde a ninguna clave
- **When** se hace `DELETE /api/ingest-keys/{id}`
- **Then** la respuesta es `204`

#### Scenario: Una clave revocada ya no ingesta

- **Given** una clave revocada con `DELETE /api/ingest-keys/{id}`
- **When** se hace `POST /api/notifications` con su token
- **Then** la respuesta es `401`
