# Spec Delta: workers

## ADDED Requirements

### Requirement: Los workers SHALL usar los parámetros de generación de settings en cada llamada

El `CollapseWorker`, la extracción de fichas y la consolidación/compresión SHALL leer su
temperatura, su razonamiento y sus tokens máximos de las claves `GENERATION_*` de `settings`
**en cada llamada** (no al arrancar) y usarlos en el `ChatRequest`, en lugar de valores
hardcodeados. Si una clave falta o no es parseable, SHALL caer al default del rol con un warning.

**Given** las claves `GENERATION_COLLAPSE_*` en `settings`  
**When** el `CollapseWorker` construye su `ChatRequest`  
**Then** `temperature`, `reasoning` y `max_tokens` SHALL proceder de esas claves

**Given** las claves `GENERATION_MEMORY_*` en `settings`  
**When** el worker episódico extrae una ficha  
**Then** `temperature`, `reasoning` y `max_tokens` SHALL proceder de esas claves

**Given** las claves `GENERATION_SEMANTIC_*` en `settings`  
**When** el consolidador construye su `ChatRequest`  
**Then** `temperature`, `reasoning` y `max_tokens` SHALL proceder de esas claves

#### Scenario: El colapso toma sus tres parámetros de settings
**Given** `GENERATION_COLLAPSE_TEMPERATURE = 0.5`, `GENERATION_COLLAPSE_REASONING = off` y `GENERATION_COLLAPSE_MAX_TOKENS = 512`  
**When** el `CollapseWorker` construye la petición  
**Then** `temperature` es `Some(0.5)`  
**And** `reasoning` es `Some(ReasoningSpec::Off)`  
**And** `max_tokens` es `Some(512)`

#### Scenario: Las fichas toman sus tres parámetros de settings
**Given** `GENERATION_MEMORY_TEMPERATURE = 0.3` y `GENERATION_MEMORY_MAX_TOKENS = 1024`  
**When** el worker episódico construye la petición  
**Then** `temperature` es `Some(0.3)`  
**And** `max_tokens` es `Some(1024)`

#### Scenario: Una clave ausente cae al default del rol
**Given** `settings` sin `GENERATION_COLLAPSE_MAX_TOKENS`  
**When** el `CollapseWorker` construye la petición  
**Then** `max_tokens` es `Some(1024)`

### Requirement: El colapso y las fichas SHALL NOT razonar por defecto

Con los valores iniciales, el `CollapseWorker` y la extracción de fichas episódicas SHALL enviar
`reasoning: Off`, de modo que no se paguen tokens de razonamiento en tareas mecánicas.

**Given** los defaults de `settings`  
**When** el `CollapseWorker` construye su `ChatRequest`  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`

**Given** los defaults de `settings`  
**When** el worker episódico extrae una ficha  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`

#### Scenario: El colapso no razona con los defaults
**Given** `GENERATION_COLLAPSE_REASONING = off`  
**When** el `CollapseWorker` construye la petición  
**Then** el body del proveedor contiene `"reasoning": { "enabled": false }`

#### Scenario: Las fichas no razonan con los defaults
**Given** `GENERATION_MEMORY_REASONING = off`  
**When** el worker episódico construye la petición  
**Then** el body del proveedor contiene `"reasoning": { "enabled": false }`

### Requirement: La consolidación y la compresión SHALL forzar modo JSON

La llamada de consolidación y la pasada de compresión SHALL enviar
`response_format: { "type": "json_object" }`, con independencia de cualquier ajuste. El
`max_tokens` SHALL proceder de `GENERATION_SEMANTIC_MAX_TOKENS` (default `2048`) para dejar holgura
frente a los tokens de razonamiento. El modo JSON SHALL NOT ser configurable.

**Given** cualquier valor de los ajustes de generación  
**When** se construye la petición de consolidación  
**Then** `response_format` SHALL ser `Some(ResponseFormat::JsonObject)`  
**And** NO existe ninguna clave de `settings` que lo desactive

**Given** los defaults de `settings`  
**When** se construye la petición de consolidación  
**Then** `max_tokens` SHALL ser `Some(2048)`  
**And** `reasoning` SHALL ser `Some(ReasoningSpec::Effort(Low))`

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
