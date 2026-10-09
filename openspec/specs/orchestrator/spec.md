# orchestrator Specification

## Purpose
Orquestador de Valet: carga del historial de conversación por presupuesto de tokens y almacenamiento de las herramientas usadas como metadatos del mensaje en lugar de incrustarlas en el contenido.

## Requirements

### Requirement: Orchestrator SHALL use list_by_token_budget instead of SessionWindow

El orquestador SHALL cargar el historial con `MessagesRepo::list_by_token_budget()` y SHALL NOT usar `SessionWindow`.

**Given** un orquestador procesando un mensaje  
**When** se construye el array de mensajes para el LLM  
**Then** el historial se carga mediante `MessagesRepo::list_by_token_budget()`  
**And** NO se usa `SessionWindow::get_window()`  
**And** el setting `max_window_tokens` controla el presupuesto de tokens

#### Scenario: Historial se carga desde DB con token budget
**Given** un orquestador con `max_window_tokens = 4000`  
**When** se procesa un mensaje  
**Then** el historial contiene solo los mensajes que caben en 4000 tokens  
**And** no hay referencia a `SessionWindow` en el código del orquestador

#### Scenario: Sin SessionWindow en el estado del orquestador
**Given** un orquestador  
**Then** su constructor NO recibe `Arc<Mutex<SessionWindow>>`  
**And** no tiene campo `session_window`

### Requirement: Orchestrator SHALL store tools_used as metadata, not in content

**Given** el orquestador procesando un mensaje con herramientas  
**When** se completa el ReAct loop y se persiste el mensaje assistant  
**Then** el contenido del mensaje NO SHALL incluir footer de herramientas  
**And** las herramientas SHALL almacenarse en el campo `tools_used`  
**And** el formato SHALL ser `"(N) tool::operation, tool::operation"` donde N es el contador si > 1

#### Scenario: Herramientas únicas sin operación
**Given** herramientas usadas: `["weather", "calendar"]`  
**When** se construye `tools_used`  
**Then** el resultado SHALL ser `"weather, calendar"`

#### Scenario: Misma herramienta con diferentes operaciones
**Given** herramientas usadas: `["calendar::get_events", "calendar::create_event", "weather::get_weather"]`  
**When** se construye `tools_used`  
**Then** el resultado SHALL ser `"calendar::get_events, calendar::create_event, weather::get_weather"`

#### Scenario: Misma herramienta y operación repetida
**Given** herramientas usadas: `["calendar::get_events", "calendar::get_events", "calendar::get_events"]`  
**When** se construye `tools_used`  
**Then** el resultado SHALL ser `"(3) calendar::get_events"`

#### Scenario: Mezcla de únicas y repetidas
**Given** herramientas usadas: `["calendar::get_events", "calendar::get_events", "weather::get_weather", "calendar::create_event"]`  
**When** se construye `tools_used`  
**Then** el resultado SHALL ser `"(2) calendar::get_events, weather::get_weather, calendar::create_event"`

### Requirement: ContextBuilder SHALL be wired with the database pool and embedding provider

En producción, `ContextBuilder` SHALL construirse con `pool: Some(...)` y, cuando los embeddings estén configurados, `provider: Some(...)`, además de `rag_budget_tokens` leído de `RAG_BUDGET_TOKENS`.

**Given** `AppState::new_with_orchestrator()`  
**When** se construye el `ContextBuilder`  
**Then** `pool` SHALL ser `Some(...)`  
**And** `provider` SHALL ser `Some(...)` cuando `EMBEDDING_PROVIDER` y `EMBEDDING_MODEL` estén definidos  
**And** `rag_budget_tokens` SHALL leerse de `RAG_BUDGET_TOKENS`

#### Scenario: Builder de producción tiene pool
**Given** el arranque de producción  
**When** se inspecciona el `ContextBuilder`  
**Then** `pool` es `Some`

#### Scenario: Provider presente cuando hay configuración
**Given** `EMBEDDING_PROVIDER` y `EMBEDDING_MODEL` definidos  
**When** se construye el `ContextBuilder`  
**Then** `provider` es `Some`

#### Scenario: Provider ausente sin configuración
**Given** `EMBEDDING_PROVIDER` sin definir  
**When** se construye el `ContextBuilder`  
**Then** `provider` es `None`

### Requirement: RAG SHALL NOT inject placeholder memories

`ContextBuilder` SHALL devolver `Vec::new()` cuando falte el pool, falte el provider o falle la búsqueda, logueando un warning. SHALL NOT devolver memorias hardcodeadas.

**Given** un `ContextBuilder` sin pool o sin provider, o una búsqueda vectorial que falla  
**When** se construye el contexto  
**Then** `rag_memories` SHALL ser vacío  
**And** SHALL NOT contener valores hardcodeados como `"memory1"` o `"memory2"`  
**And** SHALL loguearse un warning

#### Scenario: Sin pool devuelve vacío
**Given** un `ContextBuilder` con `pool: None`  
**When** se construye el contexto  
**Then** `rag_memories` es vacío

#### Scenario: Sin provider devuelve vacío
**Given** un `ContextBuilder` con `pool: Some` y `provider: None`  
**When** se construye el contexto  
**Then** `rag_memories` es vacío

#### Scenario: Error de búsqueda devuelve vacío con warning
**Given** un `ContextBuilder` con pool y provider, y una búsqueda que falla  
**When** se construye el contexto  
**Then** `rag_memories` es vacío  
**And** se loguea un warning

#### Scenario: Búsqueda real devuelve memorias formateadas
**Given** un `ContextBuilder` con pool y provider, y una fila en `memory` + `vec_memory` cuya similitud supera el umbral  
**When** se construye el contexto  
**Then** `rag_memories` contiene la ficha con su contenido, **sin ninguna fecha** precediéndola  
**And** SHALL NOT contener el prefijo `[{tags}]`  
**And** SHALL NOT contener corchetes de etiquetas vacíos (`[]`)  
**And** SHALL NOT contener ninguna fecha derivada de `memory.created_at` ni de `metadata`

### Requirement: El umbral de parecido SHALL aplicarse a la similitud, nunca al resultado final

El constructor SHALL descartar las fichas cuya **similitud** (`1 - distance` con `distance_metric=cosine`) sea inferior a `SIMILARITY_THRESHOLD`. SHALL NOT aplicar el corte al `final` ya multiplicado por el decaimiento, porque eso hundiría las fichas antiguas por debajo del umbral y volvería inalcanzable la memoria antigua. La antigüedad SHALL decidir el orden, no la pertenencia. Como la KNN de `vec0` ordena por distancia ascendente y `similitud = 1 - distancia`, el corte conserva un prefijo: si la primera candidata supera el umbral hay resultado.

**Given** una búsqueda vectorial con candidatas ordenadas por similitud descendente  
**When** el constructor filtra por `SIMILARITY_THRESHOLD`  
**Then** SHALL conservar solo las fichas cuya similitud sea `>= SIMILARITY_THRESHOLD`  
**And** una ficha antigua con similitud por encima del umbral SHALL seguir presente aunque su `final` sea bajo  
**And** si ninguna candidata supera el umbral, `rag_memories` SHALL ser `vec![]`

#### Scenario: El umbral descarta por similitud baja
**Given** un `SIMILARITY_THRESHOLD = 0.5` y dos candidatas con similitud 0.7 y 0.2  
**When** se construye el contexto  
**Then** `rag_memories` contiene solo la candidata con similitud 0.7

#### Scenario: Una ficha antigua relevante no cae por el decaimiento
**Given** una candidata antigua con similitud 0.9 y muchos días de antigüedad (su `final` es menor que el de otra candidata reciente)  
**When** se construye el contexto  
**Then** la candidata antigua sigue presente porque su similitud supera el umbral  
**And** el decaimiento solo afecta a su posición en el orden

#### Scenario: Sin candidatas por encima del umbral el resultado es vacío
**Given** un `SIMILARITY_THRESHOLD` alto y todas las candidatas con similitud inferior  
**When** se construye el contexto  
**Then** `rag_memories` es `vec![]`

### Requirement: El decaimiento temporal SHALL calcularse en Rust y ordenar los resultados

`MemoryRepo::search_by_vector` SHALL calcular `final = similitud × exp(-λ × días)`, con `λ = ln(2) / MEMORY_HALF_LIFE_DAYS`, **en Rust** (`f64::exp()`), SHALL NOT intentar calcularlo en SQL porque la SQLite que enlaza `sqlx` no expone funciones matemáticas (`exp`, `pow` y `ln` fallan con `no such function`). Los `días` de antigüedad SHALL medirse desde `metadata.last_message_at` (la fecha del mensaje de origen más reciente que compone la ficha); cuando esa clave no exista (fichas escritas antes de este cambio), SHALL caer a `memory.created_at`. `MemoryRepo::search_by_vector` SHALL ordenar las fichas por `final` descendente antes de devolverlas; el presupuesto de tokens SHALL aplicarlo después el constructor. El decaimiento SHALL ordenar, no excluir: la antigüedad decide quién va primero, no quién existe.

**Given** candidatas que superan el umbral, con distinta antigüedad  
**When** `MemoryRepo::search_by_vector` calcula la relevancia final  
**Then** SHALL calcular `final = similitud × exp(-ln2 × días / MEMORY_HALF_LIFE_DAYS)` con `f64::exp()`  
**And** SHALL tomar los `días` desde `metadata.last_message_at`, y de `created_at` si esa clave falta  
**And** SHALL ordenar por `final` descendente  
**And** SHALL NOT ejecutar `exp`/`pow`/`ln` en SQL

#### Scenario: Una ficha más reciente adelanta a una más antigua de igual similitud
**Given** dos candidatas con la misma similitud y distinta antigüedad según `metadata.last_message_at`  
**When** se ordenan por `final`  
**Then** la que tiene el `last_message_at` más reciente aparece antes que la otra

#### Scenario: El cálculo no usa funciones matemáticas de SQLite
**Given** la base de datos que enlaza `sqlx`  
**When** se comprueba la disponibilidad de `exp` en SQL  
**Then** `SELECT exp(-0.7)` falla con `no such function: exp`  
**And** el decaimiento se resuelve con `f64::exp()` en Rust

#### Scenario: La ausencia de last_message_at cae a created_at
**Given** una candidata cuya `metadata` no contiene la clave `last_message_at`  
**When** `MemoryRepo::search_by_vector` calcula su relevancia final  
**Then** SHALL medir los `días` desde `memory.created_at`  
**And** SHALL NOT fallar ni descartar la ficha por la ausencia de la clave

### Requirement: El presupuesto de tokens SHALL cortar con `break`

El constructor SHALL acumular `tokens_count` hasta `RAG_BUDGET_TOKENS` y, cuando la siguiente ficha no quepa, SHALL detener la acumulación con `break`, SHALL NOT continuar buscando una ficha posterior que sí quepa. Continuar metería una ficha menos relevante por ser más pequeña.

**Given** candidatas ordenadas por relevancia final descendente y un `RAG_BUDGET_TOKENS`   
**When** se acumulan `tokens_count`  
**Then** SHALL incluir fichas mientras la suma no supere el presupuesto  
**And** al encontrar la primera ficha que no cabe, SHALL detenerse (`break`), sin evaluar las siguientes

#### Scenario: El presupuesto corta y no salta a la siguiente
**Given** `RAG_BUDGET_TOKENS = 800` y fichas ordenadas de 347, 307, 248, 245, 210, 164 y 138 tokens  
**When** se acumulan tokens  
**Then** `rag_memories` contiene 2 fichas (347 + 307)  
**And** SHALL NOT contener la ficha de 138 tokens aunque quepa tras saltarse la de 248

### Requirement: La estrategia de contexto SHALL NOT gobernar la memoria episódica

La recuperación de memoria episódica SHALL ser ortogonal a `ContextStrategy`: las estrategias `SlidingWindow` y `Historical` SHALL inyectar memoria del mismo modo cuando existan fichas que superen el umbral.

**Given** un `ContextBuilder` con pool y provider, y fichas que superan el umbral  
**When** se construye el contexto con cualquier `ContextStrategy`  
**Then** `rag_memories` SHALL contener las mismas fichas en `SlidingWindow` y `Historical`  
**And** la estrategia SHALL NOT habilitar ni deshabilitar la recuperación

#### Scenario: Un mensaje normal sin override recibe memoria si supera el umbral
**Given** un mensaje que `ContextClassifier::classify()` resuelve como `Override::None` / `ContextStrategy::SlidingWindow`  
**And** fichas cuya similitud supera `SIMILARITY_THRESHOLD`  
**When** se construye el contexto  
**Then** `rag_memories` NO SHALL ser vacío

#### Scenario: Misma memoria en `SlidingWindow` y `Historical`
**Given** un `ContextBuilder` con pool, provider y las mismas fichas sobre el umbral  
**When** se construye el contexto con `SlidingWindow` y con `Historical`  
**Then** ambos SHALL devolver las mismas fichas

### Requirement: MEMORY_KNN_CANDIDATES SHALL acotar el número de candidatas

`MEMORY_KNN_CANDIDATES` SHALL acotar el número de candidatas que trae la KNN de `vec0`, no lo que entra al prompt (eso SHALL hacerlo `RAG_BUDGET_TOKENS`). SHALL ser mayor que el número de fichas que el presupuesto admite, para dar margen al reordenado por antigüedad. Que el resultado no quede vacío depende del corte por umbral, no de este mando (véase «El umbral de parecido SHALL aplicarse a la similitud, nunca al resultado final»).

**Given** un `MEMORY_KNN_CANDIDATES` configurado  
**When** se ejecuta la búsqueda KNN  
**Then** SHALL traer como máximo `MEMORY_KNN_CANDIDATES` candidatas ordenadas por distancia ascendente  
**And** el número de fichas que finalmente entran al prompt SHALL depender de `RAG_BUDGET_TOKENS`

#### Scenario: La KNN acota las candidatas
**Given** 50 fichas en `vec_memory` y `MEMORY_KNN_CANDIDATES = 20`  
**When** se ejecuta la búsqueda  
**Then** SHALL evaluarse como máximo 20 candidatas

#### Scenario: Una candidata sobre el umbral impide el vacío
**Given** `MEMORY_KNN_CANDIDATES = 20` y la primera candidata por distancia supera la similitud mínima  
**When** se construye el contexto  
**Then** `rag_memories` NO SHALL ser vacío
