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

El repositorio SHALL exponer `StatsRepo::summary(pool)` devolviendo un `StatsSummary` con los totales de llamadas, tokens, coste y errores.

**Given** una tabla `llm_requests` con datos poblados
**When** se llama a `StatsRepo::summary(pool)`
**Then** devuelve un `StatsSummary` con:
- `total_calls: u64` — número total de llamadas
- `total_prompt_tokens: u64` — suma de prompt_tokens
- `total_completion_tokens: u64` — suma de completion_tokens
- `total_tokens: u64` — suma de total_tokens
- `total_cached_tokens: u64` — suma de cached_tokens
- `total_reasoning_tokens: u64` — suma de reasoning_tokens
- `total_cost: f64` — suma de cost
- `total_errors: u64` — llamadas con status != 'success'
- `avg_duration_ms: Option<f64>` — media de duration_ms (None si no hay datos)

#### Scenario: Summary con datos variados
**Given** 3 llamadas:
- éxito: prompt=100, completion=50, total=150, cached=10, reasoning=5, cost=0.01
- éxito: prompt=200, completion=100, total=300, cached=20, reasoning=15, cost=0.02
- error: prompt=0, completion=0, total=0, cached=0, reasoning=0, cost=0.0
**When** `StatsRepo::summary(pool)`
**Then** total_calls=3, total_prompt_tokens=300, total_completion_tokens=150, total_tokens=450, total_cached_tokens=30, total_reasoning_tokens=20, total_cost=0.03, total_errors=1

#### Scenario: Summary sin datos
**Given** tabla `llm_requests` vacía
**When** `StatsRepo::summary(pool)`
**Then** total_calls=0, total_cost=0.0, total_errors=0, avg_duration_ms=None

### Requirement: StatsRepo SHALL provide per-model breakdown

El repositorio SHALL exponer `StatsRepo::by_model(pool)` devolviendo un `ModelStats` por modelo ordenado por coste descendente.

**Given** una tabla `llm_requests` con datos de múltiples modelos
**When** se llama a `StatsRepo::by_model(pool)`
**Then** devuelve `Vec<ModelStats>` con un elemento por modelo, cada uno con:
- `model: String`
- `calls: u64`, `total_tokens: u64`, `total_cost: f64`, `avg_duration_ms: Option<f64>`, `total_cached_tokens: u64`, `total_reasoning_tokens: u64`

#### Scenario: Dos modelos con datos
**Given** 2 llamadas a "gpt-4o" y 1 a "claude-3"
**When** `StatsRepo::by_model(pool)`
**Then** devuelve 2 filas ordenadas por coste descendente

### Requirement: StatsRepo SHALL provide daily time series

El repositorio SHALL exponer `StatsRepo::by_day(pool, days)` devolviendo un `DayStats` por día ordenado por fecha ascendente.

**Given** una tabla `llm_requests` con datos de varios días
**When** se llama a `StatsRepo::by_day(pool, days)`
**Then** devuelve `Vec<DayStats>` con un elemento por día, cada uno con:
- `date: String` (formato YYYY-MM-DD)
- `calls: u64`, `total_tokens: u64`, `total_cost: f64`, `total_cached_tokens: u64`, `total_reasoning_tokens: u64`

#### Scenario: Datos de 7 días
**Given** llamadas distribuidas en 7 días
**When** `StatsRepo::by_day(pool, 30)`
**Then** devuelve 7 filas ordenadas por fecha ascendente

### Requirement: StatsRepo SHALL provide tool call frequency

El repositorio SHALL exponer `StatsRepo::tools_summary(pool)` devolviendo la frecuencia de uso de cada herramienta.

**Given** una tabla `llm_requests` con tool_calls poblados
**When** se llama a `StatsRepo::tools_summary(pool)`
**Then** devuelve `Vec<ToolStats>` con cada tool y su frecuencia de uso

#### Scenario: Tools más usadas
**Given** 5 llamadas: 3 con tool_calls '["get_weather"]', 2 con '["search_web"]'
**When** `StatsRepo::tools_summary(pool)`
**Then** get_weather: 3, search_web: 2

### Requirement: StatsRepo SHALL provide database table sizes

El repositorio SHALL exponer `StatsRepo::db_sizes(pool)` devolviendo el número de filas de cada tabla.

**Given** una base de datos con tablas pobladas
**When** se llama a `StatsRepo::db_sizes(pool)`
**Then** devuelve `Vec<TableSize>` con nombre de tabla y row count para:
events, llm_requests, memory, message_embeddings, messages, notes, profiles, reminders, settings, tasks, tools

#### Scenario: Tablas con datos
**Given** 10 messages, 2 profiles, 5 memories
**When** `StatsRepo::db_sizes(pool)`
**Then** messages=10, profiles=2, memories=5, resto=0

### Requirement: StatsRepo SHALL provide CSV export with all OpenRouter fields

El repositorio SHALL exponer `StatsRepo::export_csv(pool)` devolviendo un CSV con todas las columnas de `llm_requests`.

**Given** una tabla `llm_requests` con datos
**When** se llama a `StatsRepo::export_csv(pool)`
**Then** devuelve un String con formato CSV con cabeceras:
`id,model,provider,prompt_tokens,completion_tokens,total_tokens,cached_tokens,reasoning_tokens,cost,is_byok,duration_ms,cache_hit,status,error_message,tool_calls,created_at`

#### Scenario: Export CSV con datos
**Given** 2 llamadas en llm_requests
**When** `StatsRepo::export_csv(pool)`
**Then** el CSV tiene 1 línea de cabecera + 2 líneas de datos
**And** cada línea incluye cost, cached_tokens, reasoning_tokens

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
StatsRepo SHALL provide `record_request` to insert LLM usage data into the `llm_requests` table.

#### Scenario: inserts row with all fields
- **WHEN** `StatsRepo::record_request(pool, "req-1", "gpt-4o", "profile-1", 100, 50, 150, 0, 0, 0.0, Some(200), "success", None, None)` is called
- **THEN** the table contains 1 row with matching field values
- **AND** created_at is NOT NULL

#### Scenario: with error status
- **WHEN** `record_request` is called with status "error" and error_message Some("timeout")
- **THEN** the inserted row has status = "error"
- **AND** error_message = "timeout"

#### Scenario: auto-assigns created_at
- **WHEN** `record_request` is called with created_at = None
- **THEN** created_at is NOT NULL (database assigned datetime('now'))

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
sin reiniciar. Habrá cuatro roles —chat, colapso, fichas y consolidador/compresión— y tres claves
por rol: temperatura, razonamiento y tokens máximos. Una migración SHALL sembrarlas con sus
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

El campo de razonamiento SHALL codificarse como cadena: vacío ⇒ no se envía el campo `reasoning`
(el modelo decide); `off` ⇒ `ReasoningSpec::Off`; cualquier otro valor ⇒
`ReasoningSpec::Effort(<nivel>)`. Una temperatura o un `max_tokens` no parseables SHALL caer al
default del rol con un warning; un nivel de razonamiento desconocido SHALL caer a `off` con un
warning.

**Given** una base de datos migrada  
**When** se leen las claves de generación  
**Then** `settings` SHALL contener las doce claves con sus valores iniciales  
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

#### Scenario: Un valor existente se respeta
**Given** `GENERATION_CHAT_TEMPERATURE = 0.9` antes de migrar  
**When** se ejecuta la migración  
**Then** `GENERATION_CHAT_TEMPERATURE` sigue siendo `0.9`

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
