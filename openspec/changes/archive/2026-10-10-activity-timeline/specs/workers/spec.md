## ADDED Requirements

### Requirement: El `EpisodicMemoryWorker` SHALL extraer hechos cronológicos en la misma pasada (Capa D)

Al procesar un lote de mensajes sin indexar, el worker SHALL obtener, además de la ficha episódica
(Capa B) y del estado persistente (Capa C), los **hechos cronológicos** del lote (Capa D), y SHALL
persistirlos en la tabla `timeline_events` **dentro de la misma transacción** que la ficha, el estado
y la marca. La extracción SHALL recibir el lote principal **numerado y con la fecha de cada mensaje**
(la línea de cada mensaje SHALL empezar por su número y por su `created_at`), y el modelo SHALL
devolver, por cada hecho, el número de la línea de la que procede (`source`), su `category` y su
`fact`. El `timestamp` y el `source_message_id` de cada hecho SHALL derivarse de ese mensaje de origen
—su `created_at` y su `id`— y SHALL NOT tomarse de la hora del sistema ni de un valor propuesto por el
modelo: el extractor SHALL NOT pedirle fechas al modelo. La llamada SHALL usar el modo JSON forzado,
SHALL tomar su prompt de `settings.timeline_prompt` —con un respaldo mínimo si la clave falta, está
vacía o no se puede leer— y SHALL registrar su estadística con `kind = 'timeline'`.

#### Scenario: Un lote produce hechos con la fecha de sus mensajes
- **Given** un lote de mensajes sin indexar, cada uno con su `created_at`
- **When** el worker lo procesa y el extractor devuelve hechos
- **Then** cada hecho se persiste en `timeline_events` con el `timestamp` del mensaje que su `source` señala
- **And** cada hecho apunta a ese mensaje en `source_message_id`
- **And** la ficha, el estado y la marca se escriben en la misma transacción

#### Scenario: Sin hechos no hay filas nuevas
- **Given** un lote cuyo extractor devuelve `{"events": []}`
- **When** el worker lo procesa
- **Then** NO se inserta ninguna fila en `timeline_events`
- **And** la ficha, el estado y la marca se escriben igual

#### Scenario: El prompt del extractor sale de settings
- **Given** una clave `timeline_prompt` sembrada en `settings`
- **When** el worker construye la llamada de la Capa D
- **Then** el prompt de sistema es el valor de esa clave
- **And** si la clave falta, está vacía o no se puede leer, se usa el respaldo mínimo y se registra el aviso

### Requirement: La extracción de hechos cronológicos SHALL ser best-effort y SHALL NOT abortar la pasada

Un extractor que devuelva contenido vacío, truncado o sin JSON válido SHALL NOT abortar la pasada: la
ficha episódica, el estado persistente y la marca de `is_indexed` SHALL escribirse igual, SHALL
registrarse el diagnóstico del contenido y la estadística de la llamada SHALL quedar con
`status = 'error'` y `kind = 'timeline'`. La validación de cada hecho SHALL descartar sin fallar lo
inservible: una `category` fuera del conjunto cerrado SHALL normalizarse a `lifestyle` sin perder el
hecho, un `fact` vacío o en blanco SHALL descartar ese hecho, y un `source` ausente o fuera del lote
SHALL descartar ese hecho —un hecho que no se puede fechar ni atribuir no se persiste—. Los hechos
repetidos dentro de un mismo lote SHALL deduplicarse.

#### Scenario: Un extractor inservible no pierde el lote
- **Given** un lote que cumple las condiciones de procesamiento y un extractor que responde vacío o sin JSON válido
- **When** el worker procesa la pasada
- **Then** la ficha episódica se escribe
- **And** el estado persistente se escribe
- **And** los mensajes del lote pasan a `is_indexed = 1`
- **And** la estadística de la llamada queda con `kind = 'timeline'` y `status = 'error'`

#### Scenario: Una categoría desconocida no descarta el hecho
- **Given** un hecho cuya `category` está fuera del conjunto cerrado
- **When** se valida el hecho
- **Then** se persiste con la categoría `lifestyle`
- **And** el hecho no se pierde

#### Scenario: Un fact vacío descarta solo ese hecho
- **Given** un extractor que devuelve dos hechos, uno de ellos con `fact` en blanco
- **When** se validan
- **Then** solo se persiste el hecho con texto
- **And** la pasada no falla

#### Scenario: Un source fuera del lote descarta solo ese hecho
- **Given** un extractor que devuelve un hecho cuyo `source` no es un número del lote
- **When** se valida el hecho
- **Then** ese hecho se descarta
- **And** el resto del lote se persiste con normalidad

#### Scenario: Los hechos repetidos se deduplican
- **Given** un lote cuyo extractor devuelve dos veces el mismo hecho
- **When** se persisten
- **Then** queda una sola fila en `timeline_events`

### Requirement: El modelo del extractor SHALL ser configurable y SHALL usar los parámetros de su propio rol

El extractor SHALL leer su modelo de la variable de entorno `TIMELINE_MODEL` y SHALL caer a
`MEMORY_MODEL` cuando esa variable no esté definida. Sus parámetros de generación —temperatura,
razonamiento y `max_tokens`— SHALL leerse del rol `GENERATION_TIMELINE_*`, y una clave ausente SHALL
caer al default de ese rol.

#### Scenario: Modelo por defecto cae a MEMORY_MODEL
- **Given** un entorno sin `TIMELINE_MODEL` y con `MEMORY_MODEL` definido
- **When** se resuelve el modelo del extractor
- **Then** es el valor de `MEMORY_MODEL`

#### Scenario: Modelo personalizado del extractor
- **Given** un entorno con `TIMELINE_MODEL` definido
- **When** se resuelve el modelo del extractor
- **Then** es ese valor

#### Scenario: Una clave ausente cae al default del rol
- **Given** un `settings` sin ninguna clave `GENERATION_TIMELINE_*`
- **When** se leen los parámetros de generación del extractor
- **Then** se usan los defaults del rol `GENERATION_TIMELINE_*`
