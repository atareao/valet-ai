# notifications Specification

## Purpose
La bandeja de notificaciones de Valet: un ingest externo, autenticado por clave de aplicación
(capability `ingest-keys`), crea notificaciones; la interfaz con sesión las consulta sin leer y las
marca como leídas; y el agente las gestiona con una herramienta propia. La ingesta se persiste de
forma sincrónica —el `INSERT` precede a la respuesta— y el despacho por WebPush es best-effort, de
modo que un fallo de envío no altera la respuesta de la API.

## Requirements

### Requirement: La tabla `notifications` SHALL almacenar la notificación y su estado de lectura

La tabla `notifications` SHALL tener las columnas `id` (`TEXT` PK, UUIDv4), `origin` (`TEXT` NOT
NULL), `author` (`TEXT` NOT NULL), `message` (`TEXT` NOT NULL), `level` (`TEXT` NOT NULL DEFAULT
`'info'`), `metadata` (`TEXT` NULL, objeto JSON serializado), `read_at` (`TEXT` NULL), y
`created_at` (`TEXT` NOT NULL DEFAULT `(datetime('now'))`). Una notificación recién insertada SHALL
tener `read_at` NULL. La tabla SHALL declarar un índice parcial `idx_notifications_unread ON
notifications(created_at) WHERE read_at IS NULL` —desviación consciente respecto de indexarlo sobre
`read_at`, porque la consulta de no leídas filtra por `read_at IS NULL` y ordena por `created_at
DESC`, de modo que un índice sobre `read_at` no serviría para ordenar— y un índice
`idx_notifications_origin ON notifications(origin, created_at)`.

#### Scenario: Una notificación nace sin leer

- **Given** una notificación insertada con `origin`, `author` y `message`
- **When** se lee la fila por su `id`
- **Then** `read_at` es NULL
- **And** `created_at` no es nulo y `level` es `'info'`

#### Scenario: El índice de no leídas es parcial y ordena por antigüedad

- **Given** el esquema migrado
- **When** se consulta la definición de `idx_notifications_unread` en `sqlite_master`
- **Then** su definición filtra por `read_at IS NULL` (índice parcial) e indexa `created_at`
- **And** la consulta de no leídas ordenada por `created_at DESC` puede apoyarse en él sin recorrer la tabla completa

#### Scenario: `metadata` admite ausencia y objeto

- **Given** una notificación insertada sin `metadata`
- **When** se lee la fila
- **Then** `metadata` es NULL
- **And** una notificación insertada con un objeto JSON tiene ese objeto serializado en `metadata`

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

### Requirement: El cuerpo de la ingesta SHALL validarse y normalizarse antes de persistir

`message` SHALL ser obligatorio: tras `trim` no vacío y de longitud `≤ 4096` bytes. `origin` y
`author` SHALL ser opcionales; si faltan o quedan en blanco tras `trim` SHALL rellenarse con el
**nombre de la clave** que autentica la petición, y si se aportan SHALL medir `≤ 64` bytes (`origin`)
y `≤ 128` bytes (`author`). `level` SHALL ser opcional con lista blanca `info` (por defecto),
`warning` y `critical`; cualquier otro valor SHALL responder `400`. `metadata` SHALL ser opcional y,
si está presente, SHALL ser un objeto JSON cuya serialización mida `≤ 8192` bytes. El cuerpo HTTP
SHALL estar limitado a 64 KiB por la ruta, y una petición que lo exceda SHALL responder `413`. Toda
validación fallida SHALL responder `400` con un cuerpo `{"error": "..."}` (a través de `AppError`).

#### Scenario: Un mensaje ausente o en blanco se rechaza

- **Given** un cuerpo sin `message`, o con `message` que solo contiene espacios
- **When** se hace `POST /api/notifications` con token válido
- **Then** la respuesta es `400`
- **And** el cuerpo es `{"error": "..."}`

#### Scenario: `origin` y `author` ausentes toman el nombre de la clave

- **Given** una clave de ingesta llamada `monitor` y un cuerpo con solo `message`
- **When** se ingesta con esa clave
- **Then** la fila persistida tiene `origin = "monitor"` y `author = "monitor"`

#### Scenario: Se recorta el mensaje antes de persistir

- **Given** un cuerpo cuyo `message` tiene espacios al principio y al final
- **When** se ingesta
- **Then** el `message` persistido no incluye esos espacios

#### Scenario: Un `level` fuera de la lista blanca se rechaza

- **Given** un cuerpo con `level = "debug"`
- **When** se ingesta
- **Then** la respuesta es `400`
- **And** un cuerpo con `level = "critical"` responde `201` y persiste `level = "critical"`

#### Scenario: `metadata` debe ser objeto

- **Given** un cuerpo con `metadata` igual a un arreglo JSON
- **When** se ingesta
- **Then** la respuesta es `400`
- **And** un cuerpo con `metadata` igual a un objeto JSON responde `201`

#### Scenario: Se rechazan los cuerpos que exceden los límites

- **Given** un `message` de más de 4096 bytes
- **When** se ingesta
- **Then** la respuesta es `400`
- **And** un cuerpo HTTP de más de 64 KiB responde `413`

### Requirement: La persistencia SHALL preceder a la respuesta y el despacho WebPush SHALL ser best-effort

El `INSERT` de la notificación SHALL ocurrir **antes** de responder; el despacho WebPush SHALL
realizarse **después**, en segundo plano, y un fallo del despacho SHALL NOT cambiar la respuesta ya
emitida. El payload despachado SHALL usar el título `[<origin>] <author>`, el mensaje como cuerpo,
`tag` `notification-<id>`, icono `/icon-192.png` y `url` `/`.

#### Scenario: El fallo del push no altera la respuesta

- **Given** una notificación válida y un servicio de push inalcanzable
- **When** se hace `POST /api/notifications`
- **Then** la respuesta es `201`
- **And** la fila existe en `notifications`

#### Scenario: El payload despachado compone el título con origen y autor

- **Given** una notificación con `origin = "monitor"` y `author = "backup"`
- **When** se despacha el WebPush
- **Then** el título es `[monitor] backup`

#### Scenario: El payload despachado fija tag, icono y url

- **Given** una notificación de `id` conocido
- **When** se despacha el WebPush
- **Then** `tag` es `notification-<id>`, el icono es `/icon-192.png` y `url` es `/`

### Requirement: `GET /api/notifications/unread` SHALL listar las no leídas con sesión

`GET /api/notifications/unread` SHALL requerir sesión OIDC y SHALL devolver solo las notificaciones
con `read_at IS NULL`, **más recientes primero**, con **paginación por cursor (keyset)**, respetando
`limit` (por defecto 50, máximo 200) y un `cursor` opaco. La respuesta SHALL ser la paginación del
proyecto (`PaginatedResponse`: `data`, `next_cursor`, `total`), ya NO con `next_cursor` siempre
`None`. El orden SHALL ser `created_at DESC, rowid DESC`. El `cursor` SHALL codificar **`created_at` y
`rowid` juntos** en la forma `"<created_at>|<rowid>"`, y la consulta SHALL paginar con
`created_at < ?a OR (created_at = ?a AND rowid < ?b)`, de modo que NO se salten los avisos con el
mismo `created_at` que el último de la página (pues `created_at` es `datetime('now')` y tiene
granularidad de segundo). `next_cursor` SHALL ser `null` cuando no haya más filas. Cada item SHALL
exponer `id`, `origin`, `author`, `message`, `level`, `metadata` (objeto JSON ya parseado, o `null`
si la columna es NULL) y `created_at`. `total` SHALL ser el recuento de no leídas, independiente de la
página. Un `limit` no numérico o fuera del rango `[1, 200]` SHALL responder `400`. Un `cursor`
malformado SHALL responder `400`. Sin sesión SHALL responder `401`.

#### Scenario: Solo se listan las no leídas, más recientes primero

- **Given** una notificación leída y dos no leídas con `created_at` distintos
- **When** se hace `GET /api/notifications/unread`
- **Then** la respuesta es `200` y `data` contiene solo las dos no leídas
- **And** la de `created_at` mayor aparece primero

#### Scenario: `metadata` se parsea a objeto o queda nulo

- **Given** una notificación con `metadata` JSON y otra con `metadata` NULL
- **When** se hace `GET /api/notifications/unread`
- **Then** el item de la primera expone `metadata` como objeto
- **And** el item de la segunda expone `metadata` como `null`

#### Scenario: Un `limit` inválido se rechaza

- **Given** `limit=0`, `limit=201` o `limit=abc`
- **When** se hace `GET /api/notifications/unread`
- **Then** la respuesta es `400`

#### Scenario: Sin sesión no se lista

- **Given** una petición a `GET /api/notifications/unread` sin sesión
- **When** se procesa
- **Then** la respuesta es `401`

#### Scenario: La primera página respeta `limit` y expone el total

- **Given** 25 notificaciones no leídas
- **When** se hace `GET /api/notifications/unread?limit=10`
- **Then** la respuesta es `200` con 10 items en `data`
- **And** `next_cursor` no es `null`
- **And** `total` es 25

#### Scenario: El cursor no salta los avisos empatados en el mismo segundo

- **Given** tres notificaciones no leídas con el MISMO `created_at`
- **When** se pide `GET /api/notifications/unread?limit=2` y después la siguiente página siguiendo el `next_cursor`
- **Then** la primera página trae 2 items y la segunda trae 1
- **And** entre las dos aparecen las tres, sin repetidos

#### Scenario: En la última página no hay cursor

- **Given** una página que agota las no leídas disponibles
- **When** se hace `GET /api/notifications/unread`
- **Then** `next_cursor` es `null`

#### Scenario: Un `cursor` malformado se rechaza

- **Given** `cursor=basura`
- **When** se hace `GET /api/notifications/unread`
- **Then** la respuesta es `400`

### Requirement: El agente SHALL disponer de una herramienta `notifications`

El agente SHALL exponer una herramienta `notifications` con las operaciones `list_unread` y
`mark_read`, con permiso `NoConfirm`, y SHALL figurar en `definitions()`. `list_unread` SHALL
devolver las mismas notificaciones no leídas que el endpoint homónimo, y `mark_read` SHALL compartir
la semántica idempotente de la ruta homónima, **incluida la marca masiva**. En concreto, `mark_read`
SHALL marcar **todas** las no leídas cuando `ids` se omite; con `ids` no vacío marca esos; y un
`ids: []` explícito SHALL ser `InvalidArguments`.

#### Scenario: La herramienta figura en las definiciones

- **Given** el registro de herramientas construido
- **When** se llama a `definitions()`
- **Then** incluye `notifications` con las operaciones `list_unread` y `mark_read`

#### Scenario: `list_unread` coincide con el endpoint

- **Given** dos notificaciones no leídas
- **When** el agente invoca `notifications.list_unread`
- **Then** devuelve esas dos notificaciones
- **And** no devuelve las leídas

#### Scenario: `mark_read` es idempotente también para el agente

- **Given** dos notificaciones no leídas
- **When** el agente invoca `notifications.mark_read` con sus ids y luego lo repite
- **Then** la primera invocación marca 2
- **And** la repetición marca 0

#### Scenario: El permiso es `NoConfirm`

- **Given** la herramienta `notifications` en el registro
- **When** se consulta su permiso
- **Then** es `NoConfirm`

#### Scenario: `mark_read` sin `ids` marca todas las no leídas

- **Given** tres notificaciones no leídas
- **When** el agente invoca `notifications.mark_read` sin `ids`
- **Then** marca 3 y las tres quedan leídas

#### Scenario: La herramienta y la ruta comparten la semántica de «todas»

- **Given** un mismo estado con notificaciones no leídas
- **When** se marca todo desde la herramienta y desde la ruta homónima
- **Then** ambas devuelven el mismo `marked` para ese estado

### Requirement: `POST /api/notifications/mark-read` SHALL marcar como leídas de forma idempotente y masiva

`POST /api/notifications/mark-read` SHALL requerir sesión OIDC. Con un cuerpo `{"ids": ["<uuid>", ...]}`,
la ruta SHALL fijar `read_at` **solo** en las notificaciones que estaban sin leer y SHALL responder
`{"marked": <n>}` con el número de filas efectivamente marcadas. Con el campo `ids` **ausente** (cuerpo
`{}`), la ruta SHALL marcar **todas** las notificaciones no leídas y SHALL responder `{"marked": <n>}`
con ese recuento. Un `ids: []` (array vacío explícito) SHALL responder `400`, porque una lista vacía es
una petición malformada mientras que la ausencia expresa la intención de marcar todas. La operación
SHALL ser idempotente: repetirla con los mismos ids SHALL responder `marked: 0`. Un id inexistente SHALL
ignorarse sin error.

#### Scenario: Marcar dos notificaciones no leídas

- **Given** dos notificaciones no leídas con `id` conocidos
- **When** se hace `POST /api/notifications/mark-read` con esos ids
- **Then** la respuesta es `{"marked": 2}`
- **And** ambas filas tienen `read_at` distinto de NULL

#### Scenario: Repetir la marca es idempotente

- **Given** las dos notificaciones ya marcadas
- **When** se repite `POST /api/notifications/mark-read` con los mismos ids
- **Then** la respuesta es `{"marked": 0}`
- **And** los `read_at` no cambian

#### Scenario: Un id inexistente se ignora

- **Given** un id que no corresponde a ninguna notificación
- **When** se hace `POST /api/notifications/mark-read` con ese id
- **Then** la respuesta es `{"marked": 0}`

#### Scenario: Un `ids` explícitamente vacío se rechaza

- **Given** un cuerpo con `ids: []`
- **When** se hace `POST /api/notifications/mark-read`
- **Then** la respuesta es `400`

#### Scenario: Omitir `ids` marca todas las no leídas

- **Given** tres notificaciones no leídas
- **When** se hace `POST /api/notifications/mark-read` con el cuerpo `{}`
- **Then** la respuesta es `{"marked": 3}`
- **And** las tres filas tienen `read_at` distinto de NULL

#### Scenario: Marcar todas es idempotente

- **Given** las tres notificaciones ya marcadas
- **When** se repite `POST /api/notifications/mark-read` con el cuerpo `{}`
- **Then** la respuesta es `{"marked": 0}`

### Requirement: NotificationsRepo SHALL exponer la marca masiva y el listado paginado por cursor

`NotificationsRepo` SHALL exponer `mark_all_read(pool) -> u64`, que fija `read_at =
datetime('now')` en todas las filas con `read_at IS NULL` y devuelve el número de filas afectadas,
siendo idempotente (una segunda llamada devuelve `0`). Asimismo SHALL exponer `list_unread(pool,
limit, cursor) -> (items, next_cursor, total)`, con orden `created_at DESC, rowid DESC`, paginación
keyset por `(created_at, rowid)`, `total` igual al recuento de no leídas —independiente de la
página— y `next_cursor` presente solo si hay más filas.

#### Scenario: La marca masiva es idempotente

- **Given** varias notificaciones con `read_at IS NULL`
- **When** se invoca `mark_all_read` y luego se repite
- **Then** la primera llamada devuelve el número de filas marcadas
- **And** la segunda devuelve `0`

#### Scenario: El cursor keyset no salta los empatados del mismo segundo

- **Given** notificaciones no leídas compartiendo el mismo `created_at`
- **When** se pagina con `list_unread` siguiendo su `next_cursor`
- **Then** los empatados aparecen repartidos entre páginas sin repetirse ni perderse

#### Scenario: `total` es independiente del tamaño de página

- **Given** más no leídas que el `limit` solicitado
- **When** se invoca `list_unread` con ese `limit`
- **Then** `total` sigue siendo el recuento completo de no leídas

#### Scenario: Sin más filas el cursor es `None`

- **Given** una consulta que agota las no leídas
- **When** se invoca `list_unread`
- **Then** `next_cursor` es `None`
