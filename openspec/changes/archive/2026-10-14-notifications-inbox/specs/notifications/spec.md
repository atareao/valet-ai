## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: `POST /api/notifications/mark-read` SHALL marcar como leídas de forma idempotente

## ADDED Requirements

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
