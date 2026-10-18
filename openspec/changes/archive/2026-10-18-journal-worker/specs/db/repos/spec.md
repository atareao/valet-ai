## MODIFIED Requirements

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
