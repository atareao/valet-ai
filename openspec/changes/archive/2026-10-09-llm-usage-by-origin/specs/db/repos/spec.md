## MODIFIED Requirements

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

El repositorio SHALL exponer `StatsRepo::by_model(pool)` devolviendo un `ModelStats` por modelo **de chat**, ordenado por coste descendente. Las filas con `kind != 'chat'` SHALL excluirse.

**Given** una tabla `llm_requests` con datos de múltiples modelos
**When** se llama a `StatsRepo::by_model(pool)`
**Then** devuelve `Vec<ModelStats>` con un elemento por modelo, cada uno con:
- `model: String`
- `calls: u64`, `total_tokens: u64`, `total_cost: f64`, `avg_duration_ms: Option<f64>`, `total_cached_tokens: u64`, `total_reasoning_tokens: u64`

#### Scenario: Dos modelos con datos
**Given** 2 llamadas de chat a "gpt-4o", 1 de chat a "claude-3" y 1 del archivist (`kind='archivist'`, modelo "mistralai/mistral-small-24b-instruct-2501")
**When** `StatsRepo::by_model(pool)`
**Then** devuelve 2 filas ordenadas por coste descendente
**And** no aparece el modelo del archivist

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

## ADDED Requirements

### Requirement: StatsRepo SHALL provide background LLM usage aggregation

El repositorio SHALL exponer `StatsRepo::background_summary(pool)` devolviendo un `BackgroundStats` por cada origen no-chat (`router`, `archivist`, `consolidator`, `collapse`). Los orígenes sin filas SHALL aparecer con los contadores a cero.

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
**Given** 2 llamadas `router`, 1 `collapse` y ninguna `archivist` ni `consolidator`
**When** `StatsRepo::background_summary(pool)`
**Then** devuelve 4 entradas (una por origen)
**And** `router` tiene `calls=2`, `collapse` tiene `calls=1`, y `archivist` y `consolidator` tienen `calls=0`

#### Scenario: Sin procesos de fondo
**Given** una tabla `llm_requests` solo con filas de chat
**When** `StatsRepo::background_summary(pool)`
**Then** las cuatro entradas tienen `calls=0` y `total_cost=0.0`
