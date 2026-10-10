## MODIFIED Requirements

### Requirement: El worker SHALL escribir la Capa B y la Capa C en la misma pasada y con una sola marca

Al procesar un lote de mensajes sin indexar, el `EpisodicMemoryWorker` SHALL: (1) leer el lote
una sola vez; (2) obtener la ficha episódica (Capa B) y el estado persistente consolidado
(Capa C) **antes** de escribir nada; y (3) escribir la ficha, escribir el estado y marcar los
mensajes como indexados en una **única transacción**. `messages.is_indexed = 1` SHALL implicar
que la ficha (Capa B) se ha escrito y que existe un estado de Capa C válido —el recién
consolidado o, si el tamaño obligó a conservarlo **o si la consolidación falló**, el anterior—.

**Given** un lote de mensajes sin indexar que cumple las condiciones de procesamiento  
**When** el worker lo procesa  
**Then** SHALL obtener la ficha episódica y el estado persistente consolidado del mismo lote  
**And** SHALL escribir ambos y marcar los mensajes en una única transacción  
**And** tras el commit, `is_indexed` SHALL ser `1` para los mensajes del lote  

#### Scenario: Un lote produce ficha y estado, y marca en una transacción
**Given** un lote de mensajes sin indexar que cumple las condiciones
**When** el worker lo procesa y ambas extracciones tienen éxito
**Then** se persiste una ficha episódica
**And** se persiste el estado persistente
**And** los mensajes del lote pasan a `is_indexed = 1` en la misma transacción

#### Scenario: Sin mensajes sin indexar no hay escritura ni marca
**Given** una base de datos sin mensajes con `is_indexed = 0`
**When** el worker evalúa
**Then** NO SHALL escribir ninguna ficha ni estado
**And** NO SHALL llamar al LLM

#### Scenario: El rechazo por techo conserva el estado previo y marca el lote
**Given** un lote cuya Capa C válida supera el techo absoluto
**When** el worker procesa
**Then** se persiste la ficha episódica
**And** NO se sobrescribe el estado persistente (se conserva el anterior)
**And** los mensajes del lote pasan a `is_indexed = 1`

#### Scenario: Un fallo de la consolidación escribe la ficha y marca el lote
**Given** un lote cuya extracción episódica tiene éxito y cuya consolidación falla
**When** el worker procesa
**Then** se persiste la ficha episódica
**And** NO se sobrescribe el estado persistente (se conserva el anterior)
**And** los mensajes del lote pasan a `is_indexed = 1` en la misma transacción

### Requirement: La consolidación y la compresión SHALL forzar modo JSON

La llamada de consolidación y la pasada de compresión SHALL enviar
`response_format: { "type": "json_object" }`, con independencia de cualquier ajuste. El
`max_tokens` SHALL proceder de `GENERATION_SEMANTIC_MAX_TOKENS` (default `2048`) y el razonamiento
SHALL solicitarse como `Off` por defecto. El worker SHALL NOT asumir que el proveedor respeta ese
`Off`: la holgura del contenido sobre el presupuesto SHALL verificarse sobre la respuesta
recibida, no sobre la petición (ver el requisito de ampliación del presupuesto). El modo JSON
SHALL NOT ser configurable.

**Given** cualquier valor de los ajustes de generación  
**When** se construye la petición de consolidación  
**Then** `response_format` SHALL ser `Some(ResponseFormat::JsonObject)`  
**And** NO existe ninguna clave de `settings` que lo desactive  

**Given** los defaults de `settings`  
**When** se construye la petición de consolidación  
**Then** `max_tokens` SHALL ser `Some(2048)`  
**And** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`  

#### Scenario: El consolidador pide JSON
**Given** una consolidación
**When** se construye la petición
**Then** el body del proveedor contiene `"response_format": { "type": "json_object" }`

#### Scenario: La compresión pide JSON
**Given** una pasada de compresión
**When** se construye la petición
**Then** el body del proveedor contiene `"response_format": { "type": "json_object" }`

#### Scenario: El modo JSON no se puede desactivar desde settings
**Given** cualquier combinación de claves `GENERATION_*`
**When** se construye la petición de consolidación
**Then** `response_format` SHALL seguir siendo `JsonObject`

#### Scenario: Holgura de tokens del consolidador
**Given** los defaults de `settings`
**When** se construye la petición de consolidación
**Then** `max_tokens` es `Some(2048)`

#### Scenario: El consolidador no razona por defecto
**Given** `GENERATION_SEMANTIC_REASONING = off`
**When** el consolidador construye la petición
**Then** `reasoning` es `Some(ReasoningSpec::Off)`
**And** el worker NO asume que el proveedor vaya a respetarlo: la holgura del contenido se verifica sobre la respuesta recibida

### Requirement: Un fallo del consolidador SHALL registrar el diagnóstico del contenido

Cuando la consolidación inicial no produce un objeto JSON válido tras los reintentos, el error
SHALL incluir la longitud del contenido (`content_len`) y un preview corto, para permitir el
diagnóstico sin depender del log del proveedor. El worker SHALL registrar además una fila en
`llm_requests` con `kind = 'consolidator'` y `status = 'error'`, cuyo `error_message` incluya
`content_len`, el preview del contenido y el motivo de finalización de la respuesta.

#### Scenario: error con diagnóstico
**Given** una consolidación cuyo contenido no es un objeto JSON
**When** la consolidación falla
**Then** el mensaje de error incluye `content_len`
**And** incluye un preview del contenido

#### Scenario: El fallo queda en las estadísticas
**Given** una consolidación que falla tras el reintento
**When** el worker la descarta
**Then** `llm_requests` gana una fila con `kind = 'consolidator'` y `status = 'error'`
**And** su `error_message` incluye `content_len`, el preview y el motivo de finalización

## REMOVED Requirements

### Requirement: Un fallo en cualquiera de las dos extracciones SHALL NOT dejar escritura parcial ni marca

**Reason**: el requisito deja de ser cierto para la Capa C, porque un fallo de la consolidación ya no aborta la pasada. Lo sustituyen «Un fallo de la extracción episódica SHALL NOT dejar escritura parcial ni marca» y «Un fallo de la consolidación SHALL degradar la pasada sin abortarla».

### Requirement: El worker SHALL reintentar una vez la consolidación antes de abortar la pasada

**Reason**: el reintento deja de ser idéntico y deja de abortar la pasada. Lo sustituyen «El reintento de la consolidación SHALL ampliar el presupuesto cuando la respuesta se trunque» y «Un fallo de la consolidación SHALL degradar la pasada sin abortarla».

## ADDED Requirements

### Requirement: Un fallo de la extracción episódica SHALL NOT dejar escritura parcial ni marca

Si falla la llamada episódica (Capa B), la validación de la ficha o la generación de embeddings, el
worker SHALL NOT abrir transacción, SHALL NOT marcar los mensajes y SHALL iniciar el cooldown. Al
reintentar, SHALL reprocesar el lote completo sin duplicar fichas.

#### Scenario: Fallo episódico no escribe ni marca
**Given** un lote y una llamada episódica que falla
**When** el worker procesa
**Then** `memory` no gana filas
**And** `messages.is_indexed` sigue en `0` para el lote
**And** el cooldown queda activo

#### Scenario: Fallo episódico seguido de éxito no duplica
**Given** un lote cuya llamada episódica falló y que se reintenta con éxito
**When** el worker procesa
**Then** se persiste exactamente una ficha

### Requirement: Un fallo de la consolidación SHALL degradar la pasada sin abortarla

Si la consolidación falla (error del LLM, contenido vacío, contenido que no es un objeto JSON,
estado que no pasa la validación del esquema, o error al leer el estado anterior), el worker SHALL
NOT abortar la pasada: SHALL persistir la ficha episódica, conservar el estado de Capa C anterior y
marcar el lote como indexado, todo en la misma transacción; y SHALL registrar el fallo como fila
`status = 'error'` en `llm_requests`. La pasada SHALL registrarse como aviso, no como un error que
detenga la memoria. El estado anterior SHALL seguir siendo un estado de Capa C válido a todos los
efectos.

#### Scenario: La ficha sobrevive al fallo del consolidador
**Given** un lote, una extracción episódica correcta y una consolidación que falla
**When** el worker procesa
**Then** se persiste la ficha episódica
**And** NO se sobrescribe el estado persistente
**And** los mensajes del lote pasan a `is_indexed = 1`
**And** existe una fila de error en `llm_requests`

#### Scenario: La pasada siguiente vuelve a la normalidad
**Given** una pasada degradada y un lote nuevo
**When** el worker procesa el lote nuevo con una consolidación correcta
**Then** el estado persistente se actualiza con normalidad

### Requirement: El reintento de la consolidación SHALL ampliar el presupuesto cuando la respuesta se trunque

Si el primer intento de consolidación falla y la causa es un corte por presupuesto (motivo de
finalización `length`), el worker SHALL repetir la llamada **una vez** con `max_tokens` al **doble**
de `GENERATION_SEMANTIC_MAX_TOKENS`. Si la causa es cualquier otra (contenido vacío sin truncar,
JSON inválido o esquema inválido), el reintento SHALL usar los parámetros configurados. Un segundo
fallo SHALL NOT abortar la pasada: se degrada según «Un fallo de la consolidación SHALL degradar la
pasada sin abortarla». La compresión de tamaño SHALL NOT reintentarse.

#### Scenario: Truncamiento provoca reintento con más presupuesto
**Given** una consolidación cuyo primer intento se trunca por presupuesto
**When** el worker reintenta
**Then** la petición lleva `max_tokens` al doble de `GENERATION_SEMANTIC_MAX_TOKENS`
**And** si el reintento devuelve un estado válido, la consolidación tiene éxito

#### Scenario: Contenido vacío no trunco mantiene los parámetros
**Given** una consolidación cuyo primer intento devuelve contenido vacío sin truncar
**When** el worker reintenta
**Then** el reintento usa los parámetros configurados

#### Scenario: Dos fallos degradan en vez de abortar
**Given** una consolidación cuyos dos intentos fallan
**When** el worker gestiona el fallo
**Then** NO se aborta la pasada
**And** se aplica la degradación (ficha sí, estado anterior)

### Requirement: El colapso SHALL NOT escribir un resumen vacío o degenerado

Si la respuesta del colapso viene vacía o solo con espacios, o la llamada falla, el `CollapseWorker`
SHALL NOT actualizar `messages.collapsed_content`: SHALL registrar el fallo como fila
`status = 'error'` con `kind = 'collapse'` en `llm_requests` y SHALL dejar el mensaje sin colapsar,
de modo que un intento posterior pueda procesarlo. Un resumen vacío NUNCA SHALL escribirse como
resumen.

#### Scenario: Un resumen vacío no se escribe
**Given** un mensaje largo y una respuesta del LLM vacía o solo con espacios
**When** el `CollapseWorker` procesa el mensaje
**Then** `collapsed_content` NO se actualiza
**And** `llm_requests` gana una fila `status = 'error'` con `kind = 'collapse'`

#### Scenario: Un resumen válido se escribe como hasta ahora
**Given** un mensaje largo y una respuesta válida
**When** el `CollapseWorker` procesa el mensaje
**Then** `collapsed_content` y `collapsed_tokens_count` se actualizan con la respuesta
