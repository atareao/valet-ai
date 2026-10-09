# Spec Delta: orchestrator

## MODIFIED Requirements

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
