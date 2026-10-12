## MODIFIED Requirements

### Requirement: La extracción de hechos cronológicos SHALL ser best-effort y SHALL NOT abortar la pasada

Un extractor que devuelva contenido vacío, truncado o sin JSON válido SHALL NOT abortar la pasada: la
ficha episódica, el estado persistente y la marca de `is_indexed` SHALL escribirse igual y SHALL
registrarse el diagnóstico del contenido. Antes de darse por vencida, la extracción SHALL reintentarse
al menos una vez cuando el contenido no sea un objeto JSON interpretable o no contenga la clave
`events`; los intentos intermedios SHALL registrarse como cualquier otra llamada, y solo tras agotar
los reintentos SHALL la pasada continuar sin hechos y registrar el fallo con `kind = 'timeline'`. Un
contenido que sea un **array JSON vacío** SHALL interpretarse como **cero hechos** y NO SHALL
registrarse como error: la estadística SHALL quedar con `status = 'success'` y `kind = 'timeline'`. La
validación de cada hecho SHALL descartar sin fallar lo inservible: una `category` fuera del conjunto
cerrado SHALL normalizarse a `lifestyle` sin perder el hecho; un `fact` vacío o en blanco SHALL
descartar ese hecho; un `fact` de más de 280 caracteres SHALL descartarse por no ser atómico; un
`source` ausente o fuera del lote SHALL descartar ese hecho —un hecho que no se puede fechar ni
atribuir no se persiste—; y un `source` que señale una línea cuyo rol no sea `user` SHALL descartarse:
los mensajes del asistente son contexto, nunca origen de un hecho. Un `location` vacío o en blanco
SHALL quedar a `NULL`, y uno de más de 120 caracteres SHALL descartarse. Los hechos repetidos dentro de
un mismo lote SHALL deduplicarse, y un hecho que ya exista en la tabla para ese mismo día SHALL
ignorarse sin fallar: la inserción SHALL ser `INSERT OR IGNORE`.

#### Scenario: Un array vacío es cero hechos, no un error

- **Given** un lote que cumple las condiciones de procesamiento y un extractor que responde `[]`
- **When** el worker procesa la pasada
- **Then** no se inserta ninguna fila en `timeline_events`
- **And** la estadística de la llamada queda con `kind = 'timeline'` y `status = 'success'`
- **And** la pasada no falla

#### Scenario: Un fallo de forma se reintenta antes de rendirse

- **Given** un extractor que no devuelve el envoltorio `{"events": [...]}` en el primer intento y sí lo devuelve, con hechos válidos, en el siguiente
- **When** el worker procesa la pasada
- **Then** los hechos del intento que sí traía el envoltorio se persisten
- **And** la pasada no falla

#### Scenario: Un extractor inservible no pierde el lote

- **Given** un lote que cumple las condiciones de procesamiento y un extractor que responde vacío o sin JSON válido **en todos los intentos**
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
