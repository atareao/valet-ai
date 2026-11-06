# orchestrator/agent Specification

## Purpose
Bucle ReAct del agente de Valet: uso del system prompt desde la base de datos, tolerancia a errores de herramientas, límite de reintentos, entrega incremental vía chat_stream, prioridad de los tool_calls del evento Done y registro de estadísticas de cada llamada al LLM.

## Requirements

### Requirement: Tool execution errors do not break the ReAct loop

El orquestador SHALL convertir los errores de ejecución de herramientas en un `ToolResult { success: false }` y continuar el bucle ReAct.
**Given** el orquestador ejecuta un tool call
**When** `registry.execute()` retorna `Err(ToolError)` (e.g. timeout, parse error)
**Then** el error NO DEBE propagarse con `?` rompiendo el loop
**And** SE DEBE convertir en `ToolResult { success: false, message: error.to_string() }`
**And** el mensaje de error DEBE pasarse al LLM como tool result para que pueda responder
**And** el ReAct loop DEBE continuar normalmente

#### Scenario: Overpass timeout no rompe el orquestador
**Given** un orquestador con el tool `geo`
**When** el LLM invoca `geo.search_places`
**And** Overpass no responde (timeout de 30s)
**Then** `registry.execute()` retorna `Err(ToolError::ExecutionError("timeout"))`
**And** el orquestador NO propaga el error
**And** envía un `SSEEvent::ToolResult` con `success: false`
**And** pasa el mensaje de error al LLM como tool result
**And** el ReAct loop continúa

#### Scenario: Tool error tiene mensaje descriptivo para el LLM
**Given** un orquestador procesando un tool call
**When** el tool retorna error
**Then** el mensaje enviado al LLM es `format!("Error: {}", error)`
**And** incluye detalles del error (status code, raw body si aplica)

#### Scenario: Herramientas exitosas no se ven afectadas
**Given** un orquestador
**When** un tool call retorna `Ok(ToolResult { success: true, ... })`
**Then** el comportamiento NO cambia
**And** el tool result se pasa al LLM normalmente

### Requirement: Orchestrator uses chat_stream for incremental response delivery

El orquestador SHALL usar `self.llm.chat_stream()` en la iteración final y emitir cada fragmento como `SSEEvent::Chunk`.

**Given** el orquestador procesando `process_message_stream()`  
**When** se alcanza la iteración final del ReAct loop (sin tool calls)  
**Then** usa `self.llm.chat_stream()` para recibir la respuesta incrementalmente  
**And** emite cada chunk como `SSEEvent::Chunk` al frontend vía `tx.send()`  
**And** al finalizar, emite `SSEEvent::Done` con los IDs

#### Scenario: Chunks se reenvían como SSEEvent::Chunk
**Given** el orquestador ha recibido `StreamEvent::Chunk("Hola")` del LLM  
**When** procesa el evento  
**Then** envía `SSEEvent::Chunk { content: "Hola" }` por el canal SSE  
**And** acumula el contenido para el mensaje final a persistir

#### Scenario: Tool call en stream (caso borde)
**Given** el orquestador en la iteración final  
**When** el stream devuelve `StreamEvent::ToolCall`  
**Then** el orquestador cambia a modo tool call  
**And** procesa el tool call normalmente  
**And** continúa el ReAct loop (no emite Done)

#### Scenario: Done con tool_calls presente
**Given** el orquestador recibe `StreamEvent::Done(response)`  
**When** `response.message.tool_calls` es `Some`  
**Then** NO emite los chunks acumulados como SSEEvent::Chunk  
**And** procesa los tool calls en el ReAct loop  
**And** continúa a la siguiente iteración

#### Scenario: Done con respuesta final sin tool calls
**Given** el orquestador recibe `StreamEvent::Done(response)`  
**When** `response.message.tool_calls` es `None`  
**Then** emite los chunks acumulados como `SSEEvent::Chunk`  
**And** persiste el mensaje en DB  
**And** emite `SSEEvent::Done` con los IDs

#### Scenario: Fallback si chat_stream() no está implementado
**Given** un provider sin implementación real de `chat_stream()`  
**When** el orquestador llama a `chat_stream()`  
**Then** recibe un stream con un único `StreamEvent::Done`  
**And** el comportamiento es equivalente al actual

### Requirement: Tool error logging

El orquestador SHALL registrar a nivel `error!` los errores de herramientas con nombre, argumentos y mensaje completo.

All tool execution errors must be logged with maximum detail including
tool name, arguments, and the full error message (Display + Debug).

#### Scenario: ToolError from registry.execute() is logged with error!
When a tool returns `Err(ToolError)`, the orchestrator must log:
- `tool_name` — the tool that failed
- `tool_args` — the JSON arguments passed to the tool
- `error` — the error Display text
- `error_debug` — the error Debug representation

#### Scenario: ToolResult success=false is logged with error!
When a tool returns `ToolResult { success: false, ... }`, the orchestrator
must log the tool name, arguments, and error message.

#### Scenario: Tool retry limit reached is logged with warn!
When one of the three ReAct tool guards (per-turn total, fingerprint, success budget or
consecutive-failure budget) blocks a tool call, the orchestrator must log a warning with the
tool name, the call count and the **guard that tripped**.

### Requirement: Done event tool_calls take precedence over stream events

El orquestador SHALL usar los `tool_calls` del evento Done (con argumentos completos) y caer en los del stream solo cuando el Done no los traiga.

#### Scenario: Done event tool_calls have full arguments
**Given** a Done event with `response.message.tool_calls = Some([...])` containing
full arguments from `StreamAccumulator.finalize()`
**When** the Done handler processes tool_calls
**Then** it uses the tool_calls from the Done response (with full arguments)
**And** not from the stream events (which have `arguments: Value::Null`)

#### Scenario: Done event without tool_calls falls back to stream
**Given** a Done event with `response.message.tool_calls = None`
**And** `tool_calls_from_stream` has tool calls
**When** the Done handler processes
**Then** it falls back to `tool_calls_from_stream`

### Requirement: Aislamiento estructural del bloque <episodic_memory>

El bloque de memoria episódica SHALL ir delimitado por las etiquetas `<episodic_memory>` dentro de la sección `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)`, con la instrucción explícita de que son antecedentes y no parte de la conversación actual, y SHALL NOT usar el prefijo `[Memory context]`. El bloque SHALL ser una sección del único mensaje de sistema, que va **antes** del historial de conversación.

**Given** un orquestador con fichas que superan el umbral  
**When** se construye la petición  
**Then** el mensaje de sistema contiene `# CONTEXTO DE MEMORIA EPISÓDICA (CAPA B)` y las etiquetas `<episodic_memory>`  
**And** contiene la instrucción de que son antecedentes y no parte del turno actual  
**And** NO contiene el prefijo `[Memory context]`  
**And** es una sección del único mensaje de sistema, que aparece antes de los mensajes del historial de conversación

#### Scenario: Etiquetas e instrucción presentes
**Given** fichas inyectables  
**When** se inspecciona la sección de memoria del mensaje de sistema  
**Then** contiene `<episodic_memory>` y su cierre  
**And** contiene la instrucción de no confundir los antecedentes con el turno actual

#### Scenario: El bloque precede al historial
**Given** un historial de conversación cargado por `list_by_token_budget`  
**When** se ordenan los mensajes de la petición  
**Then** el mensaje de sistema que contiene `<episodic_memory>` aparece antes del primer mensaje del historial

#### Scenario: No queda el prefijo antiguo
**Given** fichas inyectables  
**When** se construye el prompt  
**Then** NO aparece el literal `[Memory context]` en ningún mensaje

### Requirement: El formato de ficha SHALL NOT depender de metadata ausente

El formato de ficha SHALL NOT incluir el `[tags]`, porque el `EpisodicMemoryWorker` no escribe la clave `tags` en `metadata`.

**Given** una ficha persistida por el `EpisodicMemoryWorker` con `metadata = {"source","primary_message_ids","date_context","first_message_at","last_message_at"}` y sin `tags`  
**When** se formatea esa ficha para el bloque de memoria  
**Then** el texto SHALL NOT contener el prefijo `[tags]` ni corchetes vacíos (`[]`)

#### Scenario: Ficha del worker sin corchetes vacíos
**Given** una ficha con `metadata` sin la clave `tags`  
**When** se formatea la ficha  
**Then** el resultado NO contiene `[]`  
**And** el resultado NO contiene el prefijo `[tags]`

### Requirement: Orden por decaimiento de cada ficha

El orden de las fichas dentro del bloque SHALL ser el de relevancia final descendente definido en el requisito «El decaimiento temporal SHALL calcularse en Rust y ordenar los resultados» de `specs/orchestrator/spec.md`, de modo que la antigüedad reordena, pero no excluye. La ficha inyectada SHALL NOT precederse de fecha alguna: el bloque SHALL inyectar el contenido de la ficha tal cual.

**Given** una ficha con `created_at` y `metadata` (con o sin `last_message_at`)  
**When** se compone el bloque de memoria episódica  
**Then** el texto inyectado de esa ficha SHALL ser su contenido, sin ninguna fecha delante  
**And** el orden de las fichas en el bloque SHALL seguir el de relevancia final descendente fijado en el requisito del decaimiento de `orchestrator`

#### Scenario: El texto inyectado NO incluye ninguna fecha
**Given** una ficha con `created_at = "2026-09-29T10:00:00Z"` y `metadata` con `last_message_at`  
**When** se compone el bloque  
**Then** el texto de la ficha NO contiene ninguna fecha derivada de `created_at` ni de `metadata`  
**And** el texto de la ficha es su contenido tal cual

#### Scenario: El orden es por relevancia final
**Given** dos fichas con la misma similitud y distinta antigüedad  
**When** se compone el bloque  
**Then** la ficha más reciente aparece antes que la más antigua

### Requirement: System prompt template comes from settings with Markdown and emojis

**Given** una base de datos migrada con el prompt sembrado en `settings.system_prompt`  
**When** el orquestador construye la petición al LLM  
**Then** SHALL leer `system_prompt` de la tabla `settings`  
**And** SHALL usar ese valor como mensaje de sistema  
**And** SHALL NOT usar ningún template hardcodeado en `OrchestratorConfig`  
**And** el prompt SHALL contener instrucciones de Markdown, "Emojis" y formato rico  
**And** si `system_prompt` está ausente o vacío, SHALL usar un fallback mínimo genérico y loguear un warning

#### Scenario: System prompt incluye personaje de mayordomo
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "asistente personal británico"  
**And** contiene "usted"  
**And** contiene "caballero"

#### Scenario: System prompt tiene modo conciso y expandido
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "Modo Conciso (Predeterminado)"  
**And** contiene "Expandido"

#### Scenario: System prompt permite Markdown completo y emojis
**Given** la base de datos migrada  
**When** se lee `settings.system_prompt`  
**Then** contiene "Markdown"  
**And** contiene "Emojis"

#### Scenario: process_message_stream usa el prompt de la BD
**Given** un orquestador con `settings.system_prompt = "Prompt de prueba"`  
**When** se llama `process_message_stream()`  
**Then** el mensaje de sistema enviado al LLM es "Prompt de prueba"

#### Scenario: Fallback mínimo si falta el prompt
**Given** un orquestador cuya tabla `settings` no tiene `system_prompt`  
**When** se construye la petición al LLM  
**Then** se usa un fallback mínimo genérico no vacío  
**And** se loguea un warning indicando que falta el prompt

### Requirement: Orchestrator records stats after each streamed LLM call and propagates errors

The Orchestrator SHALL call `StatsRepo::record_request()` after each LLM call in `process_message_stream()`. When the call fails, it SHALL record a row with status `"error"` **and** SHALL propagate the failure to the caller: recording SHALL NOT swallow, replace or mask the error.

#### Scenario: process_message_stream records stats
- **WHEN** `process_message_stream()` receives `StreamEvent::Done`
- **THEN** there is at least one row in `llm_requests` with tokens

#### Scenario: LLM error records stats with error status and propagates
- **WHEN** a mock LLM whose stream always fails is used with `process_message_stream()`
- **THEN** there is a row in `llm_requests` with status = "error" and non-empty error_message
- **AND** `process_message_stream()` returns an error, so the failure is not swallowed

### Requirement: Inyección automática del bloque de memoria episódica en la ruta de streaming

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** SHALL recuperar memorias episódicas por similitud semántica en **cada** mensaje, con independencia de la estrategia de contexto  
**And** SHALL componer el bloque de memoria en código, no desde un placeholder de `settings.system_prompt`  
**And** SHALL incluir el bloque como una sección dentro del único mensaje de sistema, nunca como un mensaje `role:"system"` independiente  
**And** SHALL omitir la sección por completo cuando no haya ninguna ficha que supere el umbral, sin texto de relleno del tipo "no hay antecedentes"

#### Scenario: Se inyecta memoria cuando hay fichas sobre el umbral
**Given** un orquestador con pool, provider y al menos una ficha que supera `SIMILARITY_THRESHOLD`  
**When** se construye la petición  
**Then** el mensaje de sistema contiene la sección de memoria episódica  
**And** la sección incluye el contenido de esa ficha

#### Scenario: El bloque se omite por completo sin fichas
**Given** un orquestador con pool y provider pero sin fichas que superen el umbral  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene la sección de memoria episódica ni las etiquetas `<episodic_memory>`  
**And** NO aparece ningún texto de relleno del tipo "no hay antecedentes"

#### Scenario: El bloque no depende de un placeholder editable
**Given** un `settings.system_prompt` sin ningún placeholder de memoria  
**When** se construye la petición  
**Then** el bloque de memoria se compone igualmente en código  
**And** borrar o editar `system_prompt` NO desactiva la memoria

### Requirement: La petición SHALL abrir con un único mensaje de sistema

La petición al LLM SHALL abrir con **un solo** mensaje `role:"system"`, que reúna en este
orden las secciones presentes: (1) el prompt de `settings.system_prompt`, (2) la sección con el
nombre del usuario, (3) la sección de memoria persistente si la hay, (4) la memoria episódica si
la hay, (5) la sección de ubicación si hay coordenadas, y (6) **la sección temporal, que SHALL
estar siempre presente y cerrar el mensaje**. El mensaje SHALL preceder al historial de
conversación. Las secciones presentes SHALL separarse con una línea en blanco, y ninguna sección
ausente SHALL dejar título, marcador, separador ni línea en blanco. La sección temporal SHALL NOT
considerarse ausente en ningún caso.

**Given** un mensaje del usuario
**When** el orquestador construye la petición al LLM
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`
**And** SHALL incluir después, y solo si existen, las secciones de nombre de usuario, memoria persistente, memoria episódica y ubicación, en ese orden
**And** SHALL terminar con la sección temporal, que nunca falta
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador
**When** se construye la petición
**Then** hay exactamente un mensaje `role:"system"` antes del historial
**And** ese mensaje contiene el prompt, la sección episódica, la ubicación y la sección temporal

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, nombre de usuario, memoria persistente no vacía, fichas inyectables y contexto de navegador con coordenadas
**When** se inspecciona el mensaje de sistema
**Then** el prompt aparece antes que la sección con el nombre del usuario
**And** la sección con el nombre del usuario aparece antes que la sección de memoria persistente
**And** la sección de memoria persistente aparece antes que la sección episódica
**And** la sección episódica aparece antes que la sección de ubicación
**And** la sección de ubicación aparece antes que la sección temporal
**And** la sección temporal es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin nombre de usuario, sin fichas inyectables y sin contexto de navegador
**When** se construye la petición
**Then** el mensaje de sistema contiene el prompt y, a continuación, la sección temporal, sin nada intermedio
**And** no aparece ningún título, marcador ni línea en blanco de relleno

#### Scenario: La sección temporal no depende del navegador
**Given** una invocación sin `BrowserContext`
**When** se construye la petición
**Then** el mensaje de sistema termina igualmente con la sección temporal

### Requirement: La sección de memoria persistente SHALL inyectarse entre el prompt y la episódica

Cuando exista un estado persistente no vacío, el orquestador SHALL inyectar su sección
**entre el prompt y la memoria episódica**, como el `payload` en **JSON minificado** precedido
de un encabezado corto. El estado SHALL leerse en **cada** construcción de la petición.
SHALL omitirse por completo, sin dejar rastro —ni encabezado, ni marcador, ni línea en
blanco—, cuando el estado esté vacío o no exista.

**Given** un estado persistente no vacío  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema SHALL contener la sección de memoria persistente con el JSON minificado  
**And** esa sección SHALL situarse entre el prompt y la sección episódica

**Given** un estado persistente vacío o inexistente  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema NO SHALL contener ninguna sección de memoria persistente  
**And** NO SHALL aparecer encabezado, marcador ni línea en blanco de relleno

#### Scenario: Estado no vacío se inyecta entre el prompt y la episódica
**Given** un orquestador con un estado persistente no vacío, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene el JSON minificado del estado  
**And** la sección persistente está entre el prompt y la sección episódica

#### Scenario: Estado vacío no deja rastro
**Given** un orquestador sin estado persistente y con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** entre el prompt y la sección episódica no hay texto de memoria persistente  
**And** no aparece ningún encabezado ni marcador reservado

### Requirement: El chat SHALL usar los parámetros de generación de settings en cada turno

El orquestador SHALL leer `GENERATION_CHAT_TEMPERATURE` (default `0.7`),
`GENERATION_CHAT_REASONING` (default `off`) y `GENERATION_CHAT_MAX_TOKENS` (default `4096`) de
`settings` **en cada turno** y aplicarlos al `ChatRequest` del chat, en lugar de dejar la
temperatura sin definir y los tokens fijos en el código. El razonamiento vacío SHALL significar
`None` (no se envía el campo y decide el modelo); `off`/`none` ⇒ `ReasoningSpec::Off`; un nivel ⇒
`ReasoningSpec::Effort`. Esto SHALL aplicarse a la construcción del `ChatRequest` que usa
`process_message_stream()`, el camino del chat que llama a `chat_stream()`. Si una clave falta o
no es parseable, SHALL caer al default con un warning.

**Given** los defaults de `settings`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `temperature` SHALL ser `Some(0.7)`  
**And** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`  
**And** `max_tokens` SHALL ser `Some(4096)`

**Given** `GENERATION_CHAT_REASONING = "high"`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Effort(High))`

**Given** `GENERATION_CHAT_REASONING = "off"`  
**When** el orquestador construye el `ChatRequest` del chat  
**Then** `reasoning` SHALL ser `Some(ReasoningSpec::Off)`

#### Scenario: El chat no razona con el default

**Given** los defaults de `settings` (la clave `GENERATION_CHAT_REASONING` ausente o con el valor `off`)  
**When** el orquestador construye la petición del chat  
**Then** `reasoning` es `Some(ReasoningSpec::Off)`  
**And** el body del proveedor contiene `"reasoning": {"enabled": false}`

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

### Requirement: El nombre del usuario SHALL inyectarse en el mensaje de sistema

El orquestador SHALL leer el nombre del usuario desde `profiles.name` —por el `profile_id` recibido—
en **cada** construcción de la petición, y SHALL inyectarlo como una sección compuesta en código
—nunca desde un placeholder de `settings.system_prompt`— situada **entre el prompt y la memoria
persistente**. La sección SHALL usar el título `# USUARIO` y el texto `El nombre del usuario es {name}. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta.`.
SHALL omitirse por completo, sin dejar rastro, cuando el nombre esté vacío tras recortar espacios o
sea el valor por defecto `Valet User`. Si la lectura del perfil falla, SHALL loguear un warning y
omitir la sección sin abortar la petición.

**Given** un mensaje del usuario y un perfil con nombre no vacío y distinto del valor por defecto  
**When** el orquestador construye la petición  
**Then** el mensaje de sistema SHALL contener una sección `# USUARIO` con `El nombre del usuario es {name}.` y la guía de uso del nombre  
**And** esa sección SHALL situarse entre el prompt y la sección de memoria persistente

#### Scenario: El nombre real se inyecta
**Given** un perfil con `name = "Lorenzo"`  
**When** se construye la petición  
**Then** el mensaje de sistema contiene `El nombre del usuario es Lorenzo. Dirígete a él por su nombre cuando sea natural, sin repetirlo en cada respuesta.`  
**And** la sección aparece después del prompt

#### Scenario: Nombre vacío no deja rastro
**Given** un perfil con `name` vacío o solo espacios  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene el título `# USUARIO` ni la línea con el nombre  
**And** no queda ningún separador de relleno

#### Scenario: El nombre por defecto no se inyecta
**Given** un perfil recién creado con `name = "Valet User"`  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene la sección `# USUARIO`

#### Scenario: El nombre no depende de un placeholder editable
**Given** un `settings.system_prompt` sin ningún placeholder de nombre  
**When** se construye la petición  
**Then** la sección del nombre se compone igualmente en código

### Requirement: La petición al LLM SHALL ofrecer únicamente las herramientas habilitadas

Cada `ChatRequest` construida en el bucle ReAct SHALL incluir en `tools` únicamente las definiciones
de las herramientas habilitadas en ese momento.

#### Scenario: Una herramienta deshabilitada queda fuera de la petición

**Given** un orquestador con `unified_search` deshabilitada
**When** construye la petición al LLM
**Then** `request.tools` NO contiene la definición de `unified_search`
**And** sí contiene las definiciones de las herramientas habilitadas

### Requirement: La aprobación explícita SHALL pausar y reanudar el turno sin romper el stream

Ante un `GuardrailResult::RequiresApproval`, el orquestador SHALL emitir `SSEEvent::ApprovalRequired`
y SHALL esperar la decisión como máximo `APPROVAL_TIMEOUT`. SHALL NOT devolver error ni cerrar el
stream mientras espera.

- **Aprobada:** SHALL ejecutar la herramienta y tratarla como cualquier otro tool call.
- **Denegada, expirada o cancelada:** SHALL inyectar un mensaje `role:"tool"` indicando que la llamada
  no se ejecutó y que no debe reintentarse, y SHALL continuar el bucle ReAct para que el LLM responda.
- **En todos los casos:** SHALL emitir `SSEEvent::ApprovalResult { request_id, approved }`, con
  `approved: true` solo si hubo aprobación.
- El mensaje assistant final SHALL persistirse como en un turno normal, y el turno SHALL terminar con
  `SSEEvent::Done`.

**Contract:**

```rust
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);

// Antes: return Err(AgentError::GuardrailError(...)) al recibir RequiresApproval.
// Ahora: emitir ApprovalRequired, await_approval, decidir y continuar el bucle.
```

#### Scenario: Aprobar ejecuta la herramienta y termina el turno

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el usuario aprueba la solicitud
**Then** la herramienta se ejecuta
**And** se emite `ApprovalResult { approved: true }`
**And** el stream continúa y termina con `Done`
**And** el mensaje assistant se persiste

#### Scenario: Denegar no ejecuta y el LLM responde

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el usuario deniega la solicitud
**Then** la herramienta NO se ejecuta
**And** se emite `ApprovalResult { approved: false }`
**And** el LLM recibe un mensaje `role:"tool"` de rechazo
**And** el bucle continúa y el stream termina con `Done`

#### Scenario: El timeout se comporta como denegación

**Given** un orquestador esperando una aprobación
**When** se agota `APPROVAL_TIMEOUT` sin decisión
**Then** la herramienta NO se ejecuta
**And** se emite `ApprovalResult { approved: false }`
**And** el stream continúa y termina con `Done`

#### Scenario: Una aprobación no rompe el stream con error

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el guardrail devuelve `RequiresApproval`
**Then** NO se emite un `SSEEvent::Error` por este motivo
**And** el turno queda a la espera de la decisión

### Requirement: La ejecución exitosa de `render_widget` SHALL emitir un evento SSE `widget`

Cuando el bucle ReAct ejecuta una tool call cuyo nombre es `render_widget` y la ejecución tiene éxito,
el orquestador SHALL emitir un `SSEEvent` de tipo `widget` con un `id` único no vacío, el `name` del
widget y el `data` recibido, antes del evento `done` del turno. El resultado de la herramienta SHALL
seguir inyectándose en el bucle como mensaje `tool`, de forma que el turno continúe.

**Given** un turno de streaming en curso
**When** el modelo invoca `render_widget` con éxito
**Then** el cliente recibe un evento `widget` y el turno termina con `done`

#### Scenario: La tool call produce un evento `widget`

**Given** una tool call a `render_widget` con `widget_name: "QuickForm"` y `data` válido
**When** el orquestador procesa la tool call con éxito
**Then** emite un `SSEEvent` de tipo `widget` con `name: "QuickForm"` y el mismo `data`
**And** el `id` del evento no está vacío

#### Scenario: Un widget inválido no emite evento

**Given** una tool call a `render_widget` con un `widget_name` no permitido
**When** el orquestador procesa la tool call y la ejecución falla
**Then** no se emite ningún evento de tipo `widget`
**And** el turno continúa sin romperse

#### Scenario: El turno sigue tras el widget

**Given** una tool call a `render_widget` ejecutada con éxito
**When** el orquestador prosigue el bucle ReAct
**Then** el resultado de la herramienta se añade como mensaje `tool`
**And** el turno termina con un evento `done`

### Requirement: El orquestador SHALL persistir los widgets renderizados con el mensaje del asistente

Cuando un turno emite uno o varios eventos `widget` (es decir, ejecuta `render_widget` con éxito), el
orquestador SHALL persistir esos widgets —con el **mismo `id`** del evento, su `name` y su `data`— en la
columna `widgets` del mensaje assistant del turno, de forma que el frontend pueda reconstruirlos al
recargar. Un turno sin widgets SHALL dejar `widgets` como `null`.

#### Scenario: Un turno con widget persiste la lista
**Given** un turno en el que el modelo invoca `render_widget` con éxito
**When** el orquestador persiste el mensaje assistant
**Then** ese mensaje SHALL tener `widgets` con el `id` del evento `widget` emitido, su `name` y su `data`

#### Scenario: Un turno sin widgets no persiste nada
**Given** un turno sin ninguna invocación a `render_widget`
**When** el orquestador persiste el mensaje assistant
**Then** `widgets` SHALL ser `null`

### Requirement: El orquestador SHALL enrutar las herramientas por turno antes del bucle ReAct

Con el enrutador activo, la petición al LLM SHALL llevar el subconjunto `core ∪ skills_seleccionadas ∩ habilitadas`, decidido **una sola vez por turno y antes del bucle ReAct**. Con el enrutador apagado o ante cualquier fallo del clasificador, la petición SHALL llevar todas las herramientas habilitadas, de modo que el fallo abierto reproduzca exactamente el comportamiento sin enrutador. Este requisito complementa «La petición al LLM SHALL ofrecer únicamente las herramientas habilitadas», que sigue vigente.

#### Scenario: Con el enrutador activo la petición lleva el subconjunto decidido
- **Given** el enrutador activo y una selección con la skill de agenda
- **When** se construye la petición al LLM
- **Then** las herramientas ofrecidas son el core más las de agenda
- **And** no figuran las de las skills no seleccionadas

#### Scenario: Con el enrutador apagado nada cambia
- **Given** el enrutador apagado
- **When** se construye la petición al LLM
- **Then** se ofrecen todas las herramientas habilitadas

#### Scenario: El fallo del clasificador reproduce el comportamiento sin enrutador
- **Given** el enrutador activo y un clasificador que falla
- **When** se construye la petición al LLM
- **Then** se ofrecen todas las herramientas habilitadas

#### Scenario: La segunda iteración no vuelve a decidir
- **Given** un turno cuya primera iteración produjo una llamada a herramienta
- **When** se construye la petición de la segunda iteración
- **Then** ofrece el mismo subconjunto que la primera
- **And** no se invoca de nuevo al clasificador

### Requirement: El orquestador SHALL persistir la llamada al clasificador marcada como del router

Cuando el enrutado haya invocado al clasificador (fuente `Router` o `Error`), el orquestador SHALL insertar en `llm_requests` una fila con `kind='router'` usando `StatsRepo::record_request`, con el modelo del enrutador, los tokens de entrada y salida, el coste, la latencia y el estado (`'success'` o `'error'`). El orquestador SHALL NOT insertar ninguna fila cuando la fuente sea `Disabled` ni `NoRoutableSkills` (no hubo llamada al clasificador). El orquestador SHALL NOT alterar `last_api_call`.

#### Scenario: Un turno enrutado persiste la llamada del clasificador
- **Given** el enrutador activo y una decisión del clasificador
- **When** el orquestador completa el enrutado
- **Then** existe una fila en `llm_requests` con `kind='router'` y el modelo del enrutador
- **And** lleva el coste y la latencia de la decisión

#### Scenario: El fallo del clasificador se persiste como error
- **Given** el enrutador activo y un clasificador que falla (timeout o error HTTP)
- **When** el orquestador completa el enrutado
- **Then** existe una fila con `kind='router'` y `status='error'`

#### Scenario: Un turno que no llama al clasificador no persiste nada
- **Given** el enrutador apagado o sin skills enrutables
- **When** el orquestador completa el enrutado
- **Then** no se inserta ninguna fila con `kind='router'`

#### Scenario: La última llamada sigue siendo la del chat
- **Given** un turno enrutado
- **When** se persiste la llamada del clasificador
- **Then** `last_api_call` no cambia

### Requirement: El mensaje de sistema SHALL incluir una sección temporal en cada turno

En **cada** construcción de la petición, el mensaje de sistema SHALL cerrar con una sección temporal compuesta en código (nunca desde un placeholder de `settings.system_prompt`) con el instante del turno formateado como `YYYY-MM-DD HH:MM:SS` seguido del día de la semana entre paréntesis, precedido de la etiqueta `Fecha y hora actual:` y seguido de la instrucción de evaluar «hoy», «ayer» y «mañana» respecto a ese timestamp. La sección SHALL indicar además que la marca `[YYYY-MM-DD HH:MM]` de los mensajes es metadato y SHALL NOT reproducirse en las respuestas. La sección SHALL estar presente aunque no haya `BrowserContext`, y un fallo al resolver el instante o la zona SHALL caer a un valor seguro (UTC) sin abortar ni omitir la sección.

**Given** cualquier turno
**When** el orquestador construye la petición
**Then** el mensaje de sistema SHALL terminar con la sección temporal
**And** la sección SHALL contener la etiqueta, el instante formateado con el día de la semana y la instrucción de evaluación relativa
**And** SHALL indicar que la marca de tiempo de los mensajes no debe reproducirse

#### Scenario: Formato de la sección temporal
**Given** un instante del turno y una zona conocidos
**When** se compone la sección temporal
**Then** el texto es `Fecha y hora actual: 2026-10-09 19:00:00 (viernes). Evalúa 'hoy', 'ayer' y 'mañana' respecto a este timestamp. No reproduzcas la marca '[YYYY-MM-DD HH:MM]' de los mensajes en tus respuestas.`

#### Scenario: Presente aunque no haya navegador
**Given** una invocación sin `BrowserContext`
**When** se construye la petición
**Then** la sección temporal sigue presente y cierra el mensaje

### Requirement: La sección de ubicación SHALL llevar solo la ubicación

Cuando haya `BrowserContext` **con coordenadas**, la sección de ubicación SHALL contener únicamente `Ubicación: {nombre} ({lat}, {lon}).` cuando se resuelva el nombre, o `Coordenadas: ({lat}, {lon}).` cuando no, y SHALL NOT contener ninguna fecha ni hora. Cuando no haya coordenadas —haya o no `BrowserContext`— la sección SHALL omitirse por completo, sin dejar rastro.

**Given** una invocación con o sin `BrowserContext`
**When** el orquestador construye la petición
**Then** SHALL añadir la sección de ubicación solo si hay coordenadas
**And** SHALL omitirla por completo si no las hay

#### Scenario: Con coordenadas la sección lleva solo la ubicación
**Given** un `BrowserContext` con coordenadas (y, opcionalmente, nombre de lugar)
**When** se construye la petición
**Then** la sección de ubicación contiene la ubicación o las coordenadas
**And** NO contiene ninguna fecha ni hora

#### Scenario: Sin coordenadas no hay sección de ubicación
**Given** un `BrowserContext` con marca de tiempo y zona pero sin coordenadas
**When** se construye la petición
**Then** el mensaje de sistema NO contiene ninguna sección de ubicación

### Requirement: Los mensajes conversacionales SHALL llevar su marca de tiempo

Cada mensaje `user` incluido en la petición SHALL ir prefijado con su marca de tiempo en la zona efectiva del turno, con el formato `[YYYY-MM-DD HH:MM]` y un espacio antes del contenido. Para los mensajes del historial la marca SHALL derivarse de su `created_at`; para el mensaje del turno actual, del instante efectivo del turno. Los mensajes `assistant` SHALL NOT prefijarse: el modelo imita el formato de sus propios turnos y lo reproduciría en sus respuestas. Al montar el historial, una marca `[YYYY-MM-DD HH:MM]` inicial heredada en el contenido de un `assistant` SHALL retirarse antes de enviarlo al LLM, de modo que el patrón no vuelva a primar al modelo aunque la fila almacenada la conserve; esa retirada SHALL NOT modificar la base de datos. Los mensajes `role:"tool"`, los de `role:"system"` y los de contenido vacío o solo espacios SHALL NOT prefijarse. La marca SHALL NOT persistirse en la base de datos —SHALL existir únicamente en la petición al LLM—, de modo que el contenido almacenado y el mostrado por la interfaz queden intactos. Una `created_at` ilegible SHALL dejar el mensaje sin prefijo, sin romper la petición.

**Given** un historial de conversación y un mensaje de turno
**When** el orquestador construye la petición
**Then** cada mensaje `user` con contenido SHALL llevar delante `[YYYY-MM-DD HH:MM]`
**And** los mensajes `assistant`, `tool` y los vacíos quedan sin prefijo
**And** el contenido persistido en la base de datos NO cambia

#### Scenario: El historial va prefijado
**Given** un mensaje `user` del historial con `created_at`
**When** se construye la petición
**Then** empieza por `[YYYY-MM-DD HH:MM] ` en la zona del turno

#### Scenario: Las respuestas del asistente viajan sin marca
**Given** un mensaje `assistant` del historial
**When** se construye la petición
**Then** su contenido viaja sin marca de tiempo

#### Scenario: Una marca heredada del asistente se retira
**Given** un mensaje `assistant` cuyo contenido almacenado empieza por `[YYYY-MM-DD HH:MM] `
**When** se construye la petición
**Then** ese prefijo heredado NO aparece en el contenido enviado al LLM
**And** el contenido almacenado en la base de datos NO se modifica

#### Scenario: Las herramientas no se prefijan
**Given** un mensaje `role:"tool"` del historial
**When** se construye la petición
**Then** su contenido viaja sin marca de tiempo

#### Scenario: El mensaje del turno va prefijado
**Given** un mensaje del usuario y un instante efectivo de turno
**When** se construye la petición
**Then** el mensaje del turno empieza por `[YYYY-MM-DD HH:MM] ` con el instante efectivo

#### Scenario: Una fecha ilegible no rompe
**Given** un mensaje del historial con `created_at` no parseable
**When** se construye la petición
**Then** ese mensaje viaja sin prefijo
**And** la petición se construye con normalidad

### Requirement: El instante y la zona horaria del turno SHALL resolverse una vez por turno

El instante efectivo del turno SHALL ser la marca de tiempo del `BrowserContext` cuando esté presente y sea parseable, y `Utc::now()` en caso contrario. La zona efectiva SHALL ser la zona del `BrowserContext` cuando esté presente y no vacía, si no `settings.timezone`, y si no `Europe/Madrid`; una zona inválida SHALL caer a UTC. Ambos SHALL resolverse una sola vez por turno y SHALL ser la misma fuente para la sección temporal y para las marcas del historial y del turno.

**Given** un turno con o sin `BrowserContext`
**When** el orquestador construye la petición
**Then** el instante y la zona efectivos quedan fijados
**And** la sección temporal y todas las marcas de tiempo usan esa misma zona

#### Scenario: El navegador manda el instante y la zona
**Given** un `BrowserContext` con timestamp y zona válidos
**When** se construye la petición
**Then** la sección temporal y las marcas se calculan con ese instante y esa zona

#### Scenario: Sin navegador, se usa el reloj del sistema y la zona guardada
**Given** una invocación sin `BrowserContext` y `settings.timezone = "Europe/Madrid"`
**When** se construye la petición
**Then** el instante es el actual del sistema
**And** la zona usada es `Europe/Madrid`

#### Scenario: Zona inválida cae a UTC
**Given** una zona efectiva inválida
**When** se formatea la marca de tiempo
**Then** se usa UTC sin abortar

### Requirement: Guardas del bucle ReAct para llamadas a herramientas

El orquestador SHALL limitar las llamadas a herramientas en el bucle ReAct con **tres guardas
ortogonales** en lugar de un contador único por nombre de herramienta, de modo que el **progreso**
(muchas llamadas con argumentos distintos) no se confunda con un **bucle** (la misma llamada
repetida o un fallo repetido). Los umbrales SHALL leerse de `settings` **en cada turno**
(`REACT_TOOL_FINGERPRINT_LIMIT` default `2`, `REACT_TOOL_FAILURE_LIMIT` default `3`,
`REACT_TOOL_SUCCESS_LIMIT` default `50`, `REACT_TOOL_TOTAL_LIMIT` default `100`), cayendo al default en silencio si la clave
**falta**, y al default con un `warn!` si el valor **no es parseable** (mismo contrato que los
`GENERATION_*`). Los contadores SHALL vivir en memoria y
reiniciarse en cada turno. Las tres guardas son:

1. **Huella** — la repetición de la **misma** llamada (mismo nombre de tool, misma `operation` y
   mismos argumentos normalizados), con independencia de su resultado, SHALL bloquearse tras
   `REACT_TOOL_FINGERPRINT_LIMIT` ejecuciones.
2. **Presupuesto por `tool::operation`** — contabilizado aparte para **éxitos** y **fallos
   consecutivos**; SHALL bloquearse al alcanzar `REACT_TOOL_SUCCESS_LIMIT` éxitos o
   `REACT_TOOL_FAILURE_LIMIT` fallos consecutivos. Un éxito SHALL reiniciar el contador de fallos
   consecutivos.
3. **Tope global del turno** — SHALL bloquearse toda llamada al alcanzar `REACT_TOOL_TOTAL_LIMIT`
   llamadas ejecutadas en el turno.

El orden de comprobación SHALL ser: tope global, huella, éxito, fallo. La comprobación SHALL
hacerse **antes** de ejecutar la herramienta y la actualización de contadores **después**, según el
`success` real del resultado (un `ToolResult { success: false }` cuenta como fallo). Un corte NO
DEBE romper el bucle: se DEBE pasar al LLM un mensaje `tool` con el **motivo exacto** y continuar,
se DEBE emitir `SSEEvent::ToolResult { success: false }` y se DEBE registrar un `warn!` con el
nombre de la herramienta y el motivo.

**Given** un turno con `REACT_TOOL_SUCCESS_LIMIT = 50`, `REACT_TOOL_FAILURE_LIMIT = 3`, `REACT_TOOL_FINGERPRINT_LIMIT = 2` y `REACT_TOOL_TOTAL_LIMIT = 100`  
**When** el LLM llama a la misma herramienta con argumentos distintos y todas triunfan  
**Then** ninguna llamada se bloquea hasta el éxito número 50  

#### Scenario: Marcar N elementos distintos no se bloquea
**Given** el default `REACT_TOOL_SUCCESS_LIMIT = 50`
**When** el LLM llama a `email::mark_read` 10 veces, cada una con un `id` distinto, y todas triunfan
**Then** las 10 llamadas se ejecutan
**And** ninguna se bloquea por la guarda de éxito ni por la de huella

#### Scenario: La misma llamada repetida se corta por huella
**Given** `REACT_TOOL_FINGERPRINT_LIMIT = 2`
**When** el LLM llama a `geo::search` dos veces con **argumentos idénticos**
**Then** ambas se ejecutan
**When** el LLM la llama una tercera vez con los mismos argumentos
**Then** la tercera NO se ejecuta
**And** el mensaje al LLM nombra el motivo «huella» (no «3 times»)
**And** el ReAct loop continúa

#### Scenario: Los fallos consecutivos cortan antes que los éxitos
**Given** `REACT_TOOL_FAILURE_LIMIT = 3`
**When** el LLM llama a la misma `tool::operation` con argumentos distintos y falla 3 veces seguidas
**Then** la siguiente llamada al mismo `tool::operation` se bloquea
**And** el mensaje nombra el motivo «fallo»

#### Scenario: Un éxito reinicia el contador de fallos
**Given** `REACT_TOOL_FAILURE_LIMIT = 3`
**When** el tool falla 2 veces seguidas y luego triunfa
**Then** el contador de fallos consecutivos vuelve a 0
**And** dos fallos posteriores no bloquean todavía

#### Scenario: El tope global corta cualquier herramienta
**Given** `REACT_TOOL_TOTAL_LIMIT = 5`
**When** se han ejecutado 5 llamadas en el turno (de cualquier herramienta)
**Then** cualquier llamada posterior se bloquea
**And** el mensaje nombra el motivo «tope global»

#### Scenario: Los contadores se reinician en el turno siguiente
**Given** un turno que alcanzó un umbral y bloqueó una herramienta
**When** empieza el turno siguiente
**Then** los contadores arrancan a cero
**And** la misma herramienta vuelve a poder ejecutarse

#### Scenario: Un umbral no parseable cae al default
**Given** `REACT_TOOL_SUCCESS_LIMIT = "muchos"`
**When** el orquestador lee sus umbrales
**Then** `success_limit` es `50`
**And** se registra un `warn!`

### Requirement: El orquestador SHALL proyectar widgets deterministas al cierre del turno

Al cerrar el turno, y **antes** de persistir el mensaje del asistente, el orquestador SHALL ejecutar una
proyección determinista sobre el texto final del asistente (`content_buffer`): detectar una forma de
presentación **y el tramo que ocupa**, sintetizar el widget correspondiente con ese tramo, fusionarlo con los
widgets ya renderizados por el modelo (`render_widget`), de modo que un widget del modelo gane a la proyección
de su mismo tipo y no se persista ningún duplicado, persistir el resultado con `MessagesRepo::set_widgets` y
emitir el/los evento(s) SSE `widget` **antes** del evento `done` del turno.

La proyección SHALL NOT depender de que el modelo invoque `render_widget`: es el **servidor** quien decide la
presentación a partir de lo que el modelo está a punto de presentar. La proyección SHALL ser una función
**pura** —sin I/O ni async—, de modo que cada detección y, sobre todo, cada **no** detección sea verificable
con tests unitarios.

El **disparador SHALL ser el texto**: si el texto final no presenta una forma reconocible, no SHALL proyectarse
ningún widget y `widgets` SHALL quedar como `null`, sin cambio alguno respecto al comportamiento previo. Las
formas reconocibles SHALL ser, al menos:

- **opciones**: una lista de opciones etiquetadas (`A)`, `A.`, `A:`, `1)`, `1.`, `Opción A`); una lista de
  alternativas (en viñetas o numerada) cerrada por una interrogativa del tipo «¿cuál…?», aunque entre la lista
  y la interrogativa haya **líneas en blanco**; o una **tabla Markdown que declare alternativas**: con una
  cabecera que **nombre** la columna de opciones (`Opción`/`Opcion`/`Alternativa`/`Alternativas`/`Option`/
  `Options`), o con una primera columna cuyas celdas lleven **letras** consecutivas (`A`, `B`, `C`) y ninguna
  celda de la cabecera nombre la columna de opciones;
- **lista de tareas**: una tabla Markdown cuya primera columna es un índice o una casilla, o una lista de
  viñetas con forma de tarea (cosas que marcar);
- **lugar**: una latitud y una longitud.
  La forma «dirección» queda **fuera de la v1**: el `LocationWidget` del cliente exige coordenadas numéricas y,
  sin ellas, muestra un aviso en lugar del mapa, de modo que proyectar una dirección suelta sería un widget
  peor que la prosa. Geocodificar no es viable porque la proyección SHALL ser pura.

- **elección de cierre**: la **última línea con pregunta** del **párrafo final** del turno es una pregunta
  cerrada de sí/no o una pregunta de dos caminos «¿X o Y?»;

Una columna cuyas celdas sean **números** consecutivos SHALL leerse como un **índice** de la tabla (vueltas,
divisiones, enumeraciones de datos) y SHALL NOT producir opciones: la rama heurística sin cabecera que nombre
opciones SHALL exigir etiquetas **alfabéticas**.

Una pregunta de cierre SHALL reconocerse **solo** en el **párrafo final** del turno: SHALL buscarse hacia atrás
la última línea con contenido que contenga `?`, **sin cruzar una línea en blanco** (frontera de párrafo) y sin
salir de un bloque de código delimitado por vallas. La línea SHALL NOT llevar ningún interrogativo abierto
(`qué`, `cuál`, `quién`, `cómo`, `cuándo`, `dónde`, `cuánto`, `por qué`, y sus formas plurales o femeninas
acentuadas `cuáles`, `quiénes`, `cuántos`, `cuántas`, `cuánta`, `adónde`).

Antes de extraer la pregunta, la línea SHALL normalizarse con `strip_line_prefix` y
`strip_markdown_emphasis`. Sobre la línea normalizada, la pregunta SHALL extraerse entre el **último** `¿` de la
línea y el **primer** `?` a partir de ahí. Cuando la línea SHALL NOT llevar `¿`, la pregunta SHALL empezar en su
primer carácter con contenido y SHALL terminar en el primer `?`, que SHALL considerarse de cierre **solo** si
tras él no queda ningún carácter alfanumérico (espacios, emoji, puntuación o marcadores): un `?` con texto
alfanumérico detrás SHALL NOT cerrar la pregunta, de modo que una línea como `Más info en
https://ejemplo.com/x?from=2026-11-01` SHALL NOT ser una pregunta. El `span` SHALL calcularse siempre sobre el
texto **original** y SHALL NOT verse alterado por la normalización.

Si la pregunta contiene una conjunción `o`/`ó`/`u` a nivel de palabra y la palabra siguiente **no** es
`no`/`sí`/`si`, la pregunta SHALL ser de **dos caminos**: SHALL partirse por **la** conjunción y SHALL
emitirse **dos opciones** con los dos tramos. Si la palabra siguiente a la conjunción **sí** es `no`/`sí`/`si`,
la pregunta SHALL ser **binaria** y SHALL emitirse con las dos opciones fijas `Sí` y `No`.

Cada tramo de una pregunta de dos caminos SHALL recortarse, quitar sus guiones o rayas y comas, `;` y `:` finales, quitar
el `¿` inicial y el `?` final, y capitalizar la primera letra. La elección de dos caminos SHALL NOT proyectarse
si: hay **más de una** conjunción `o`/`ó`/`u` (una elección de tres o más caminos), algún tramo queda vacío o
con menos de 3 caracteres alfanuméricos, o algún tramo supera los 160 caracteres.

La elección de cierre SHALL emitirse como `Presentation::ClosingChoice { question, options, span }`, donde
`options` es `["Sí","No"]` en el caso binario o los dos caminos en el caso «¿X o Y?». SHALL NOT proyectarse si
el turno ya produjo un widget de **opciones**: SHALL proyectarse **una sola** cuando ambas formas compitan. La
**pregunta SHALL ser la etiqueta del grupo de opciones** (`label`) y el widget SHALL NOT llevar `title`: el
`span` sustituye el texto que ocupa, de modo que sin la pregunta dentro del widget esta desaparecería de la
pantalla. El `span` SHALL delimitar exactamente la línea de la pregunta, y la etiqueta SHALL llevar el texto de
esa línea sin los marcadores de énfasis Markdown.

El **texto de cada opción de una tabla SHALL ser el de su fila completa**: todas las celdas con contenido de la
fila, en el orden de la tabla, unidas por ` — `, con la etiqueta incluida cuando la haya. La proyección SHALL
NOT quedarse con una sola celda de la fila. Una fila sin ninguna celda con texto SHALL NOT aportar opción, y si
quedan menos de dos opciones no SHALL proyectarse widget.

Los **marcadores de énfasis Markdown** (`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``) SHALL ignorarse al extraer
la etiqueta y el texto de cada opción o ítem, de modo que el widget muestre el texto interior. El `span` SHALL
calcularse siempre sobre el texto **original**, sin verse afectado por ese ignorado.

Los **datos** del widget SHALL tomarse del `data` de la herramienta del turno cuando esa herramienta haya
producido la lista (por ejemplo `tasks::list_tasks`, `tasks::search_tasks`, `email_list_unread`,
`rss_list_unread` o `notifications::list_unread`); en su defecto, SHALL tomarse del texto.

La proyección SHALL declarar, para cada widget proyectado, el **tramo** del texto que ese widget ocupa
(`span`), de modo que el cliente pueda colocar el widget **en su sitio** en lugar de debajo del texto (ver el
requisito siguiente).

Si el modelo ya renderizó con éxito un widget de un tipo (`render_widget`), la proyección **para ese tipo**
SHALL omitirse: el widget del modelo gana, porque su payload es más rico.

La verificación de la elección de cierre SHALL incluir un **corpus de regresión con cadenas literales extraídas
de producción**, además de los casos sintéticos, de modo que un cambio en la normalización o en las guardas no
pueda volver a pasar en verde sin cubrir la prosa real del modelo.

#### Scenario: Una lista de opciones acaba en widget sin que el modelo lo pida

**Given** un turno cuyo texto final del asistente presenta dos o más opciones etiquetadas y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con `widgets` no vacío
**And** el widget SHALL ser de opciones, con las opciones del texto
**And** el orquestador SHALL emitir un evento SSE `widget` antes del evento `done`

#### Scenario: Una lista de tareas acaba en widget

**Given** un turno cuyo texto final presenta una lista de cosas que marcar y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con un widget de lista de tareas
**And** sus ítems SHALL conservar el texto de cada elemento

#### Scenario: El widget del modelo gana y no se duplica

**Given** un turno en el que el modelo invocó `render_widget` con un widget de lista de tareas
**And** el texto final presenta además una lista de tareas
**When** el turno cierra
**Then** `widgets` SHALL contener **un solo** widget de lista de tareas, el del modelo
**And** la proyección para ese tipo SHALL omitirse
**And** no SHALL persistirse ningún widget duplicado

#### Scenario: Sin forma reconocible no hay proyección

**Given** un turno cuyo texto final es un dato único o una explicación, sin opciones ni listas que marcar
**When** el turno cierra
**Then** `widgets` SHALL ser `null`
**And** no SHALL emitirse ningún evento SSE `widget`

#### Scenario: Los datos salen del resultado de la herramienta cuando existe

**Given** un turno que llamó a una herramienta que produjo la lista
**And** el texto final presenta esa lista
**When** el turno cierra
**Then** los ítems del widget SHALL salir del `data` de esa herramienta
**And** SHALL NOT depender de lo que el modelo haya escrito en prosa

#### Scenario: Los datos salen del texto cuando no hay herramienta

**Given** un turno que no llamó a ninguna herramienta que produjera la lista
**And** el texto final presenta una lista de opciones
**When** el turno cierra
**Then** las opciones del widget SHALL salir del texto

#### Scenario: La proyección declara el tramo que el widget ocupa

**Given** un turno cuyo texto final presenta una lista de opciones y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el widget proyectado SHALL llevar un `span` con el tramo del texto que ocupa
**And** el tramo SHALL corresponder exactamente al bloque que el proyector reconoció
**And** SHALL NOT solaparse con el `span` de otro widget del mismo mensaje

#### Scenario: Una lista de opciones separada de la interrogativa por líneas en blanco acaba en widget

**Given** un turno cuyo texto final presenta una lista numerada o con viñetas de dos o más alternativas, seguida de una o más líneas en blanco y de una interrogativa del tipo «¿cuál…?» (por ejemplo «¿Cuál prefiere?»), y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con `widgets` no vacío
**And** el widget SHALL ser de opciones, con las alternativas del texto
**And** el `span` SHALL delimitar únicamente el bloque de la lista, dejando fuera las líneas en blanco y la interrogativa

#### Scenario: Los ítems con énfasis Markdown se muestran sin los marcadores

**Given** un turno cuyo texto final presenta alternativas cuyo texto está envuelto en marcadores de énfasis Markdown (`**…**`, `*…*`, `_…_`, `` `…` ``)
**When** el turno cierra
**Then** las opciones del widget SHALL llevar el texto interior, sin los marcadores Markdown
**And** el `span` SHALL seguir correspondiendo al tramo del texto original

#### Scenario: Una tabla de opciones acaba en widget

**Given** un turno cuyo texto final presenta una tabla Markdown cuya cabecera nombra la columna de opciones (`| Opción | Acción |`) y cuyas filas llevan la etiqueta en esa columna y el texto de la alternativa en la siguiente
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de opciones
**And** cada opción SHALL ser el texto completo de su fila, con la etiqueta incluida
**And** el `span` SHALL delimitar la tabla

#### Scenario: Una tabla con una columna de números no es una tabla de opciones

**Given** un turno cuyo texto final presenta una tabla Markdown de datos cuya primera columna es un índice numérico consecutivo (`| Lap | Dist. | Ritmo | Pulso |`, `| Split | Ritmo | Desnivel |`)
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de opciones para esa tabla

#### Scenario: Una tabla de tareas no se degrada a opciones

**Given** un turno cuyo texto final presenta una tabla Markdown cuya primera columna es un índice o una casilla (forma de lista de tareas)
**When** el turno cierra
**Then** SHALL proyectarse un widget de lista de tareas
**And** SHALL NOT proyectarse un widget de opciones para esa tabla

#### Scenario: Una fila sin contenido no aporta opción

**Given** una tabla Markdown de opciones en la que una fila no tiene ninguna celda con texto
**When** el turno cierra
**Then** esa fila SHALL NOT aportar ninguna opción al widget

#### Scenario: Una pregunta de sí/no que cierra el turno acaba en widget de elección

**Given** un turno cuyo texto final es `> ¿Le saco el detalle?` o `> ¿Cerramos por hoy, Lorenzo?` y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con las opciones `Sí` y `No`
**And** la pregunta SHALL ser la etiqueta del grupo de opciones
**And** el widget SHALL NOT llevar `title`
**And** el `span` SHALL delimitar la línea de la pregunta

#### Scenario: El emoji tras el signo de interrogación no impide la elección

**Given** un turno cuyo texto final es `> ¿Le doy el parte del día — tiempo, agenda y entrenamiento — o va directo a algo concreto? ☕️` o `> ¿Le pongo un recordatorio un rato antes, o lo dejo así? ☕️`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección
**And** el emoji SHALL NOT impedir la elección **porque la pregunta termina en el `?`**: lo que sigue al `?` SHALL ser solo no alfanumérico
**And** el `span` SHALL seguir delimitando la línea original, emoji incluido

#### Scenario: Una pregunta con alternativas nombradas no es una elección binaria

**Given** un turno cuyo texto final es `> ¿Le busco el repositorio y las instrucciones de instalación, o lo dejamos para mañana con luz? ☕️`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con **dos** opciones, los dos caminos
**And** la primera opción SHALL ser `Le busco el repositorio y las instrucciones de instalación`
**And** la segunda opción SHALL ser `Lo dejamos para mañana con luz`
**And** SHALL NOT emitirse las opciones fijas `Sí` y `No`
**And** la pregunta completa SHALL ser la etiqueta del grupo de opciones
**And** el widget SHALL NOT llevar `title`

#### Scenario: Una pregunta de dos caminos con la conjunción en medio de la frase se parte por la última conjunción

**Given** un turno cuyo texto final es `> ¿Quiere que le suba el tono de guasa o se lo deje más sobrio? Confirme y sale disparado. 🎩`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección con las opciones `Quiere que le suba el tono de guasa` y `Se lo deje más sobrio`
**And** el `span` SHALL delimitar la línea de la pregunta

#### Scenario: La forma binaria «o no» se mantiene como Sí/No

**Given** un turno cuyo texto final es `> ¿Hacemos esto o no?`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con las opciones `Sí` y `No`
**And** SHALL NOT emitirse dos opciones con los tramos de la pregunta

#### Scenario: Una pregunta de tres caminos no se proyecta

**Given** un turno cuyo texto final es `> ¿Prefieres A, B o C?`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Una pregunta abierta no es una elección binaria

**Given** un turno cuyo texto final es `> ¿Qué necesita? 😒`, `> ¿Cuál, Lorenzo?` o `> ¿Por dónde empezamos, Lorenzo?`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Un cierre sin pregunta no proyecta elección

**Given** un turno cuyo texto final es `> Dígame. ☕️` o `> Todo cerrado, Lorenzo. **Ahora sí, a la calle** — el progresivo no se corre desde el sofá. 😒☕️`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: La pregunta no tiene que ser la última línea del párrafo final

**Given** un turno cuyo texto final es `### 📋 ¿Seguimos?` o una línea `…o se lo deje más sobrio? Confirme y sale disparado. 🎩`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección para esa pregunta
**And** el `span` SHALL delimitar la pregunta: hasta el final de la línea cuando tras el `?` solo haya emoji o puntuación, y hasta el `?` inclusive cuando tras él venga prosa

#### Scenario: Una pregunta que no cierra el turno no proyecta elección binaria

**Given** un turno cuyo texto final lleva una pregunta de sí/no en un párrafo anterior, seguida de una línea en blanco y de un párrafo final sin pregunta
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Una pregunta dentro de un bloque de código no se proyecta

**Given** un turno cuyo texto final contiene una pregunta de sí/no dentro de un bloque de código delimitado por vallas
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección para esa pregunta

#### Scenario: El turno ya proyectó opciones y no se añade la elección binaria

**Given** un turno cuyo texto final presenta una lista de alternativas cerrada por una interrogativa y además una pregunta de sí/no al final
**When** el turno cierra
**Then** SHALL proyectarse un solo widget de opciones
**And** SHALL NOT proyectarse ningún widget de elección de cierre

#### Scenario: Un procedimiento seguido de una pregunta de sí/no no se degrada a opciones

**Given** un turno cuyo texto final presenta una lista numerada de pasos y termina en una pregunta cerrada de sí/no (por ejemplo «1. Abre el fichero\n2. Edita la línea\n¿Quieres que continúe?»)
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección con las opciones `Sí` y `No`
**And** SHALL NOT proyectarse un widget de opciones para el procedimiento

### Requirement: El contenido del mensaje del asistente SHALL persistirse íntegro y el widget SHALL declarar el tramo que ocupa

El `content_buffer` que se persiste SHALL ser el texto del asistente **íntegro**: la proyección SHALL NOT
recortar, resumir, reordenar ni reescribir el contenido del modelo, de modo que el índice FTS, el contexto
que ve el modelo (`effective_content`) y la auditoría sigan viendo exactamente lo que el modelo dijo.

Cada widget **proyectado** SHALL llevar un campo `span` con el rango **semiabierto** `[start, end)` —en
**bytes UTF-8** sobre ese contenido— del tramo que el widget ocupa, que SHALL corresponder exactamente al
bloque que `detect` reconoció. Los tramos de un mismo mensaje SHALL NOT solaparse. Los widgets del modelo
(`render_widget`) SHALL NOT llevar `span`: no sustituyen ningún tramo del texto.

El campo `span` SHALL ser **opcional** en todo el recorrido del widget —el evento SSE `widget` y la columna
`messages.widgets`—, de modo que un widget sin `span`, o una fila persistida antes de este change, se
comporte como hoy: el widget se pinta **debajo** del texto. No SHALL escribirse ninguna migración de datos
para añadir el campo a las filas existentes.

El evento `done` SHALL llevar, en el campo `content`, ese mismo contenido íntegro, que es la **base
autoritativa** contra la que los `span` están calculados (ver `stream-route`).

#### Scenario: El contenido persistido conserva el texto íntegro del modelo

**Given** un turno cuyo texto final presenta una lista de opciones
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el contenido persistido SHALL contener el bloque de opciones tal y como lo escribió el modelo
**And** `widgets` SHALL contener el widget de esas opciones
**And** SHALL NOT haberse recortado ninguna parte del texto

#### Scenario: El widget declara el tramo exacto que ocupa

**Given** un turno cuyo texto final rodea la lista de opciones con una frase de encabezado y una de cierre
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el widget SHALL llevar un `span`
**And** el tramo del contenido que el `span` delimita SHALL ser exactamente el bloque de opciones
**And** la frase de encabezado y la de cierre SHALL quedar fuera del tramo

#### Scenario: Los widgets del modelo no declaran tramo

**Given** un turno en el que el modelo invocó `render_widget`
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el widget persistido SHALL NOT llevar `span`

#### Scenario: Un turno sin proyección persiste el texto íntegro

**Given** un turno cuyo texto final no presenta ninguna forma reconocible
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el contenido persistido SHALL ser el texto íntegro del asistente
**And** `widgets` SHALL ser `null`

### Requirement: El orquestador SHALL persistir las herramientas usadas en `llm_requests.tool_calls`

En cada llamada de chat del bucle ReAct, el orquestador SHALL grabar en `llm_requests.tool_calls` un JSON con los nombres de las herramientas solicitadas por el modelo en esa respuesta, con la forma `[{"name":"<tool>"}, …]`. SHALL aplicar la misma precedencia que la ejecución: los `tool_calls` del evento `Done` y, solo si el `Done` no los trae, los acumulados del stream. Cuando la respuesta no solicite ninguna herramienta, la fila SHALL grabarse con `tool_calls = NULL`. Esto SHALL hacer que `StatsRepo::tools_summary()` (que lee `llm_requests.tool_calls` filtrando `kind='chat'`) refleje el uso real de herramientas.

#### Scenario: Una respuesta con tool_calls queda registrada
**Given** una llamada de chat cuyo evento `Done` trae `tool_calls = [{name:"get_weather"},{name:"search_web"}]`
**When** el orquestador persiste la llamada con `record_request`
**Then** la fila de `llm_requests` tiene `kind='chat'` y `tool_calls = '[{"name":"get_weather"},{"name":"search_web"}]'`
**And** `StatsRepo::tools_summary()` cuenta `get_weather: 1` y `search_web: 1`

#### Scenario: Una respuesta sin tool_calls no ensucia el recuento
**Given** una llamada de chat cuya respuesta no solicita ninguna herramienta
**When** el orquestador persiste la llamada
**Then** la fila tiene `tool_calls = NULL`
**And** `StatsRepo::tools_summary()` no la cuenta

#### Scenario: Precedencia Done sobre stream
**Given** un `Done` sin `tool_calls` y `tool_calls_from_stream` con `[{name:"rss_list_unread"}]`
**When** el orquestador persiste la llamada
**Then** la fila graba `tool_calls = '[{"name":"rss_list_unread"}]'`
