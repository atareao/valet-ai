# notifications Specification

## MODIFIED Requirements

### Requirement: El cuerpo de la ingesta SHALL validarse y normalizarse antes de persistir

`message` SHALL ser opcional cuando exista `title`: el mensaje SHALL derivarse de `message` y, si
este falta o queda en blanco tras `trim`, de `title`; el mensaje resultante SHALL ser no vacío tras
`trim` y medir `≤ 4096` bytes, y si ni `message` ni `title` aportan texto la respuesta SHALL ser
`400`. `origin` y `author` SHALL ser opcionales; si faltan o quedan en blanco tras `trim` SHALL
rellenarse con el **nombre de la clave** que autentica la petición, y si se aportan SHALL medir `≤ 64`
bytes (`origin`) y `≤ 128` bytes (`author`). `level` SHALL ser opcional: si viene SHALL pertenecer a
la lista blanca `info` (por defecto), `warning` y `critical`, y cualquier otro valor SHALL responder
`400`; si falta, el nivel SHALL tomarse de `severity` normalizado a minúsculas (`critical`, `warning`,
`info`), cayendo a `info` ante cualquier otro valor. `metadata` SHALL ser opcional y, si está
presente, SHALL ser un objeto JSON cuya serialización mida `≤ 8192` bytes; `body_markdown`, si viene
no vacío tras `trim`, SHALL anexarse a `metadata` bajo la clave `body_markdown` sin pisar una clave ya
presente, contando para ese límite. El cuerpo HTTP SHALL estar limitado a 64 KiB por la ruta, y una
petición que lo exceda SHALL responder `413`. Toda validación fallida SHALL responder `400` con un
cuerpo `{"error": "..."}` (a través de `AppError`).

#### Scenario: Un mensaje ausente o en blanco se rechaza

- **Given** un cuerpo cuyo `message` falta o queda en blanco tras `trim`
- **When** tampoco se aporta `title` (o también queda en blanco) y se hace `POST /api/notifications` con token válido
- **Then** la respuesta es `400`
- **And** el cuerpo es `{"error": "..."}`

#### Scenario: Un mensaje ausente se deriva del `title`

- **Given** un cuerpo sin `message` pero con `title` no vacío
- **When** se hace `POST /api/notifications` con token válido
- **Then** la respuesta es `201`
- **And** el `message` persistido es el `title` recortado

#### Scenario: Un mensaje explícito manda sobre el `title`

- **Given** un cuerpo con `message` y `title` distintos
- **When** se ingesta
- **Then** el `message` persistido es el de `message`

#### Scenario: `severity` determina el nivel cuando falta `level`

- **Given** un cuerpo sin `level` y con `severity = "critical"`
- **When** se ingesta
- **Then** la respuesta es `201` y el `level` persistido es `critical`
- **And** un `severity` desconocido (p. ej. `"emergency"`) responde `201` con `level = "info"`

#### Scenario: Un `level` explícito manda sobre `severity`

- **Given** un cuerpo con `level = "warning"` y `severity = "critical"`
- **When** se ingesta
- **Then** el `level` persistido es `warning`

#### Scenario: `body_markdown` se anexa a `metadata`

- **Given** un cuerpo con `body_markdown` no vacío
- **When** se ingesta
- **Then** la `metadata` persistida incluye la clave `body_markdown` con ese valor
- **And** una `metadata` que ya traía `body_markdown` no se pisa

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
