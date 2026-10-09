# Spec Delta: workers

## MODIFIED Requirements

### Requirement: La consolidación y la compresión SHALL forzar modo JSON

La llamada de consolidación y la pasada de compresión SHALL enviar
`response_format: { "type": "json_object" }`, con independencia de cualquier ajuste. El
`max_tokens` SHALL proceder de `GENERATION_SEMANTIC_MAX_TOKENS` (default `2048`) y el razonamiento
SHALL ser `Off` por defecto, de modo que el presupuesto íntegro quede para el JSON. El modo JSON
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
**And** el presupuesto de `max_tokens` queda disponible para el contenido

## ADDED Requirements

### Requirement: El worker SHALL reintentar una vez la consolidación antes de abortar la pasada

Si la consolidación inicial devuelve contenido vacío, un contenido que no sea un objeto JSON, o un
estado que no pase la validación del esquema, el worker SHALL repetir la llamada de consolidación
**una vez** antes de devolver el error y abortar la pasada. El reintento SHALL usar los mismos
parámetros de generación. La compresión de tamaño SHALL NOT reintentarse (ya degrada sin abortar).

#### Scenario: contenido vacío seguido de JSON válido
**Given** una consolidación cuyo primer intento devuelve contenido vacío y cuyo segundo intento devuelve un JSON válido
**When** el worker consolida
**Then** la consolidación tiene éxito
**And** la pasada continúa y persiste ficha y estado

#### Scenario: dos intentos inválidos abortan
**Given** una consolidación cuyos dos intentos devuelven contenido vacío o esquema inválido
**When** el worker consolida
**Then** devuelve error
**And** la pasada se aborta sin escribir ficha ni estado

#### Scenario: el reintento no se aplica a la compresión
**Given** una pasada de compresión que falla
**When** el worker gestiona el tamaño
**Then** NO se reintenta la compresión
**And** se degrada al estado sin comprimir o se conserva el estado anterior

### Requirement: Un fallo del consolidador SHALL registrar el diagnóstico del contenido

Cuando la consolidación inicial no produce un objeto JSON válido tras los reintentos, el error
SHALL incluir la longitud del contenido (`content_len`) y un preview corto, para permitir el
diagnóstico sin depender del log del proveedor.

#### Scenario: error con diagnóstico
**Given** una consolidación cuyo contenido no es un objeto JSON  
**When** la consolidación falla  
**Then** el mensaje de error incluye `content_len`  
**And** incluye un preview del contenido
