# db/repos Specification

## Purpose
Especificación de los repositorios de datos de Valet: mensajes con presupuesto de tokens y paginación, ajustes, y agregación de métricas de uso de LLM (resumen, por modelo, series diarias, herramientas, tamaños de tabla, export CSV y purga por retención).

## Requirements

### Requirement: MessagesRepo::list_by_token_budget SHALL select messages by token budget

El repositorio SHALL devolver los mensajes más recientes cuya suma de tokens no supere el presupuesto indicado, en orden cronológico ascendente.

**Given** una conversación con mensajes almacenados en DB  
**When** se llama `MessagesRepo::list_by_token_budget(conn, conversation_id, max_tokens)`  
**Then** devuelve los mensajes más recientes cuya suma acumulada de `tokens_count`
(o `collapsed_tokens_count` si existe) no supere `max_tokens`  
**And** los mensajes se devuelven en orden cronológico ascendente

#### Scenario: Selecciona mensajes dentro del presupuesto
**Given** una conversación con 3 mensajes de 1000, 2000 y 1000 tokens respectivamente  
**When** `list_by_token_budget(conn, conv_id, 3500)`  
**Then** devuelve los 2 mensajes más recientes (2000 + 1000 = 3000 ≤ 3500)  
**And** el mensaje más antiguo (1000) NO se incluye

#### Scenario: Presupuesto suficiente para todos los mensajes
**Given** una conversación con 2 mensajes de 500 tokens cada uno  
**When** `list_by_token_budget(conn, conv_id, 2000)`  
**Then** devuelve ambos mensajes

#### Scenario: Usa collapsed_tokens_count cuando existe
**Given** un mensaje con `tokens_count: 3000` y `collapsed_tokens_count: 200`  
**When** `list_by_token_budget(conn, conv_id, 500)`  
**Then** el mensaje colapsado se incluye (200 ≤ 500)  
**And** el contenido devuelto es `collapsed_content`

#### Scenario: Presupuesto cero devuelve lista vacía
**Given** una conversación con mensajes  
**When** `list_by_token_budget(conn, conv_id, 0)`  
**Then** devuelve una lista vacía

#### Scenario: Conversación sin mensajes
**Given** una conversación sin mensajes
**When** `list_by_token_budget(conn, conv_id, 10000)`
**Then** devuelve una lista vacía

### Requirement: MessagesRepo::list_by_conversation SHALL use configurable page size

**Given** una conversación con mensajes en DB
**When** se llama `MessagesRepo::list_by_conversation(conn, conv_id, limit, cursor)`
**Then** el límite SHALL estar clampado entre 1 y 100
**And** el límite por defecto desde el handler SHALL venir del setting `message_page_size`

#### Scenario: Límite respeta clamp máximo
**Given** limit = 200
**When** se llama `list_by_conversation(conn, conv_id, 200, None)`
**Then** el clamp SHALL limitar a 100

#### Scenario: Límite respeta clamp mínimo
**Given** limit = 0
**When** se llama `list_by_conversation(conn, conv_id, 0, None)`
**Then** el clamp SHALL limitar a 1

### Requirement: SettingsRepo SHALL seed message_page_size default

La migración SHALL sembrar el setting `message_page_size` con el valor por defecto `50`.

**Given** una base de datos recién migrada
**When** se consulta `SettingsRepo::get(conn, "message_page_size")`
**Then** devuelve `Some("50")`

#### Scenario: Setting por defecto disponible tras migrar
**Given** una base de datos recién migrada
**When** se consulta `SettingsRepo::get(conn, "message_page_size")`
**Then** devuelve `Some("50")`

### Requirement: list_recent SHALL be removed

**Given** el código base actual
**When** se busca `list_recent` en `MessagesRepo`
**Then** la función SHALL haber sido eliminada
**And** sus tests SHALL haber sido eliminados

#### Scenario: list_recent ya no existe
**Given** el código base actual
**When** se busca `list_recent` en `MessagesRepo`
**Then** no existe la función ni sus tests

### Requirement: StatsRepo SHALL provide LLM usage aggregation queries

El repositorio SHALL exponer `StatsRepo::summary(pool)` devolviendo un `StatsSummary` con los totales de llamadas, tokens, coste y errores de las peticiones **de chat**. Las filas con `kind != 'chat'` (router, archivist, consolidator, collapse) SHALL excluirse.

**Given** una tabla `llm_requests` con datos poblados
**When** se llama a `StatsRepo::summary(pool)`
**Then** devuelve un `StatsSummary` con:
- `total_calls: u64` — número de llamadas de chat
- `total_prompt_tokens: u64` — suma de prompt_tokens
- `total_completion_tokens: u64` — suma de completion_tokens
- `total_tokens: u64` — suma de total_tokens
- `total_cached_tokens: u64` — suma de cached_tokens
- `total_reasoning_tokens: u64` — suma de reasoning_tokens
- `total_cost: f64` — suma de cost
- `total_errors: u64` — llamadas con status != 'success'
- `avg_duration_ms: Option<f64>` — media de duration_ms (None si no hay datos)
**And** las filas con `kind != 'chat'` no se cuentan.

#### Scenario: Summary con datos variados
**Given** 3 llamadas de chat y 1 llamada de otro origen (`kind='archivist'`):
- éxito: prompt=100, completion=50, total=150, cached=10, reasoning=5, cost=0.01
- éxito: prompt=200, completion=100, total=300, cached=20, reasoning=15, cost=0.02
- error: prompt=0, completion=0, total=0, cached=0, reasoning=0, cost=0.0
**When** `StatsRepo::summary(pool)`
**Then** total_calls=3, total_prompt_tokens=300, total_completion_tokens=150, total_tokens=450, total_cached_tokens=30, total_reasoning_tokens=20, total_cost=0.03, total_errors=1
**And** la llamada del archivist no altera ningún total

#### Scenario: Summary sin datos
**Given** tabla `llm_requests` vacía
**When** `StatsRepo::summary(pool)`
**Then** total_calls=0, total_cost=0.0, total_errors=0, avg_duration_ms=None

### Requirement: StatsRepo SHALL provide per-model breakdown

El repositorio SHALL exponer `StatsRepo::by_model(pool, days)` devolviendo un `ModelStats` por modelo, **agregando todos los orígenes** y ordenado por coste descendente. El filtro `WHERE kind = 'chat'` SHALL retirarse: un modelo que solo trabaja en roles de fondo SHALL aparecer con su coste total. El parámetro `days` SHALL acotar la ventana a
`created_at >= datetime('now', '-' || ?1 || ' days')`. `ModelStats` conserva sus campos actuales.

**Given** una tabla `llm_requests` con datos de múltiples modelos y orígenes
**When** se llama a `StatsRepo::by_model(pool, days)`
**Then** devuelve `Vec<ModelStats>` con un elemento por modelo, cada uno con:
- `model: String`
- `calls: u64`, `total_tokens: u64`, `total_cost: f64`, `avg_duration_ms: Option<f64>`, `total_cached_tokens: u64`, `total_reasoning_tokens: u64`
**And** las llamadas de chat y las de los orígenes de fondo se agregan juntas por modelo
**And** las llamadas fuera de la ventana `days` no se cuentan.

#### Scenario: Dos modelos con datos
**Given** 2 llamadas de chat a "gpt-4o" y 1 de chat a "claude-3", todas dentro de la ventana
**When** `StatsRepo::by_model(pool, 7)`
**Then** devuelve 2 filas ordenadas por coste descendente
**And** cada fila agrega las llamadas de ese modelo

#### Scenario: Un modelo que solo trabaja en un rol de fondo aparece
**Given** 3 llamadas de consolidación a "qwen/qwen3-235b-a22b-2507" y ninguna llamada de chat a ese modelo
**When** `StatsRepo::by_model(pool, 7)`
**Then** el resultado incluye una fila para "qwen/qwen3-235b-a22b-2507"
**And** su `total_cost` agrega las llamadas de todos los orígenes

#### Scenario: Un modelo del archivist se agrega con los de chat
**Given** 1 llamada de chat a "mistralai/mistral-small-24b-instruct-2501" y 1 del archivist al mismo modelo
**When** `StatsRepo::by_model(pool, 7)`
**Then** devuelve una sola fila para "mistralai/mistral-small-24b-instruct-2501" con `calls=2`

#### Scenario: La ventana descarta las llamadas antiguas
**Given** 3 llamadas a "gpt-4o", 2 dentro de los últimos 7 días y 1 de hace 30 días
**When** `StatsRepo::by_model(pool, 7)`
**Then** la fila de "gpt-4o" tiene `calls=2`

### Requirement: StatsRepo SHALL provide daily time series

El repositorio SHALL exponer `StatsRepo::by_day(pool, days)` devolviendo un `DayStats` por día **de chat**, ordenado por fecha ascendente. Las filas con `kind != 'chat'` SHALL excluirse.

**Given** una tabla `llm_requests` con datos de varios días
**When** se llama a `StatsRepo::by_day(pool, days)`
**Then** devuelve `Vec<DayStats>` con un elemento por día, cada uno con:
- `date: String` (formato YYYY-MM-DD)
- `calls: u64`, `total_tokens: u64`, `total_cost: f64`, `total_cached_tokens: u64`, `total_reasoning_tokens: u64`

#### Scenario: Datos de 7 días
**Given** llamadas de chat distribuidas en 7 días y llamadas de otros orígenes en esos mismos días
**When** `StatsRepo::by_day(pool, 30)`
**Then** devuelve 7 filas ordenadas por fecha ascendente
**And** las llamadas de otros orígenes no se cuentan en ningún día

### Requirement: StatsRepo SHALL provide tool call frequency

El repositorio SHALL exponer `StatsRepo::tools_summary(pool)` devolviendo la frecuencia de uso de cada herramienta en las peticiones **de chat**. Las filas con `kind != 'chat'` SHALL excluirse.

**Given** una tabla `llm_requests` con tool_calls poblados
**When** se llama a `StatsRepo::tools_summary(pool)`
**Then** devuelve `Vec<ToolStats>` con cada tool y su frecuencia de uso

#### Scenario: Tools más usadas
**Given** 5 llamadas de chat: 3 con tool_calls '["get_weather"]', 2 con '["search_web"]'
**When** `StatsRepo::tools_summary(pool)`
**Then** get_weather: 3, search_web: 2

### Requirement: StatsRepo SHALL provide database table sizes

El repositorio SHALL exponer `StatsRepo::db_sizes(pool)` devolviendo el número de filas de cada tabla.

**Given** una base de datos con tablas pobladas
**When** se llama a `StatsRepo::db_sizes(pool)`
**Then** devuelve `Vec<TableSize>` con nombre de tabla y row count para:
events, llm_requests, memory, message_embeddings, messages, notes, profiles, reminders, settings, tasks, timeline_events, tools

#### Scenario: Tablas con datos
**Given** 10 messages, 2 profiles, 5 memories
**When** `StatsRepo::db_sizes(pool)`
**Then** messages=10, profiles=2, memories=5, resto=0

#### Scenario: El timeline sale en el recuento de tablas
**Given** una base de datos con 7 hechos en `timeline_events`
**When** `StatsRepo::db_sizes(pool)`
**Then** el resultado incluye una entrada con `table = "timeline_events"` y `rows = 7`

#### Scenario: Una tabla del timeline vacía también aparece
**Given** una base de datos recién migrada, con `timeline_events` sin filas
**When** `StatsRepo::db_sizes(pool)`
**Then** el resultado incluye `table = "timeline_events"` con `rows = 0`

### Requirement: StatsRepo SHALL provide CSV export with all OpenRouter fields

El repositorio SHALL exponer `StatsRepo::export_csv(pool)` devolviendo un CSV con todas las columnas de `llm_requests`, incluida `kind`.

**Given** una tabla `llm_requests` con datos
**When** se llama a `StatsRepo::export_csv(pool)`
**Then** devuelve un String con formato CSV con cabeceras:
`id,model,provider,profile_id,prompt_tokens,completion_tokens,total_tokens,cached_tokens,reasoning_tokens,cost,is_byok,duration_ms,cache_hit,status,error_message,tool_calls,kind,created_at`

#### Scenario: Export CSV con datos
**Given** 2 llamadas en llm_requests
**When** `StatsRepo::export_csv(pool)`
**Then** el CSV tiene 1 línea de cabecera + 2 líneas de datos
**And** cada línea incluye cost, cached_tokens, reasoning_tokens y kind

### Requirement: StatsRepo SHALL purge data older than retention period

El repositorio SHALL exponer `StatsRepo::purge_old(pool, days)` eliminando los registros anteriores al periodo de retención y devolviendo cuántos borró.

**Given** una tabla `llm_requests` con datos de 60 días
**When** se llama a `StatsRepo::purge_old(pool, 30)`
**Then** borra todos los registros con created_at anterior a hace 30 días
**And** devuelve el número de registros eliminados

#### Scenario: Purga con datos mixtos
**Given** 10 registros de hace 45 días y 10 de hace 15 días
**When** `StatsRepo::purge_old(pool, 30)`
**Then** elimina 10 registros (los de 45 días)
**And** devuelve 10

### Requirement: StatsRepo SHALL record LLM requests on each chat call

`StatsRepo::record_request(pool, kind, id, model, profile_id, …)` SHALL insertar una fila en `llm_requests` con el `kind` indicado (`CallKind`). El chat pasa `CallKind::Chat`; el archivist, el consolidator y el collapse pasan el suyo; la llamada al clasificador la graba el orquestador con `CallKind::Router`. El `kind` SHALL ser uno de los admitidos por el `CHECK` de la columna.

#### Scenario: inserts row with all fields
- **WHEN** `StatsRepo::record_request(pool, CallKind::Chat, "req-1", "gpt-4o", "profile-1", 100, 50, 150, 0, 0, 0.0, Some(200), "success", None, None)` is called
- **THEN** the table contains 1 row with matching field values and `kind='chat'`
- **AND** created_at is NOT NULL

#### Scenario: with error status
- **WHEN** `record_request` is called with status "error" and error_message Some("timeout")
- **THEN** the inserted row has status = "error"
- **AND** error_message = "timeout"

#### Scenario: auto-assigns created_at
- **WHEN** `record_request` is called with created_at = None
- **THEN** created_at is NOT NULL (database assigned datetime('now'))

#### Scenario: records the origin kind
- **WHEN** `record_request` is called with `CallKind::Collapse`
- **THEN** the inserted row has `kind = 'collapse'`

### Requirement: search_by_vector SHALL resolver la búsqueda KNN dentro de SQLite con vec0

`MemoryRepo::search_by_vector` SHALL resolver la búsqueda dentro de SQLite con `MATCH … AND k = MEMORY_KNN_CANDIDATES ORDER BY distance` sobre la tabla virtual `vec_memory`, haciendo JOIN con `memory` para recuperar la ficha (`memory m JOIN vec_memory v ON m.id = v.id`). SHALL NOT cargar todas las filas ni parsear el embedding como JSON ni calcular el coseno en Rust. `memory` SHALL seguir siendo la fuente de verdad.

**Given** una tabla virtual `vec_memory` y una consulta con su embedding  
**When** se ejecuta `search_by_vector`  
**Then** SHALL usar `MATCH … AND k = MEMORY_KNN_CANDIDATES ORDER BY distance`  
**And** SHALL recuperar los campos de `memory` mediante el JOIN por id  
**And** SHALL NOT parsear el embedding almacenado como JSON  
**And** SHALL NOT calcular el coseno en Rust

#### Scenario: La búsqueda se resuelve en SQLite
**Given** filas en `memory` y sus vectores en `vec_memory`  
**When** se ejecuta `search_by_vector`  
**Then** las candidatas vuelven ordenadas por `distance` ascendente  
**And** cada resultado incluye el `id`, `content` y `tokens_count` de `memory` junto con su `distance`

#### Scenario: La deriva de dimensión es imposible
**Given** `vec_memory USING vec0(id TEXT PRIMARY KEY, embedding float[1024] distance_metric=cosine)`  
**When** se intenta almacenar un vector de 1536 dims  
**Then** la operación falla por el esquema `vec0`  
**And** no queda ningún embedding inbuscable

### Requirement: Los mandos de memoria SHALL vivir en settings y leerse en cada consulta

Los cuatro mandos de la memoria episódica SHALL vivir en la tabla `settings` (como los prompts) y SHALL leerse en cada consulta, de modo que cambiarlos surta efecto sin reiniciar. Los defaults iniciales SHALL ser `MEMORY_HALF_LIFE_DAYS = 90`, `SIMILARITY_THRESHOLD = 0.5` (provisional), `RAG_BUDGET_TOKENS = 800` y `MEMORY_KNN_CANDIDATES = 20`. `MEMORY_KNN_CANDIDATES` SHALL acotar el número de candidatas de la KNN, no lo que entra al prompt (eso lo determina `RAG_BUDGET_TOKENS`), y SHALL ser mayor que lo que el presupuesto admite para dar margen al reordenado por antigüedad.

| clave | qué hace | inicial |
|---|---|---|
| `MEMORY_HALF_LIFE_DAYS` | cuánto pesa la antigüedad | 90 |
| `SIMILARITY_THRESHOLD` | qué se descarta por irrelevante | 0,5 (provisional) |
| `RAG_BUDGET_TOKENS` | cuánto ocupa la memoria en el prompt | 800 |
| `MEMORY_KNN_CANDIDATES` | cuántas candidatas se traen antes de filtrar | 20 |

**Given** una base de datos migrada  
**When** se leen los mandos de memoria  
**Then** `settings` SHALL contener `MEMORY_HALF_LIFE_DAYS`, `SIMILARITY_THRESHOLD`, `RAG_BUDGET_TOKENS` y `MEMORY_KNN_CANDIDATES` con sus valores iniciales  
**And** SHALL respetarse cualquier valor no vacío ya existente  
**And** los mandos SHALL leerse en cada consulta, no solo al arrancar

#### Scenario: Los cuatro mandos se siembran con sus defaults
**Given** una base de datos recién migrada  
**When** se consulta `settings`  
**Then** `MEMORY_HALF_LIFE_DAYS = 90`, `SIMILARITY_THRESHOLD = 0.5`, `RAG_BUDGET_TOKENS = 800` y `MEMORY_KNN_CANDIDATES = 20`

#### Scenario: Un cambio en caliente surte efecto sin reiniciar
**Given** una consulta que devuelve resultado con `SIMILARITY_THRESHOLD = 0.5`  
**When** se actualiza `settings.SIMILARITY_THRESHOLD` a un valor más alto y se repite la consulta sin reiniciar  
**Then** el resultado refleja el nuevo umbral

#### Scenario: La personalización existente se respeta
**Given** `settings.RAG_BUDGET_TOKENS = 1200` antes de migrar  
**When** se ejecuta la migración  
**Then** `RAG_BUDGET_TOKENS` sigue siendo `1200`

### Requirement: Los parámetros de generación SHALL vivir en settings y leerse en cada llamada

Los parámetros de generación de cada rol SHALL vivir en la tabla `settings` (como los prompts y los
mandos de memoria) y SHALL leerse **en cada llamada** al LLM, de modo que cambiarlos surta efecto
sin reiniciar. Habrá cinco roles —chat, colapso, fichas, consolidador/compresión y timeline— y tres
claves por rol: temperatura, razonamiento y tokens máximos. Una migración SHALL sembrarlas con sus
valores iniciales, respetando cualquier valor ya existente (solo rellena si la clave falta o está
vacía).

| clave | rol | qué es | inicial |
|---|---|---|---|
| `GENERATION_CHAT_TEMPERATURE` | chat | temperatura | `0.7` |
| `GENERATION_CHAT_REASONING` | chat | razonamiento | `` (vacío = default del modelo) |
| `GENERATION_CHAT_MAX_TOKENS` | chat | tokens máximos | `4096` |
| `GENERATION_COLLAPSE_TEMPERATURE` | colapso | temperatura | `0.2` |
| `GENERATION_COLLAPSE_REASONING` | colapso | razonamiento | `off` |
| `GENERATION_COLLAPSE_MAX_TOKENS` | colapso | tokens máximos | `1024` |
| `GENERATION_MEMORY_TEMPERATURE` | fichas | temperatura | `0.3` |
| `GENERATION_MEMORY_REASONING` | fichas | razonamiento | `off` |
| `GENERATION_MEMORY_MAX_TOKENS` | fichas | tokens máximos | `1024` |
| `GENERATION_SEMANTIC_TEMPERATURE` | consolidador/compresión | temperatura | `0.1` |
| `GENERATION_SEMANTIC_REASONING` | consolidador/compresión | razonamiento | `off` |
| `GENERATION_SEMANTIC_MAX_TOKENS` | consolidador/compresión | tokens máximos | `2048` |
| `GENERATION_TIMELINE_TEMPERATURE` | timeline | temperatura | `0.2` |
| `GENERATION_TIMELINE_REASONING` | timeline | razonamiento | `off` |
| `GENERATION_TIMELINE_MAX_TOKENS` | timeline | tokens máximos | `2048` |

El campo de razonamiento SHALL codificarse como cadena: vacío ⇒ no se envía el campo `reasoning`
(el modelo decide); `off` ⇒ `ReasoningSpec::Off`; cualquier otro valor ⇒
`ReasoningSpec::Effort(<nivel>)`. Una temperatura o un `max_tokens` no parseables SHALL caer al
default del rol con un warning; un nivel de razonamiento desconocido SHALL caer a `off` con un
warning.

**Given** una base de datos migrada  
**When** se leen las claves de generación  
**Then** `settings` SHALL contener las quince claves con sus valores iniciales  
**And** cualquier valor no vacío ya existente SHALL respetarse

**Given** un rol y sus claves en `settings`  
**When** una llamada al LLM de ese rol construye su `ChatRequest`  
**Then** SHALL leer las claves en ese momento, no al arrancar

#### Scenario: Las doce claves se siembran con sus defaults
**Given** una base de datos recién migrada  
**When** se consulta `settings`  
**Then** existe `GENERATION_CHAT_TEMPERATURE = 0.7`  
**And** existe `GENERATION_SEMANTIC_REASONING = off`  
**And** existe `GENERATION_SEMANTIC_MAX_TOKENS = 2048`  
**And** existen las nueve claves restantes con sus valores iniciales

#### Scenario: Las tres claves del timeline se siembran con los defaults de su rol
**Given** una base de datos migrada  
**When** se consultan las claves del rol timeline  
**Then** `GENERATION_TIMELINE_TEMPERATURE` es `0.2`  
**And** `GENERATION_TIMELINE_REASONING` es `off`  
**And** `GENERATION_TIMELINE_MAX_TOKENS` es `2048`

#### Scenario: Un valor existente se respeta
**Given** `GENERATION_CHAT_TEMPERATURE = 0.9` antes de migrar  
**When** se ejecuta la migración  
**Then** `GENERATION_CHAT_TEMPERATURE` sigue siendo `0.9`

#### Scenario: Un valor de timeline personalizado no se sobrescribe
**Given** `GENERATION_TIMELINE_REASONING = low` antes de migrar  
**When** se ejecuta la migración de defaults del timeline  
**Then** `GENERATION_TIMELINE_REASONING` sigue siendo `low`

#### Scenario: Un cambio en caliente surte efecto sin reiniciar
**Given** `GENERATION_COLLAPSE_TEMPERATURE = 0.2`  
**When** se actualiza a `0.5` y se repite una llamada de colapso sin reiniciar  
**Then** el `temperature` del `ChatRequest` es `0.5`

#### Scenario: Una temperatura no parseable cae al default con warning
**Given** `GENERATION_MEMORY_TEMPERATURE = "alta"`  
**When** una llamada de fichas lee sus parámetros  
**Then** la temperatura es `0.3`  
**And** se registra un warning

#### Scenario: Un razonamiento desconocido cae a off con warning
**Given** `GENERATION_COLLAPSE_REASONING = "super"`  
**When** una llamada de colapso lee sus parámetros  
**Then** el razonamiento es `Off`  
**And** se registra un warning

#### Scenario: El razonamiento vacío no se envía
**Given** `GENERATION_CHAT_REASONING = ""`  
**When** una llamada del chat lee sus parámetros  
**Then** el `reasoning` del `ChatRequest` es `None`

#### Scenario: Un `low` heredado del consolidador se corrige a `off`
**Given** una base migrada con `GENERATION_SEMANTIC_REASONING = low`  
**When** se aplica la migración de endurecimiento del consolidador  
**Then** `GENERATION_SEMANTIC_REASONING` pasa a `off`

#### Scenario: Un razonamiento del consolidador distinto de `low` se respeta
**Given** una base migrada con `GENERATION_SEMANTIC_REASONING = medium`  
**When** se aplica la migración de endurecimiento del consolidador  
**Then** `GENERATION_SEMANTIC_REASONING` sigue siendo `medium`

### Requirement: ProfilesRepo::get_by_id SHALL return the profile or None

El repositorio SHALL exponer `ProfilesRepo::get_by_id(pool, id)` que devuelve el perfil cuyo `id`
coincide, o `None` si no existe. NO SHALL insertar ni modificar filas.

#### Scenario: Perfil existente
**Given** la tabla `profiles` con una fila `id = "p1"` y `name = "Lorenzo"`  
**When** se llama `ProfilesRepo::get_by_id(pool, "p1")`  
**Then** devuelve `Some(profile)` con `name = "Lorenzo"`

#### Scenario: Perfil inexistente
**Given** la tabla `profiles` sin la fila `id = "nope"`  
**When** se llama `ProfilesRepo::get_by_id(pool, "nope")`  
**Then** devuelve `None`  
**And** no se inserta ninguna fila

### Requirement: ToolsRepo SHALL reconciliar la tabla tools con el registry

La tabla `tools` SHALL reconciliarse con el catálogo de herramientas registradas: SHALL insertar las
herramientas registradas ausentes en la tabla con la descripción del registry y `enabled = 1`, SHALL
eliminar las filas cuyo nombre ya no corresponde a ninguna herramienta registrada, y SHALL preservar
el valor de `enabled` de las filas existentes actualizando solo su descripción. La operación SHALL ser
idempotente.

#### Scenario: Alta de herramientas nuevas

**Given** una tabla `tools` con `calendar` y `weather`
**When** se reconcilia con un registry que además contiene `notes` y `unified_search`
**Then** `notes` y `unified_search` quedan insertadas con `enabled = 1`

#### Scenario: Baja de herramientas obsoletas

**Given** una tabla `tools` que contiene la fila `geo`, que no está en el registry
**When** se reconcilia con el registry
**Then** la fila `geo` se elimina

#### Scenario: Se preserva el estado habilitado

**Given** una tabla `tools` con `weather` y `enabled = 0`
**When** se reconcilia con el registry
**Then** `weather` sigue con `enabled = 0`
**And** su descripción se actualiza con la del registry

#### Scenario: La reconciliación es idempotente

**Given** una tabla `tools` ya reconciliada con el registry
**When** se reconcilia de nuevo
**Then** el contenido de la tabla no cambia

### Requirement: RemindersRepo SHALL provide CRUD for reminders

`RemindersRepo` SHALL exponer `create`, `find_by_id`, `list`, `dismiss`, `snooze` y `delete` sobre la
tabla `reminders`. `list` SHALL aceptar `profile_id` y un `status` opcional; `dismiss` y `snooze` SHALL
actualizar el estado y/o el `datetime`.

**Given** un pool SQLite
**When** se invoca `RemindersRepo`
**Then** DEBE operar sobre la tabla `reminders` y devolver `Reminder`/`Option<Reminder>`

#### Scenario: create inserta un recordatorio
**Given** un `Reminder` con id, profile_id, text, datetime y status
**When** se llama a `RemindersRepo::create`
**Then** DEBE insertarse la fila correspondiente

#### Scenario: list filtra por estado opcional
**Given** recordatorios en estados distintos
**When** se llama a `RemindersRepo::list` con `status = "pending"`
**Then** DEBE devolver solo los `pending`

#### Scenario: dismiss y snooze actualizan el estado
**Given** un recordatorio existente
**When** se llama a `RemindersRepo::dismiss` o `snooze`
**Then** DEBE actualizarse su `status` (`dismissed`/`snoozed`) y, en `snooze`, su `datetime`

### Requirement: TasksRepo SHALL provide CRUD for tasks

`TasksRepo` SHALL exponer `create`, `find_by_id`, `list`, `update`, `complete`, `cancel` y `delete`
sobre la tabla `tasks`. `list` SHALL aceptar filtros opcionales (`status`, `priority`, `project`,
`scope`) además de `profile_id`.

**Given** un pool SQLite
**When** se invoca `TasksRepo`
**Then** DEBE operar sobre la tabla `tasks` y devolver `Task`/`Option<Task>`

#### Scenario: create inserta una tarea
**Given** un `Task` con los campos requeridos
**When** se llama a `TasksRepo::create`
**Then** DEBE insertarse la fila

#### Scenario: complete marca la tarea como done
**Given** una tarea existente
**When** se llama a `TasksRepo::complete`
**Then** DEBE actualizarse su `status` a `done`

#### Scenario: update modifica campos seleccionados
**Given** una tarea existente
**When** se llama a `TasksRepo::update` con nuevos valores
**Then** DEBE persistir los cambios y actualizar `updated_at`

#### Scenario: delete elimina la tarea
**Given** una tarea existente
**When** se llama a `TasksRepo::delete`
**Then** DEBE eliminarse la fila

### Requirement: NotesRepo SHALL provide CRUD for notes

`NotesRepo` SHALL exponer `create`, `find_by_id`, `list`, `update` y `delete` sobre la tabla `notes`.
`list` SHALL aceptar `profile_id` y una `category` opcional.

**Given** un pool SQLite
**When** se invoca `NotesRepo`
**Then** DEBE operar sobre la tabla `notes` y devolver `Note`/`Option<Note>`

#### Scenario: create inserta una nota
**Given** una `Note` con content y category
**When** se llama a `NotesRepo::create`
**Then** DEBE insertarse la fila

#### Scenario: list filtra por categoría opcional
**Given** notas de varias categorías
**When** se llama a `NotesRepo::list` con una `category`
**Then** DEBE devolver solo las de esa categoría

#### Scenario: delete elimina la nota
**Given** una nota existente
**When** se llama a `NotesRepo::delete`
**Then** DEBE eliminarse la fila

### Requirement: StatsRepo SHALL provide background LLM usage aggregation

El repositorio SHALL exponer `StatsRepo::background_summary(pool, days)` devolviendo un `BackgroundStats` por cada **pareja (origen, modelo)** de los orígenes no-chat (`router`, `archivist`, `consolidator`, `collapse`, `timeline`, `journal`). Los orígenes canónicos sin ninguna fila SHALL aparecer con los contadores a cero y `model: null`. El parámetro `days` SHALL acotar la ventana a `created_at >= datetime('now', '-' || ?1 || ' days')`. Una ventana menor que un día SHALL tratarse como un día, de modo que `days = 0` signifique las últimas 24 horas y no una ventana vacía. Las filas SHALL ordenarse por `kind` canónico y, dentro de cada `kind`, por coste descendente.

**Given** una tabla `llm_requests` con filas de varios orígenes y modelos
**When** se llama a `StatsRepo::background_summary(pool, days)`
**Then** devuelve un `BackgroundStats` por pareja (origen, modelo), cada uno con:
- `kind: String`
- `model: Option<String>` — `None` cuando la pareja es un origen sin filas
- `calls: u64`
- `input_tokens: u64`, `output_tokens: u64`, `total_tokens: u64`
- `total_cost: f64`
- `total_errors: u64` — llamadas con status != 'success'
- `suspicious_calls: u64` — llamadas con `status = 'success'` y `completion_tokens <= 1`
- `avg_duration_ms: Option<f64>`
- `last_call_at: Option<String>` — `MAX(created_at)` de la pareja; `None` si no hay filas
**And** las filas de chat no se cuentan
**And** las parejas fuera de la ventana `days` no se cuentan.

#### Scenario: Una entrada por origen
**Given** 2 llamadas `router` al modelo "A", 1 `collapse` al modelo "B" y ninguna `archivist`, `consolidator`, `timeline` ni `journal`
**When** `StatsRepo::background_summary(pool, 7)`
**Then** devuelve una fila por pareja (origen, modelo) más una fila por cada origen canónico sin filas
**And** `router` tiene `calls=2`, `collapse` tiene `calls=1`, y `archivist`, `consolidator`, `timeline` y `journal` tienen `calls=0` con `model=null`

#### Scenario: El extractor de timeline se cuenta como un origen más
**Given** 3 llamadas con `kind='timeline'` al mismo modelo, una de ellas con `status='error'`
**When** `StatsRepo::background_summary(pool, 7)`
**Then** la pareja de `timeline` tiene `calls=3` y `total_errors=1`
**And** sus tokens y su coste se suman igual que en los demás orígenes

#### Scenario: Sin procesos de fondo
**Given** una tabla `llm_requests` solo con filas de chat
**When** `StatsRepo::background_summary(pool, 7)`
**Then** los seis orígenes canónicos tienen `calls=0`, `model=null` y `total_cost=0.0`

#### Scenario: Varios modelos dentro del mismo origen
**Given** 2 llamadas `consolidator` al modelo "deepseek/deepseek-v4.1-flash" y 3 al modelo "qwen/qwen3-235b-a22b-2507"
**When** `StatsRepo::background_summary(pool, 7)`
**Then** el origen `consolidator` aparece en dos filas, una por modelo, cada una con sus cifras
**And** la fila de mayor coste aparece antes dentro de `consolidator`

#### Scenario: Una respuesta vacía exitosa se marca como sospechosa
**Given** 4 llamadas `consolidator` con `status='success'` y `completion_tokens=1` y 2 llamadas `consolidator` con `status='success'` y 654 tokens
**When** `StatsRepo::background_summary(pool, 7)`
**Then** la pareja de `consolidator` tiene `suspicious_calls=4`
**And** `total_errors=0` (la avería no se ve en la columna de errores)

#### Scenario: Una llamada con error no cuenta como sospechosa
**Given** una llamada `consolidator` con `status='error'` y `completion_tokens=1`
**When** `StatsRepo::background_summary(pool, 7)`
**Then** esa llamada cuenta en `total_errors` pero NO en `suspicious_calls`

#### Scenario: La última llamada de la pareja
**Given** dos llamadas `router` al modelo "A" con `created_at` distinto
**When** `StatsRepo::background_summary(pool, 7)`
**Then** `last_call_at` de la pareja es la fecha mayor de las dos

#### Scenario: La bitácora se cuenta como un origen más
**Given** 3 llamadas con `kind='journal'` al mismo modelo, dos de ellas con `status='error'`
**When** `StatsRepo::background_summary(pool, 7)`
**Then** el origen `journal` aparece entre las parejas
**And** tiene `calls=3` y `total_errors=2`
**And** sus tokens y su coste se suman igual que en los demás orígenes

### Requirement: StatsRepo SHALL flag suspiciously empty successful calls

`BackgroundStats` SHALL exponer `suspicious_calls: u64`, el recuento de llamadas con
`status = 'success'` **y** `completion_tokens <= 1`. La condición SHALL aplicar **solo** a los
orígenes estructurados canónicos (`router`, `archivist`, `consolidator`, `collapse`, `timeline`,
`journal`) y **SHALL NOT** aplicar a `chat`, donde una respuesta de un token puede ser legítima. El
umbral SHALL vivir en una constante con nombre (no un literal suelto), para que su valor quede
documentado y cambiable en un solo sitio.

La razón del umbral y del filtro por `status` está en el `design.md`: un contenido vacío no es un
error de transporte, así que la avería —respuestas con `completion_tokens = 1` y `status = 'success'`
— es invisible para la columna de errores; `suspicious_calls` es la que la habría cazado el primer
día.

#### Scenario: Un éxito con salida vacía suma en sospechosas
**Given** una llamada a un origen estructurado con `status='success'` y `completion_tokens=1`
**When** se agrega ese origen
**Then** `suspicious_calls` se incrementa en uno
**And** `total_errors` no cambia

#### Scenario: Un éxito con salida normal no suma en sospechosas
**Given** una llamada a un origen estructurado con `status='success'` y `completion_tokens > 1`
**When** se agrega ese origen
**Then** `suspicious_calls` no se incrementa

#### Scenario: El chat nunca cuenta como sospechoso
**Given** una llamada de chat con `status='success'` y `completion_tokens=1`
**When** se agrega la estadística
**Then** esa llamada no cuenta como sospechosa

#### Scenario: La bitácora es un origen estructurado más
**Given** una llamada `journal` con `status='success'` y `completion_tokens=1`
**When** se agrega ese origen
**Then** `suspicious_calls` del origen `journal` se incrementa en uno

### Requirement: El modelo `JournalEntry` y `Mood` SHALL definir el contrato de la bitácora

El modelo SHALL exponer `JournalEntry { id, profile_id, date, mood: Option<Mood>, content,
achievements: Vec<String>, struggles: Vec<String>, tags: Vec<String>, created_at, updated_at }` y
`enum Mood { Great, Good, Neutral, Bad, Terrible }`. `Mood` SHALL serializarse y deserializarse en
`snake_case` (`great`, `good`, `neutral`, `bad`, `terrible`) y SHALL ser **estricto**: un valor fuera
del conjunto NO SHALL ser válido, ni se inventa un ánimo ni se traga el dato en silencio. El ánimo
SHALL ser opcional: una entrada sin `mood` SHALL ser válida. Los campos `achievements`, `struggles` y
`tags` SHALL exponerse como listas de cadenas; un valor `NULL` en la base de datos SHALL leerse como
lista vacía `[]` y una lista vacía SHALL persistirse sin ambigüedad (`[]` o `NULL`).

**Contracts:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood { Great, Good, Neutral, Bad, Terrible }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub profile_id: String,
    pub date: String,               // ISO YYYY-MM-DD, en la zona horaria del usuario
    pub mood: Option<Mood>,
    pub content: String,
    pub achievements: Vec<String>,  // JSON array o NULL en la base
    pub struggles: Vec<String>,     // JSON array o NULL en la base
    pub tags: Vec<String>,          // JSON array o NULL en la base
    pub created_at: String,
    pub updated_at: String,
}
```

#### Scenario: El ánimo se serializa en snake_case

- **Given** un `Mood::Great`
- **When** se serializa a JSON
- **Then** el valor es `"great"`
- **And** los cinco ánimos se serializan como `great`, `good`, `neutral`, `bad` y `terrible`

#### Scenario: Un ánimo fuera del conjunto no es válido

- **Given** el valor `"genial"` para el campo `mood`
- **When** se deserializa una `JournalEntry`
- **Then** la deserialización falla
- **And** no se sustituye por ningún ánimo por defecto

#### Scenario: Una entrada sin ánimo es válida

- **Given** una entrada con `mood` ausente (`NULL`)
- **When** se lee la entrada
- **Then** su `mood` es `None`
- **And** la entrada se conserva entera

#### Scenario: Los campos JSON nulos se leen como listas vacías

- **Given** una entrada con `achievements`, `struggles` y `tags` a `NULL`
- **When** se lee la entrada
- **Then** sus tres campos son listas vacías `[]`

### Requirement: `JournalRepo` SHALL exponer upsert, get, list y delete por perfil y fecha

`JournalRepo` SHALL operar sobre `journal_entries` y exponer `upsert(pool, entry) -> JournalEntry`,
`get(pool, profile_id, date) -> Option<JournalEntry>`, `list(pool, profile_id, from, to, limit) ->
Vec<JournalEntry>` y `delete(pool, profile_id, date) -> bool`. El `upsert` SHALL crear la entrada si
no existe y **reemplazar** la del mismo día si existe, sin duplicar fila y conservando una sola;
`updated_at` SHALL cambiar en el reemplazo. `get` y `delete` SHALL direccionarse por
`(profile_id, date)` —la fecha es la clave pública; el `id` es interno— y `delete` SHALL devolver
`true` si borró una fila y `false` si no había ninguna. `list` SHALL devolver las entradas del rango
**inclusivo** `[from, to]` ordenadas de más reciente a más antigua y acotadas por `limit`.

**Contracts:**

```rust
impl JournalRepo {
    pub async fn upsert(pool: &SqlitePool, entry: &JournalEntry) -> Result<JournalEntry, sqlx::Error>;
    pub async fn get(pool: &SqlitePool, profile_id: &str, date: &str) -> Result<Option<JournalEntry>, sqlx::Error>;
    pub async fn list(pool: &SqlitePool, profile_id: &str, from: &str, to: &str, limit: i64) -> Result<Vec<JournalEntry>, sqlx::Error>;
    pub async fn delete(pool: &SqlitePool, profile_id: &str, date: &str) -> Result<bool, sqlx::Error>;
}
```

#### Scenario: `upsert` crea la entrada del día

- **Given** un perfil sin entrada para `'2026-10-18'`
- **When** se invoca `upsert` con esa fecha y un relato
- **Then** queda una fila nueva en `journal_entries` con ese relato

#### Scenario: `upsert` reemplaza la entrada del mismo día sin duplicar

- **Given** una entrada existente para `'2026-10-18'` con un relato
- **When** se invoca `upsert` con la misma fecha y un relato distinto
- **Then** sigue habiendo **una sola** fila para ese perfil y día
- **And** su `content` es el nuevo

#### Scenario: El reemplazo actualiza `updated_at`

- **Given** una entrada existente con su `created_at` y `updated_at`
- **When** se invoca `upsert` con la misma fecha
- **Then** `created_at` se conserva
- **And** `updated_at` es posterior o distinto del anterior

#### Scenario: `get` devuelve la entrada del día o nada

- **Given** una entrada guardada para `'2026-10-18'`
- **When** se invoca `get` con esa fecha
- **Then** devuelve la entrada
- **When** se invoca `get` con `'2026-10-19'`
- **Then** devuelve `None`

#### Scenario: `list` devuelve el rango de más reciente a más antiguo

- **Given** entradas en `'2026-10-15'`, `'2026-10-16'` y `'2026-10-20'`
- **When** se invoca `list` con `from = '2026-10-15'` y `to = '2026-10-16'`
- **Then** devuelve las dos entradas del rango
- **And** `'2026-10-16'` aparece antes que `'2026-10-15'`
- **And** la del `'2026-10-20'` no aparece

#### Scenario: `list` respeta el tope quedándose con lo más reciente

- **Given** más entradas que el tope pedido dentro del rango
- **When** se invoca `list` con ese `limit`
- **Then** devuelve a lo sumo `limit` entradas
- **And** son las más recientes del rango

#### Scenario: `delete` borra la del día y avisa de lo que no existía

- **Given** una entrada existente para `'2026-10-18'`
- **When** se invoca `delete` con esa fecha
- **Then** devuelve `true` y la fila desaparece
- **When** se invoca `delete` con `'2026-10-19'`
- **Then** devuelve `false`
