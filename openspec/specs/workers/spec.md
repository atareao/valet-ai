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
