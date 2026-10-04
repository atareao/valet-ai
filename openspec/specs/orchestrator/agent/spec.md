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

### Requirement: Per-tool max retry limit of 3 in ReAct loop

El orquestador SHALL limitar a 3 las llamadas a una misma herramienta por bucle ReAct y continuar el bucle tras alcanzar el límite.
**Given** el orquestador ejecuta el ReAct loop
**When** un tool es invocado 3 veces o más en el mismo loop
**Then** NO DEBE ejecutarse el tool otra vez
**And** SE DEBE enviar un mensaje al LLM indicando que el tool no está disponible tras 3 intentos
**And** el ReAct loop DEBE continuar para que el LLM responda con alternativas

#### Scenario: Tool fails 3 times, 4th call is blocked
**Given** un orquestador con un tool que siempre falla
**When** el LLM llama al tool por 4ª vez en el mismo ReAct loop
**Then** el tool NO se ejecuta
**And** el LLM recibe el mensaje "Tool 'X' has been called 3 times. No more retries allowed."
**And** el ReAct loop continúa

#### Scenario: Successful tool calls don't count towards limit
**Given** un orquestador
**When** un tool se ejecuta con éxito 2 veces
**Then** el contador del tool aumenta a 2
**And** aún se puede llamar una 3ª vez
**And** el límite de 3 aplica tanto a fallos como a éxitos

#### Scenario: Different tools have independent counters
**Given** un orquestador
**When** el tool "geo" se ha llamado 3 veces y el tool "weather" 1 vez
**Then** "geo" está bloqueado
**And** "weather" aún puede ejecutarse

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
When a tool has been called 3 times in the same ReAct loop, the orchestrator
must log a warning with the tool name and call count.

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
la hay, y (5) la fecha, hora y ubicación si las hay. El mensaje SHALL preceder al historial de
conversación. Las secciones presentes SHALL separarse con una línea en blanco, y ninguna sección
ausente SHALL dejar título, marcador, separador ni línea en blanco.

**Given** un mensaje del usuario  
**When** el orquestador construye la petición al LLM  
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`  
**And** SHALL incluir después, y solo si existen, las secciones de nombre de usuario, memoria persistente, memoria episódica y fecha/hora/ubicación, en ese orden  
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador  
**When** se construye la petición  
**Then** hay exactamente un mensaje `role:"system"` antes del historial  
**And** ese mensaje contiene el prompt, la sección episódica y la sección de fecha

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, nombre de usuario, memoria persistente no vacía, fichas inyectables y contexto de navegador  
**When** se inspecciona el mensaje de sistema  
**Then** el prompt aparece antes que la sección con el nombre del usuario  
**And** la sección con el nombre del usuario aparece antes que la sección de memoria persistente  
**And** la sección de memoria persistente aparece antes que la sección episódica  
**And** la sección episódica aparece antes que la sección de fecha, hora y ubicación  
**And** la sección de fecha, hora y ubicación es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin nombre de usuario, sin fichas inyectables y sin contexto de navegador  
**When** se construye la petición  
**Then** el mensaje de sistema contiene solo el prompt  
**And** no aparece ningún título, marcador ni línea en blanco de relleno

### Requirement: La sección de fecha, hora y ubicación SHALL cerrar el mensaje de sistema

Cuando el navegador aporte contexto (`BrowserContext`), la sección de fecha, hora y ubicación SHALL ser la **última** del mensaje de sistema, con el formato actual: `{fecha}.` y, si hay coordenadas, `Ubicación: {nombre} ({lat}, {lon}).` cuando se resuelva el nombre, o `Coordenadas: ({lat}, {lon}).` cuando no. SHALL omitirse por completo, sin dejar rastro, cuando no haya contexto de navegador.

**Given** una invocación con o sin `BrowserContext`  
**When** el orquestador construye la petición  
**Then** SHALL añadir la sección de fecha, hora y ubicación como última sección del mensaje de sistema si hay contexto  
**And** SHALL omitirla por completo si no lo hay

#### Scenario: La fecha, hora y ubicación cierran el mensaje
**Given** un `BrowserContext` con marca de tiempo y coordenadas  
**When** se construye la petición  
**Then** la última sección del mensaje de sistema es la de fecha, hora y ubicación  
**And** el texto contiene la marca de tiempo formateada y la ubicación o las coordenadas

#### Scenario: Sin contexto no hay sección
**Given** una invocación sin `BrowserContext`  
**When** se construye la petición  
**Then** el mensaje de sistema NO contiene ninguna sección de fecha, hora ni ubicación

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
