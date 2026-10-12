## MODIFIED Requirements

### Requirement: El modelo del consolidador SHALL ser configurable por variable de entorno y registrar stats

`Config::from_env()` SHALL leer `SEMANTIC_MODEL` y, si no está definida, SHALL usar el valor
de `MEMORY_MODEL`. La llamada de consolidación SHALL usar ese modelo en `ChatRequest.model`.
La llamada SHALL registrar stats con `profile_id` NULL y `kind='consolidator'`, como las demás llamadas de worker.
La fila SHALL reflejar el resultado de la **validación posterior**, no solo el del transporte: cuando
la respuesta llegue pero su contenido no valide (vacío, sin objeto JSON o con un esquema inválido),
la fila SHALL quedar con `status = 'error'` y el motivo SHALL quedar en `error_message`; solo una
respuesta que valide SHALL quedar con `status = 'success'`.

**Given** las variables de entorno  
**When** se construye la configuración  
**Then** `semantic_model` SHALL tomar `SEMANTIC_MODEL` si existe  
**And** SHALL caer a `MEMORY_MODEL` si no existe

#### Scenario: Modelo por defecto cae a MEMORY_MODEL
**Given** `MEMORY_MODEL = "mistralai/mistral-small-24b-instruct-2501"` y sin `SEMANTIC_MODEL`  
**When** se construye la configuración  
**Then** `semantic_model` es `"mistralai/mistral-small-24b-instruct-2501"`

#### Scenario: Modelo personalizado del consolidador
**Given** `SEMANTIC_MODEL = "google/gemini-2.0-flash-lite"`  
**When** se construye la configuración  
**Then** `semantic_model` es `"google/gemini-2.0-flash-lite"`

#### Scenario: Stats del consolidador con profile NULL
**Given** una consolidación que llama al LLM  
**When** se registra la petición  
**Then** `llm_requests.profile_id` es `NULL`  
**And** `llm_requests.kind` es `'consolidator'`

#### Scenario: Una respuesta que no valida se registra como error
**Given** una consolidación cuyo contenido validó el transporte pero no la validación posterior (vacío, sin objeto JSON o con esquema inválido)  
**When** se registra la petición  
**Then** `llm_requests.status` es `'error'`  
**And** `llm_requests.error_message` lleva el motivo del fallo de validación

#### Scenario: Una respuesta válida se registra como éxito
**Given** una consolidación cuyo contenido es un objeto JSON que pasa la validación del esquema  
**When** se registra la petición  
**Then** `llm_requests.status` es `'success'`

### Requirement: El prompt sembrado del extractor SHALL definir qué es un hecho y SHALL pedir fecha y lugar

El valor sembrado de `timeline_prompt` SHALL definir un hecho como **un acontecimiento del pasado,
completo y comprensible sin la conversación**, y SHALL decir explícitamente que las fases de un mismo
acontecimiento son **un solo hecho** y no varios: salir a correr, correr diez kilómetros y terminar el
entrenamiento son un único hecho. SHALL excluir expresamente las órdenes o peticiones dirigidas al
asistente, los estados transitorios, las opiniones, las preguntas y los planes futuros. SHALL pedir el
`location` cuando el mensaje lo diga, SHALL pedir el `timestamp` del hecho en ISO 8601 cuando se pueda
deducir del mensaje, y SHALL ordenar no repetir ningún hecho ya registrado —ni con las mismas palabras ni
con otras—. Para el lote que no contenga ningún hecho, SHALL pedir el envoltorio `{"events": []}` —la
forma exacta del contrato de salida— y SHALL NOT pedir una lista suelta `[]`. El respaldo mínimo del
código SHALL ser coherente con ese contrato.

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

#### Scenario: El prompt sembrado pide el envoltorio vacío para el lote sin hechos
- **Given** el valor sembrado de `timeline_prompt`
- **When** se inspecciona
- **Then** pide el objeto `{"events": []}` para el lote que no contiene ningún hecho
- **And** NO pide una lista suelta `[]`

### Requirement: La extracción de hechos cronológicos SHALL ser best-effort y SHALL NOT abortar la pasada

Un extractor que devuelva contenido vacío, truncado o sin JSON válido SHALL NOT abortar la pasada: la
ficha episódica, el estado persistente y la marca de `is_indexed` SHALL escribirse igual y SHALL
registrarse el diagnóstico del contenido. Antes de darse por vencida, la extracción SHALL reintentarse
al menos una vez cuando el contenido no sea un objeto JSON interpretable o no contenga la clave
`events`; los intentos intermedios SHALL registrarse como cualquier otra llamada, y solo tras agotar
los reintentos SHALL la pasada continuar sin hechos y registrar el fallo con `kind = 'timeline'`. Un
contenido que sea un **array JSON vacío suelto** (`[]`, y no el envoltorio `{"events": []}`) SHALL
tratarse como **fallo de forma**: SHALL NOT interpretarse como cero hechos y SHALL NOT registrarse con
`status = 'success'`; SHALL agotar los reintentos como cualquier otro fallo de forma, y solo tras
agotarlos SHALL la pasada continuar sin hechos y registrar el fallo con `kind = 'timeline'`. El caso
de cero hechos **legítimo** SHALL ser el **envoltorio** `{"events": []}`, y ese SHALL quedar con
`status = 'success'`. La
validación de cada hecho SHALL descartar sin fallar lo inservible: una `category` fuera del conjunto
cerrado SHALL normalizarse a `lifestyle` sin perder el hecho; un `fact` vacío o en blanco SHALL
descartar ese hecho; un `fact` de más de 280 caracteres SHALL descartarse por no ser atómico; un
`source` ausente o fuera del lote SHALL descartar ese hecho —un hecho que no se puede fechar ni
atribuir no se persiste—; y un `source` que señale una línea cuyo rol no sea `user` SHALL descartarse:
los mensajes del asistente son contexto, nunca origen de un hecho. Un `location` vacío o en blanco
SHALL quedar a `NULL`, y uno de más de 120 caracteres SHALL descartarse. Los hechos repetidos dentro de
un mismo lote SHALL deduplicarse, y un hecho que ya exista en la tabla para ese mismo día SHALL
ignorarse sin fallar: la inserción SHALL ser `INSERT OR IGNORE`.

#### Scenario: Un array suelto es un fallo de forma, no cero hechos

- **Given** un lote que cumple las condiciones de procesamiento y un extractor que responde un `[]` suelto **en todos los intentos**
- **When** el worker procesa la pasada
- **Then** no se inserta ninguna fila en `timeline_events`
- **And** la estadística de la llamada queda con `kind = 'timeline'` y `status = 'error'`
- **And** la ficha episódica, el estado persistente y la marca `is_indexed` se escriben igual
- **And** la pasada no falla

#### Scenario: Un array vacío es cero hechos, no un error

- **Given** un lote que cumple las condiciones de procesamiento y un extractor que responde un array vacío **dentro del envoltorio** (`{"events": []}`)
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

## ADDED Requirements

### Requirement: La petición del consolidador SHALL incluir un turno de usuario además del de sistema

La llamada de consolidación SHALL enviar `ChatRequest.messages` con, al menos, un mensaje de rol
`user` **además** del mensaje de rol `system` que porta el prompt. El turno de usuario SHALL ser no
vacío. La forma de un único mensaje `system` SHALL NOT usarse para la consolidación: el modelo
configurado devuelve contenido vacío con `finish_reason = "stop"` ante esa forma, y un contenido
vacío no es un error de transporte, de modo que la avería pasa inadvertida. El mensaje de sistema
SHALL seguir llevando el prompt configurable por el usuario.

#### Scenario: La petición de consolidación lleva turno de usuario
- **Given** una consolidación en curso
- **When** el worker construye el `ChatRequest`
- **Then** `messages` contiene un mensaje de rol `user` no vacío
- **And** contiene el mensaje de rol `system` con el prompt de consolidación

#### Scenario: El prompt configurable no cambia de rol
- **Given** un `consolidator_prompt` personalizado en `settings`
- **When** el worker construye la petición de consolidación
- **Then** el mensaje de rol `system` es ese valor
- **And** el turno de usuario es independiente del prompt
