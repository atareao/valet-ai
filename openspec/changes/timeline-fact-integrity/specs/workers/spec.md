## MODIFIED Requirements

### Requirement: El `EpisodicMemoryWorker` SHALL extraer hechos cronológicos en la misma pasada (Capa D)

Al procesar un lote de mensajes sin indexar, el worker SHALL obtener, además de la ficha episódica
(Capa B) y del estado persistente (Capa C), los **hechos cronológicos** del lote (Capa D), y SHALL
persistirlos en la tabla `timeline_events` **dentro de la misma transacción** que la ficha, el estado y
la marca. La extracción SHALL recibir el lote principal **numerado y con la fecha y el rol de cada
mensaje** (la línea de cada mensaje SHALL empezar por su número, por su `created_at` y por su rol), el
**instante de referencia** del lote y la lista de **hechos ya registrados** de los últimos días. El
modelo SHALL devolver, por cada hecho, el número de la línea de la que procede (`source`), su `category`
y su `fact`, y opcionalmente su `timestamp` y su `location`. La llamada SHALL usar el modo JSON forzado,
SHALL tomar su prompt de `settings.timeline_prompt` —con un respaldo mínimo si la clave falta, está vacía
o no se puede leer— y SHALL registrar su estadística con `kind = 'timeline'`.

**La fecha SHALL ser la del hecho, no la de la mención.** Cuando el hecho traiga un `timestamp` válido,
SHALL ser ese su `timestamp`; si falta, no parsea, no lleva desfase horario, está en el futuro más allá
de la tolerancia admitida o es anterior a la ventana admitida, SHALL caer al `created_at` del mensaje
citado. El `source_message_id` SHALL ser siempre el `id` de ese mensaje.

#### Scenario: Un lote produce hechos con la fecha de sus mensajes
- **Given** un lote de mensajes sin indexar, cada uno con su `created_at`
- **When** el worker lo procesa y el extractor devuelve hechos sin `timestamp`
- **Then** cada hecho se persiste en `timeline_events` con el `timestamp` del mensaje que su `source` señala
- **And** cada hecho apunta a ese mensaje en `source_message_id`
- **And** la ficha, el estado y la marca se escriben en la misma transacción

#### Scenario: Una fecha propuesta y válida es la fecha del hecho
- **Given** un hecho cuyo mensaje citado es de las 15:25 y que trae un `timestamp` válido de las 10:21 del mismo día
- **When** se valida
- **Then** el hecho se persiste con las 10:21

#### Scenario: Sin hechos no hay filas nuevas
- **Given** un lote cuyo extractor devuelve `{"events": []}`
- **When** el worker lo procesa
- **Then** NO se inserta ninguna fila en `timeline_events`
- **And** la ficha, el estado y la marca se escriben igual

#### Scenario: El bloque lleva el rol, el instante de referencia y los hechos conocidos
- **Given** un lote de mensajes sin indexar y un timeline con hechos de los últimos días
- **When** el worker construye el bloque de la Capa D
- **Then** cada línea empieza por su número, su `created_at` y su rol
- **And** el bloque incluye el instante de referencia del lote
- **And** el bloque incluye los hechos ya registrados de los últimos días

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
hecho; un `fact` vacío o en blanco SHALL descartar ese hecho; un `fact` de más de 280 caracteres SHALL
descartarse por no ser atómico; un `source` ausente o fuera del lote SHALL descartar ese hecho —un hecho
que no se puede fechar ni atribuir no se persiste—; y un `source` que señale una línea cuyo rol no sea
`user` SHALL descartarse: los mensajes del asistente son contexto, nunca origen de un hecho. Un
`location` vacío o en blanco SHALL quedar a `NULL`, y uno de más de 120 caracteres SHALL descartarse. Los
hechos repetidos dentro de un mismo lote SHALL deduplicarse, y un hecho que ya exista en la tabla para
ese mismo día SHALL ignorarse sin fallar: la inserción SHALL ser `INSERT OR IGNORE`.

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

#### Scenario: Un fact desmedido descarta solo ese hecho
- **Given** un extractor que devuelve un hecho cuyo `fact` pasa de 280 caracteres
- **When** se valida
- **Then** ese hecho se descarta
- **And** el resto del lote se persiste con normalidad

#### Scenario: Un source fuera del lote descarta solo ese hecho
- **Given** un extractor que devuelve un hecho cuyo `source` no es un número del lote
- **When** se valida el hecho
- **Then** ese hecho se descarta
- **And** el resto del lote se persiste con normalidad

#### Scenario: Un hecho del asistente no se persiste
- **Given** un extractor que devuelve un hecho cuyo `source` señala una línea de rol `assistant`
- **When** se valida
- **Then** ese hecho se descarta
- **And** el resto del lote se persiste con normalidad

#### Scenario: Una fecha inservible cae al mensaje citado
- **Given** un hecho cuyo `timestamp` no parsea, no lleva desfase horario, está en el futuro o es anterior a la ventana admitida
- **When** se valida
- **Then** el hecho se persiste con el `created_at` del mensaje citado
- **And** el hecho no se pierde

#### Scenario: Un lugar inservible queda a nulo
- **Given** un hecho con `location` vacío o de más de 120 caracteres
- **When** se valida
- **Then** el hecho se persiste con `location` a `NULL`
- **And** el hecho no se pierde

#### Scenario: Los hechos repetidos se deduplican
- **Given** un lote cuyo extractor devuelve dos veces el mismo hecho
- **When** se persisten
- **Then** queda una sola fila en `timeline_events`

#### Scenario: Un hecho que ya está en la tabla no se duplica
- **Given** un hecho ya registrado en `timeline_events` para un día
- **When** una pasada posterior vuelve a extraer ese mismo hecho con ese mismo día
- **Then** no se inserta una segunda fila
- **And** la pasada termina sin error

## ADDED Requirements

### Requirement: El prompt sembrado del extractor SHALL definir qué es un hecho y SHALL pedir fecha y lugar

El valor sembrado de `timeline_prompt` SHALL definir un hecho como **un acontecimiento del pasado,
completo y comprensible sin la conversación**, y SHALL decir explícitamente que las fases de un mismo
acontecimiento son **un solo hecho** y no varios: salir a correr, correr diez kilómetros y terminar el
entrenamiento son un único hecho. SHALL excluir expresamente las órdenes o peticiones dirigidas al
asistente, los estados transitorios, las opiniones, las preguntas y los planes futuros. SHALL pedir el
`location` cuando el mensaje lo diga, SHALL pedir el `timestamp` del hecho en ISO 8601 cuando se pueda
deducir del mensaje, y SHALL ordenar no repetir ningún hecho ya registrado —ni con las mismas palabras ni
con otras—. El respaldo mínimo del código SHALL ser coherente con ese contrato.

#### Scenario: El prompt sembrado excluye lo que no es un hecho
- **Given** el valor sembrado de `timeline_prompt`
- **When** se inspecciona
- **Then** nombra las órdenes dirigidas al asistente y los estados transitorios entre lo que no debe extraerse

#### Scenario: El prompt sembrado une las fases de un acontecimiento
- **Given** el valor sembrado de `timeline_prompt`
- **When** se inspecciona
- **Then** dice que las fases de un mismo acontecimiento son un solo hecho
- **And** pone el ejemplo de salir a correr, correr y terminar el entrenamiento

#### Scenario: El prompt sembrado pide fecha y lugar
- **Given** el valor sembrado de `timeline_prompt`
- **When** se inspecciona
- **Then** pide el `timestamp` del hecho en ISO 8601
- **And** pide el `location` cuando el mensaje lo diga

#### Scenario: El prompt sembrado prohíbe repetir lo ya registrado
- **Given** el valor sembrado de `timeline_prompt`
- **When** se inspecciona
- **Then** ordena no repetir los hechos ya registrados que se le listan
- **And** no solo con las mismas palabras, también con otras
