# workers Specification

## Purpose
Workers en segundo plano de Valet: colapso de mensajes largos y memoria episódica, con sus prompts y modelos configurables.

## Requirements

### Requirement: CollapseWorker SHALL process messages in background
**Given** a message ID is sent through the collapse channel  
**When** the `CollapseWorker` receives it  
**Then** it SHALL read the message from the database  
**And** it SHALL call the LLM with the collapse prompt  
**And** it SHALL update `collapsed_content` and `collapsed_tokens_count` in the database

#### Scenario: Worker collapses a long message
**Given** a message with id "msg-1" and content of 8000 chars exists in DB  
**When** "msg-1" is sent through the collapse channel  
**Then** the worker SHALL read the message  
**And** SHALL call `llm_provider.chat()` with the collapse prompt  
**And** SHALL update `collapsed_content` to the LLM response  
**And** SHALL update `collapsed_tokens_count` to `estimate_tokens(response)`

### Requirement: Collapse prompt SHALL be configurable via settings
**Given** the `settings` table has a `collapse_prompt` key  
**When** the worker starts  
**Then** it SHALL read `collapse_prompt` from settings  
**And** SHALL use it as the system prompt for the LLM call  
**And** SHALL fall back to a minimal default prompt if the setting is missing, empty, or cannot be read due to a database error

#### Scenario: Default collapse prompt
**Given** a freshly migrated database with the seeded `collapse_prompt`  
**When** the worker processes a message  
**Then** it SHALL use the seeded prompt containing "Resume el siguiente texto"

#### Scenario: Custom collapse prompt
**Given** `collapse_prompt` = "Summarize in 3 bullet points" in settings  
**When** the worker processes a message  
**Then** it SHALL use "Summarize in 3 bullet points" as the system prompt

#### Scenario: Collapse prompt read failure is distinguished in logs
**Given** reading `collapse_prompt` fails with a database error
**When** the worker starts
**Then** it SHALL log a warning including the error and use the minimal fallback

#### Scenario: Missing or empty collapse prompt is distinguished in logs
**Given** `collapse_prompt` is absent or empty in settings
**When** the worker starts
**Then** it SHALL log a warning stating the key is missing or empty and use the minimal fallback

### Requirement: Collapse model SHALL be configurable via env var
**Given** `COLLAPSE_MODEL` env var  
**When** `Config::from_env()` is called  
**Then** `collapse_model` SHALL take the env var value  
**And** SHALL default to `mistralai/mistral-small-24b-instruct-2501`

#### Scenario: Default collapse model
**Given** no `COLLAPSE_MODEL` env var  
**When** `Config::from_env()` is called  
**Then** `collapse_model` SHALL be `"mistralai/mistral-small-24b-instruct-2501"`

#### Scenario: Custom collapse model
**Given** `COLLAPSE_MODEL` = `"google/gemini-2.0-flash-lite"`  
**When** `Config::from_env()` is called  
**Then** `collapse_model` SHALL be `"google/gemini-2.0-flash-lite"`

### Requirement: Handler SHALL wire collapse callback on message creation

**Given** a message with `tokens_count >= collapse_threshold_tokens`  
**When** `create_message` handler is called  
**Then** it SHALL send the message ID through the collapse channel  
**And** the threshold SHALL be read from `config.collapse_threshold_tokens` (not hardcoded)

#### Scenario: Short message does not trigger collapse
**Given** a message with 100 chars  
**When** created via the API  
**Then** the collapse channel SHALL NOT receive the message ID

#### Scenario: Long message triggers collapse
**Given** a message with 8000 chars  
**When** created via the API  
**Then** the collapse channel SHALL receive the message ID

#### Scenario: Custom threshold is honoured
**Given** `COLLAPSE_THRESHOLD_TOKENS` = `500`  
**When** a message with ~600 tokens is created  
**Then** the collapse channel SHALL receive the message ID

### Requirement: WorkerPool SHALL include CollapseWorker
**Given** a `WorkerPool::start()` call  
**Then** it SHALL spawn a `CollapseWorker`  
**And** `pool.collapse` SHALL be `Some`

#### Scenario: CollapseWorker starts and shuts down
**Given** a started WorkerPool
**When** `pool.shutdown()` is called
**Then** the collapse worker SHALL be aborted

### Requirement: CollapseWorker SHALL accept model parameter
**Given** `CollapseWorker::start()`
**When** se invoca
**Then** SHALL aceptar un parámetro `model: String`
**And** SHALL usar ese modelo en `ChatRequest.model` en lugar del hardcodeado `"mistralai/mistral-small-24b-instruct-2501"`

#### Scenario: Worker usa el modelo pasado como parámetro
**Given** `model = "google/gemini-2.0-flash-lite"`
**When** se inicia el worker
**Then** `ChatRequest.model` SHALL ser `"google/gemini-2.0-flash-lite"`

### Requirement: WorkerPool SHALL use real CollapseWorker
**Given** `WorkerPool::start()`
**When** se inicia el pool
**Then** SHALL usar `CollapseWorker::start()` con el modelo de `config.collapse_model`
**And** SHALL leer `collapse_prompt` de settings y pasarlo al worker
**And** el placeholder actual (que solo logea) SHALL ser eliminado

#### Scenario: CollapseWorker recibe mensajes del canal
**Given** un WorkerPool iniciado con CollapseWorker real
**When** un mensaje largo se crea y supera el threshold
**Then** el worker SHALL recibir el message_id por el canal mpsc
**And** SHALL procesarlo (LLM + update DB)

### Requirement: Agent SHALL wire collapse callback
**Given** el orquestador con acceso a `collapse_tx`
**When** persiste mensajes de usuario y asistente
**Then** SHALL pasar `collapse_tx` como `on_collapse_needed` en `MessagesRepo::create()`

#### Scenario: Mensaje largo del usuario dispara collapse
**Given** un mensaje de usuario con 8000 chars
**When** el orquestador lo persiste
**Then** el message_id SHALL enviarse por el canal de collapse

#### Scenario: Mensaje largo del asistente dispara collapse
**Given** un mensaje de asistente con 8000 chars
**When** el orquestador lo persiste
**Then** el message_id SHALL enviarse por el canal de collapse

### Requirement: WorkerPool shutdown_tx SHALL be retained for application lifetime

**Given** a `WorkerPool` is created in `new_with_orchestrator()`  
**When** the pool finishes starting all workers  
**Then** the `shutdown_tx` broadcast sender SHALL NOT be dropped  
**And** all workers SHALL continue running until the application exits

#### Scenario: Workers survive beyond scope of new_with_orchestrator
**Given** `WorkerPool::start()` is called in `new_with_orchestrator`  
**When** the function returns the `AppState`  
**Then** all workers SHALL still be running (no "shutting down" log emitted)  
**And** `shutdown_tx` SHALL be retained in `AppState`

#### Scenario: Worker pool runs on startup
**Given** Valet is started  
**When** the server begins listening  
**Then** the Collapse worker SHALL still be running  
**And** the EpisodicMemoryWorker SHALL still be running

### Requirement: EpisodicMemoryWorker SHALL rate-limit LLM retries after parse failure

**Given** the worker calls `call_llm()` and receives `None` (unparseable response)  
**When** `evaluate()` is called again within the cooldown window (half of poll_interval, min 30s)  
**Then** the worker SHALL skip the LLM call  
**And** SHALL log a warning that the unindexed messages are still pending

#### Scenario: Consecutive evaluations skip LLM after failure
**Given** a batch of unindexed messages that failed LLM parsing  
**When** `evaluate()` is called again within the cooldown window  
**Then** the LLM call SHALL be skipped  
**And** a warning is logged with the unindexed count

### Requirement: Archivist prompt SHALL be loaded from settings

**Given** the `settings` table has an `archivist_prompt` key seeded by migration
**When** `EpisodicMemoryWorker::call_llm()` builds the LLM request
**Then** it SHALL read `archivist_prompt` from settings
**And** SHALL substitute the `{{ BLOQUE_DE_MENSAJES }}` placeholder with the message block
**And** SHALL NOT use a hardcoded constant
**And** SHALL fall back to a minimal default prompt if the setting is missing, empty, or cannot be read due to a database error

#### Scenario: Seeded archivist prompt is used
- **GIVEN** a freshly migrated database with the seeded `archivist_prompt`
- **WHEN** the worker calls the LLM
- **THEN** the system message contains "archivista de memoria"
- **AND** the `{{ BLOQUE_DE_MENSAJES }}` placeholder is replaced by the message block

#### Scenario: Custom archivist prompt is used
- **GIVEN** `archivist_prompt = "CUSTOM ARCHIVIST {{ BLOQUE_DE_MENSAJES }}"` in settings
- **WHEN** the worker calls the LLM
- **THEN** the system message starts with "CUSTOM ARCHIVIST "
- **AND** contains the message block

#### Scenario: Fallback when archivist prompt is missing
- **GIVEN** a database without `archivist_prompt`
- **WHEN** the worker calls the LLM
- **THEN** a minimal non-empty fallback prompt is used
- **AND** a warning stating the key is missing or empty is logged

#### Scenario: Fallback when archivist prompt read fails
- **GIVEN** reading `archivist_prompt` fails with a database error
- **WHEN** the worker calls the LLM
- **THEN** a minimal non-empty fallback prompt is used
- **AND** a warning including the error is logged

### Requirement: Collapse channel SHALL NOT silently drop message IDs

**Given** the collapse channel is full  
**When** a message ID is sent  
**Then** the sender SHALL apply backpressure (await capacity) or log a warning  
**And** SHALL NOT discard the ID without any log

#### Scenario: Full channel does not lose the ID silently
**Given** a collapse channel with capacity 1 already holding one ID  
**When** a second ID is sent  
**Then** the ID SHALL be delivered once capacity frees up (or a warning SHALL be logged)

### Requirement: EpisodicMemoryWorker SHALL NOT re-call the LLM after a persist failure within the cooldown

**Given** the worker obtained a valid memory card from the LLM  
**When** `persist()` fails  
**Then** the worker SHALL start the cooldown window  
**And** SHALL NOT call the LLM again until the cooldown elapses

#### Scenario: Persist failure does not trigger an immediate LLM retry
**Given** a batch that produces a valid card but whose persist fails  
**When** `evaluate()` is called again within the cooldown  
**Then** the LLM SHALL NOT be called again

### Requirement: EpisodicMemoryWorker SHALL persist memory and embedding atomically

**Given** a memory card to persist  
**When** the worker writes to `memory` and `vec_memory`  
**Then** both writes SHALL happen in a single transaction  
**And** a failure in either SHALL leave no orphan row

#### Scenario: vec_memory failure leaves no orphan memory row
**Given** the `vec_memory` insert fails  
**When** `persist()` runs  
**Then** the `memory` table SHALL NOT contain a row for that card

### Requirement: Workers SHALL record LLM stats with a NULL profile_id

**Given** a worker (Collapse or EpisodicMemory) records an LLM request  
**When** it calls `StatsRepo::record_request`  
**Then** the `profile_id` SHALL be `NULL` (system operation)  
**And** SHALL NOT use a literal such as `"background"` or `"episodic"` that violates the FK  
**And** SHALL pass its origin kind: `CallKind::Collapse` for the CollapseWorker, `CallKind::Archivist` for the memory card and `CallKind::Consolidator` for the consolidation

#### Scenario: Collapse stats are recorded with NULL profile
**Given** a database with one profile  
**When** the CollapseWorker records an LLM request  
**Then** the `llm_requests` row SHALL be inserted successfully  
**And** its `profile_id` SHALL be `NULL`  
**And** its `kind` SHALL be `'collapse'`

#### Scenario: Episodic stats are recorded with NULL profile
**Given** a database with one profile  
**When** the EpisodicMemoryWorker records an LLM request  
**Then** the `llm_requests` row SHALL be inserted successfully  
**And** its `profile_id` SHALL be `NULL`

#### Scenario: Cada origen del EpisodicMemoryWorker registra su kind
**Given** una ficha (archivist) y una consolidación (consolidator) que llaman al LLM  
**When** se registran sus peticiones  
**Then** la ficha lleva `kind='archivist'` y la consolidación `kind='consolidator'`

### Requirement: EpisodicMemoryWorker SHALL use the EmbeddingProvider for embeddings

`EpisodicMemoryWorker` SHALL generar los embeddings de las fichas mediante un `Arc<dyn EmbeddingProvider>` inyectado, y SHALL NOT usar `LLMProvider::embed`. El mismo provider SHALL ser el usado por `ContextBuilder` para las consultas.

**Given** el `EpisodicMemoryWorker`  
**When** persiste una ficha de memoria  
**Then** SHALL generar el embedding vía `Arc<dyn EmbeddingProvider>`  
**And** SHALL NOT llamar a `LLMProvider::embed`  
**And** el provider SHALL ser el mismo que usa `ContextBuilder` para consultar

#### Scenario: persist usa EmbeddingProvider
**Given** un worker con un `EmbeddingProvider` mock  
**When** `persist()` guarda una ficha  
**Then** el mock registra la llamada a `embed`  
**And** el embedding se almacena en `vec_memory`

#### Scenario: Worker no arranca sin provider configurado
**Given** `WorkerPool::start` con `embedding_provider = None`  
**When** se construye el pool  
**Then** el worker episódico NO SHALL arrancar  
**And** SHALL loguearse un warning

### Requirement: El EpisodicMemoryWorker SHALL registrar cuándo ocurrieron los hechos en la metadata de la ficha

Al persistir una ficha, el `EpisodicMemoryWorker` SHALL escribir en la `metadata` de la ficha las claves `first_message_at` y `last_message_at`, con el `created_at` del mensaje de origen más antiguo y del más reciente de los que componen la ficha. SHALL NOT cambiar el esquema de `memory`: `metadata` ya es TEXT con JSON. El worker SHALL NOT alterar las demás claves de la `metadata` (`source`, `primary_message_ids`, `date_context`), que SHALL seguir escribiéndose igual.

**Given** una ficha a persistir con sus mensajes de origen en `metadata.primary_message_ids`  
**When** el `EpisodicMemoryWorker` la persiste  
**Then** la `metadata` SHALL contener `first_message_at` con el `created_at` del mensaje de origen más antiguo  
**And** la `metadata` SHALL contener `last_message_at` con el `created_at` del mensaje de origen más reciente  
**And** los valores SHALL proceder del `created_at` real de los mensajes, no del `created_at` de la ficha

#### Scenario: Una ficha de un lote con fechas produce ambas claves
**Given** un lote de mensajes de origen con `created_at` que van de `2026-09-29T17:13:00Z` a `2026-09-30T17:37:00Z`  
**When** el `EpisodicMemoryWorker` persiste la ficha que los resume  
**Then** `metadata.first_message_at` SHALL ser `2026-09-29T17:13:00Z`  
**And** `metadata.last_message_at` SHALL ser `2026-09-30T17:37:00Z`

#### Scenario: No cambia nada más de la metadata
**Given** una ficha a persistir cuyos mensajes de origen existen en `messages`  
**When** el `EpisodicMemoryWorker` la persiste  
**Then** `metadata.source` SHALL seguir escribiéndose como `episodic_worker`  
**And** `metadata.primary_message_ids` SHALL seguir conteniendo los identificadores de origen  
**And** `metadata.date_context` SHALL seguir escribiéndose  
**And** la `metadata` SHALL NOT perder ni renombrar ninguna de esas claves al añadir `first_message_at` y `last_message_at`

### Requirement: El worker SHALL escribir la Capa B y la Capa C en la misma pasada y con una sola marca

Al procesar un lote de mensajes sin indexar, el `EpisodicMemoryWorker` SHALL: (1) leer el lote
una sola vez; (2) obtener la ficha episódica (Capa B) y el estado persistente consolidado
(Capa C) **antes** de escribir nada; y (3) escribir la ficha, escribir el estado y marcar los
mensajes como indexados en una **única transacción**. `messages.is_indexed = 1` SHALL implicar
que la ficha (Capa B) se ha escrito y que existe un estado de Capa C válido —el recién
consolidado o, si el tamaño obligó a conservarlo, el anterior—.

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

### Requirement: Un fallo en cualquiera de las dos extracciones SHALL NOT dejar escritura parcial ni marca

Si falla la llamada episódica, la llamada de consolidación, la validación del estado o la
generación de embeddings, el worker SHALL NOT abrir transacción, SHALL NOT marcar los mensajes y SHALL
iniciar el cooldown. Al reintentar, SHALL reprocesar el lote completo sin duplicar fichas.

**Given** un lote cuya extracción episódica falla  
**When** el worker lo procesa  
**Then** SHALL NOT escribir nada  
**And** SHALL NOT marcar los mensajes  
**And** SHALL iniciar el cooldown

**Given** un lote cuya consolidación del estado falla tras una extracción episódica correcta  
**When** el worker lo procesa  
**Then** SHALL NOT escribir ni la ficha ni el estado  
**And** SHALL NOT marcar los mensajes  
**And** un reintento posterior SHALL producir una única ficha, sin duplicados

#### Scenario: Fallo episódico no escribe ni marca
**Given** un lote y una llamada episódica que falla  
**When** el worker procesa  
**Then** `memory` no gana filas  
**And** `messages.is_indexed` sigue en `0` para el lote  
**And** el cooldown queda activo

#### Scenario: Fallo del consolidador no deja ficha huérfana ni duplicados
**Given** un lote, una extracción episódica correcta y una consolidación que falla  
**When** el worker procesa  
**Then** NO se persiste la ficha  
**And** NO se persiste el estado  
**And** los mensajes siguen sin marcar  
**And** al reintentar con éxito se persiste exactamente una ficha

### Requirement: El modelo del consolidador SHALL ser configurable por variable de entorno y registrar stats

`Config::from_env()` SHALL leer `SEMANTIC_MODEL` y, si no está definida, SHALL usar el valor
de `MEMORY_MODEL`. La llamada de consolidación SHALL usar ese modelo en `ChatRequest.model`.
La llamada SHALL registrar stats con `profile_id` NULL y `kind='consolidator'`, como las demás llamadas de worker.

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

### Requirement: WorkerPool SHALL incluir el ReminderPushWorker

El `WorkerPool` SHALL arrancar un `ReminderPushWorker` junto al resto de workers y SHALL detenerlo
con la señal de `shutdown`, siguiendo el mismo patrón que `EpisodicMemoryWorker`.

#### Scenario: El arranque del pool arranca el worker

- **Given** un `WorkerPool::start()`
- **When** se inicia el pool
- **Then** arranca el `ReminderPushWorker`
- **And** la referencia al worker queda retenida en el pool

#### Scenario: El shutdown detiene el worker

- **Given** un `WorkerPool` en marcha
- **When** se emite la señal de `shutdown`
- **Then** el `ReminderPushWorker` se detiene

### Requirement: El worker SHALL entregar cada recordatorio vencido al menos una vez

En cada ciclo —de 60 segundos— el worker SHALL seleccionar los `reminders` con `status='pending'`,
`notified_at IS NULL` y `datetime <= ahora`, y al entregar uno con éxito SHALL fijar su `notified_at`.

La garantía es *at-least-once*: si el proceso muere entre el envío correcto y la marca, el ciclo
siguiente reenvía el aviso. El `tag` del payload evita apilar dos avisos del mismo recordatorio, pero
no elimina la repetición; un aviso puede repetirse en ese caso.

#### Scenario: Un recordatorio vencido se envía y se marca

- **Given** un recordatorio vencido con `status='pending'` y `notified_at` a `NULL`, y al menos una suscripción
- **When** el worker ejecuta un ciclo
- **Then** se envía el aviso
- **And** el recordatorio queda con `notified_at` fijado

#### Scenario: El mismo recordatorio no se reenvía en el ciclo siguiente

- **Given** un recordatorio ya entregado y marcado con `notified_at`
- **When** el worker ejecuta otro ciclo
- **Then** el recordatorio no se selecciona
- **And** no se envía ningún aviso de nuevo

#### Scenario: Un recordatorio futuro no se toca

- **Given** un recordatorio cuyo `datetime` es posterior a `ahora`
- **When** el worker ejecuta un ciclo
- **Then** el recordatorio no se selecciona
- **And** su `notified_at` sigue a `NULL`

### Requirement: El worker SHALL NO consumir un recordatorio cuando no hay ninguna suscripción

Sin dispositivos suscritos el worker SHALL NOT marcar `notified_at`: el recordatorio SHALL entregarse
cuando aparezca una suscripción, si sigue dentro de la ventana.

#### Scenario: Sin suscripciones no hay envío ni marca

- **Given** un recordatorio vencido y ninguna suscripción registrada
- **When** el worker ejecuta un ciclo
- **Then** no se envía ningún aviso
- **And** el recordatorio sigue con `notified_at` a `NULL`

#### Scenario: Al aparecer una suscripción se entrega el recordatorio vigente

- **Given** un recordatorio vencido que sigue dentro de la ventana y ninguna suscripción
- **When** se registra un dispositivo y el worker ejecuta el ciclo siguiente
- **Then** el recordatorio se entrega
- **And** su `notified_at` queda fijado

### Requirement: Los recordatorios vencidos fuera de la ventana SHALL descartarse sin entregarse

Si `ahora - datetime` supera `push_reminder_grace_hours` —por defecto 24 horas—, el worker SHALL
fijar `notified_at` y registrar un aviso, sin enviar, para no inundar de avisos antiguos al primer
dispositivo que se suscriba.

#### Scenario: Un recordatorio vencido dentro de la ventana se entrega

- **Given** un recordatorio vencido hace menos de la ventana configurada
- **When** el worker ejecuta un ciclo
- **Then** el recordatorio se entrega
- **And** su `notified_at` queda fijado

#### Scenario: Un recordatorio vencido fuera de la ventana se descarta con un aviso

- **Given** un recordatorio vencido hace más que la ventana configurada
- **When** el worker ejecuta un ciclo
- **Then** no se envía ningún aviso
- **And** su `notified_at` queda fijado
- **And** se registra un aviso de descarte

#### Scenario: El umbral se lee del ajuste y con '0' todo queda fuera de ventana

- **Given** `push_reminder_grace_hours` con el valor `'0'`
- **When** el worker evaluaba un recordatorio ya vencido
- **Then** se considera fuera de la ventana
- **And** se marca sin enviarse

### Requirement: El payload del aviso SHALL identificar el recordatorio

El payload del aviso SHALL llevar `title` «Recordatorio», `body` el texto del recordatorio, `url` `/`
y `tag` `reminder-<id>`, de modo que el `tag` evite que dos avisos del mismo recordatorio se apilen.

#### Scenario: El payload contiene el texto del recordatorio

- **Given** un recordatorio con su texto
- **When** el worker construye el payload del aviso
- **Then** el `body` es el texto del recordatorio
- **And** el `title` es «Recordatorio»
- **And** el `url` es `/`

#### Scenario: El tag incluye el id del recordatorio

- **Given** un recordatorio con su `id`
- **When** el worker construye el payload del aviso
- **Then** el `tag` es `reminder-<id>`

### Requirement: El envío SHALL ser inyectable para poder probarlo sin red

El worker SHALL depender de una abstracción `PushSender` y SHALL NO construir el cliente HTTP por
dentro, de modo que las pruebas puedan sustituirlo.

#### Scenario: Un PushSender de prueba registra las llamadas

- **Given** un `PushSender` de prueba inyectado en el worker
- **When** el worker entrega un recordatorio
- **Then** el sender de prueba registra la llamada de envío

#### Scenario: El worker no falla si el sender devuelve error

- **Given** un `PushSender` de prueba que devuelve un error
- **When** el worker intenta entregar un recordatorio
- **Then** el worker no se cae
- **And** registra el error

### Requirement: El resumen del ciclo SHALL contabilizar el resultado por suscripción

`TickOutcome` SHALL contar, además de sus contadores actuales de recordatorios (`delivered`,
`skipped_old`, `pruned`, `unparseable`), el resultado de cada envío individual: cuántos envíos se
intentaron (`attempted`), cuántos aceptó el push service (`accepted`) y cuántos fallaron (`failed`).
`failed` SHALL agrupar **todo lo que no fue aceptado ni depurado**: los rechazos del push service
(`Failed`, cualquier código que no sea 404/410) y los errores de transporte. Un envío que termina
en `Err(PushError)` —fallo de red, timeout o respuesta ilegible— SHALL contar como **intentado**
(`attempted`) y como **fallido** (`failed`), porque desde la perspectiva de la observabilidad es un
envío que **no llegó**, exactamente igual que un 5xx, y SHALL NOT desaparecer del recuento ni
contarse como aceptado. Una suscripción depurada por `Gone` (404/410) SHALL contar como
**intentada** y como **depurada** (`pruned`), y SHALL NOT contar como fallida. La decisión de
registrar el resumen SHALL vivir en un predicado puro sobre los contadores, `should_log_summary`,
que SHALL ser verdadero cuando **cualquiera** de ellos sea distinto de cero —incluidos `unparseable`
y `failed`—.

#### Scenario: Una mezcla de resultados cuenta cada suscripción por separado

- **Given** un recordatorio vencido y 3 suscripciones registradas
- **When** el sender devuelve `Delivered`, `Gone` y `Failed` respectivamente
- **Then** `attempted` es 3, `accepted` es 1, `pruned` es 1 y `failed` es 1
- **And** `delivered` es 1, porque al menos una entrega fue aceptada y el recordatorio se marca

#### Scenario: Un fallo de transporte cuenta como intentado y fallido

- **Given** un recordatorio vencido y una suscripción registrada
- **When** el sender devuelve `Err(PushError)` —un fallo de red, un timeout o una respuesta ilegible—
- **Then** `attempted` es 1, `failed` es 1, `accepted` es 0 y `pruned` es 0
- **And** `delivered` es 0 y el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Todos los envíos fallan pide resumen aunque no haya entrega

- **Given** un recordatorio vencido y 2 suscripciones registradas
- **When** el sender devuelve `Failed` en ambas
- **Then** `failed` es 2, `accepted` es 0 y `delivered` es 0
- **And** el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Un ciclo sin nada que hacer no registra resumen

- **Given** ninguna suscripción registrada y ningún recordatorio vencido
- **When** el worker ejecuta un ciclo
- **Then** todos los contadores son 0
- **And** `should_log_summary()` es falso

#### Scenario: Un datetime no parseable se cuenta y pide resumen

- **Given** un recordatorio pendiente sin notificar cuyo `datetime` no parsea
- **When** el worker ejecuta un ciclo
- **Then** `unparseable` es 1 y `delivered` es 0
- **And** el recordatorio NO se marca
- **And** `should_log_summary()` es verdadero

#### Scenario: Sin suscripciones no hay envíos intentados

- **Given** un recordatorio vencido y ninguna suscripción registrada
- **When** el worker ejecuta un ciclo
- **Then** `attempted` es 0 y `accepted` es 0
- **And** `delivered` es 0 y el recordatorio NO se marca

### Requirement: Los registros SHALL NOT exponer el endpoint completo de una suscripción

Un endpoint de suscripción es una URL de capacidad. Los registros del `ReminderPushWorker` SHALL
incluir solo el **host** del endpoint y el identificador de la suscripción, y SHALL NOT incluir la
URL completa del endpoint. La extracción del host SHALL ser un ayudante puro que devuelva el host de
una URL normal y un marcador (`(endpoint no parseable)`) cuando el valor no rinda un host
reconocible, sin devolver nunca la cadena original.

#### Scenario: Un fallo de envío redacta el endpoint

- **Given** una suscripción cuyo endpoint es `https://fcm.googleapis.com/fcm/send/<identificador-largo>`
- **When** el sender devuelve `Failed`
- **Then** el registro incluye `fcm.googleapis.com`
- **And** el registro NO incluye la ruta `/fcm/send/<identificador-largo>`

#### Scenario: El ayudante devuelve el host de una URL normal

- **Given** el endpoint `https://fcm.googleapis.com/fcm/send/abc`
- **When** se extrae el host
- **Then** devuelve `fcm.googleapis.com`

#### Scenario: El ayudante marca lo que no puede interpretar

- **Given** un endpoint sin host reconocible, como una cadena vacía
- **When** se extrae el host
- **Then** devuelve el marcador `(endpoint no parseable)`
- **And** NO devuelve la cadena original
