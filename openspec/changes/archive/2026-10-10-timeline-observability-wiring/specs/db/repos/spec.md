## MODIFIED Requirements

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

### Requirement: StatsRepo SHALL provide background LLM usage aggregation

El repositorio SHALL exponer `StatsRepo::background_summary(pool)` devolviendo un `BackgroundStats` por cada origen no-chat (`router`, `archivist`, `consolidator`, `collapse`, `timeline`). Los orígenes sin filas SHALL aparecer con los contadores a cero.

**Given** una tabla `llm_requests` con filas de varios orígenes
**When** se llama a `StatsRepo::background_summary(pool)`
**Then** devuelve un `BackgroundStats` por origen no-chat, cada uno con:
- `kind: String`
- `calls: u64`
- `input_tokens: u64`, `output_tokens: u64`, `total_tokens: u64`
- `total_cost: f64`
- `total_errors: u64` — llamadas con status != 'success'
- `avg_duration_ms: Option<f64>`
**And** las filas de chat no se cuentan.

#### Scenario: Una entrada por origen
**Given** 2 llamadas `router`, 1 `collapse` y ninguna `archivist`, `consolidator` ni `timeline`
**When** `StatsRepo::background_summary(pool)`
**Then** devuelve 5 entradas (una por origen)
**And** `router` tiene `calls=2`, `collapse` tiene `calls=1`, y `archivist`, `consolidator` y `timeline` tienen `calls=0`

#### Scenario: El extractor de timeline se cuenta como un origen más
**Given** 3 llamadas con `kind='timeline'`, una de ellas con `status='error'`
**When** `StatsRepo::background_summary(pool)`
**Then** la entrada de `timeline` tiene `calls=3` y `total_errors=1`
**And** sus tokens y su coste se suman igual que en los demás orígenes

#### Scenario: Sin procesos de fondo
**Given** una tabla `llm_requests` solo con filas de chat
**When** `StatsRepo::background_summary(pool)`
**Then** las cinco entradas tienen `calls=0` y `total_cost=0.0`

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
