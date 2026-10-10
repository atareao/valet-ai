## ADDED Requirements

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
