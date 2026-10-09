# Spec Delta: persistent-memory-ui

## MODIFIED Requirements

### Requirement: La lectura del estado persistente SHALL exponerse por HTTP

El sistema SHALL exponer `GET /api/persistent-memory`, que devuelve el estado de la Capa C y sus
cotas de tamaño en un objeto JSON con las claves `payload`, `updated_at`, `token_count`,
`budget_tokens`, `ceiling_tokens` e `is_empty`. `payload` SHALL ser el estado ya parseado (objeto
JSON) o `null` si no existe fila; `updated_at` SHALL ser la marca almacenada o `null`;
`token_count` SHALL medirse sobre la forma minificada con el mismo medidor que la Capa C;
`budget_tokens` SHALL leerse de `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` (por defecto 800) y
`ceiling_tokens` SHALL ser su doble. La ausencia de fila SHALL leerse como estado vacío sin crear
la fila.

**Given** una base migrada  
**When** se invoca `GET /api/persistent-memory`  
**Then** SHALL devolverse 200 con el estado y las cotas  
**And** la lectura SHALL NOT crear la fila

#### Scenario: Estado existente se devuelve con sus cotas
**Given** un estado persistente de 120 tokens y un presupuesto de 800  
**When** se lee por HTTP  
**Then** `token_count` es 120, `budget_tokens` es 800 y `ceiling_tokens` es 1600  
**And** `payload` es el objeto almacenado y `updated_at` su marca

#### Scenario: Estado ausente se lee como vacío
**Given** una base migrada sin fila en `persistent_memory`  
**When** se lee por HTTP  
**Then** `payload` es `null`, `updated_at` es `null`, `is_empty` es `true` y `token_count` es 0  
**And** no se ha insertado ninguna fila
