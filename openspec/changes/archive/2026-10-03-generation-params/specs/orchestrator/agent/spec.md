# Spec Delta: orchestrator/agent

## ADDED Requirements

### Requirement: El chat SHALL usar los parámetros de generación de settings en cada turno

El orquestador SHALL leer `GENERATION_CHAT_TEMPERATURE` (default `0.7`),
`GENERATION_CHAT_REASONING` (default vacío) y `GENERATION_CHAT_MAX_TOKENS` (default `4096`) de
`settings` **en cada turno** y aplicarlos al `ChatRequest` del chat, en lugar de dejar la
temperatura sin definir y los tokens fijos en el código. El razonamiento vacío SHALL significar
`None` (no se envía el campo y decide el modelo); `off`/`none` ⇒ `ReasoningSpec::Off`; un nivel ⇒
`ReasoningSpec::Effort`. Esto SHALL aplicarse a la construcción del `ChatRequest` que usa
`process_message_stream()`, el camino del chat que llama a `chat_stream()`. Si una clave falta o
no es parseable, SHALL caer al default con un warning.

**Given** los defaults de `settings`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `temperature` SHALL ser `Some(0.7)`  
**And** `reasoning` SHALL ser `None`  
**And** `max_tokens` SHALL ser `Some(4096)`

**Given** `GENERATION_CHAT_REASONING = "high"`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Effort(High))`

**Given** `GENERATION_CHAT_REASONING = "off"`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`

#### Scenario: El chat envía una temperatura explícita
**Given** `GENERATION_CHAT_TEMPERATURE = 0.7`  
**When** el orquestador construye la petición del chat  
**Then** el body del proveedor contiene `"temperature": 0.7`

#### Scenario: El chat no envía razonamiento con el default vacío
**Given** `GENERATION_CHAT_REASONING = ""`  
**When** el orquestador construye la petición del chat  
**Then** `reasoning` es `None`  
**And** el body del proveedor NO contiene la clave `reasoning`

#### Scenario: El chat puede pedir un nivel de razonamiento
**Given** `GENERATION_CHAT_REASONING = "high"`  
**When** el orquestador construye la petición del chat  
**Then** `reasoning` es `Some(ReasoningSpec::Effort(High))`

#### Scenario: Los tokens máximos del chat vienen de settings
**Given** `GENERATION_CHAT_MAX_TOKENS = 8000`  
**When** el orquestador construye la petición del chat  
**Then** `max_tokens` es `Some(8000)`

#### Scenario: El camino de streaming usa los parámetros de settings
**Given** `GENERATION_CHAT_TEMPERATURE = 0.5` y `GENERATION_CHAT_REASONING = "medium"`  
**When** se ejecuta `process_message_stream()`  
**Then** el `ChatRequest` de `chat_stream()` lleva `temperature = Some(0.5)` y el mismo `reasoning`

#### Scenario: Una temperatura no parseable cae al default con warning
**Given** `GENERATION_CHAT_TEMPERATURE = "caliente"`  
**When** el orquestador lee sus parámetros  
**Then** `temperature` es `Some(0.7)`  
**And** se registra un warning
