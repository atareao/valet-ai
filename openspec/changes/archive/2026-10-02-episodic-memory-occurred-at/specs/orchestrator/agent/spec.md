# Spec Delta: orchestrator/agent

## REMOVED Requirements

### Requirement: Anclaje temporal y orden por decaimiento de cada ficha

## ADDED Requirements

### Requirement: Orden por decaimiento de cada ficha

El orden de las fichas dentro del bloque SHALL ser el de relevancia final descendente definido en el requisito «El decaimiento temporal SHALL calcularse en Rust y ordenar los resultados» de `specs/orchestrator/spec.md`, de modo que la antigüedad reordena, pero no excluye. La ficha inyectada SHALL NOT precederse de fecha alguna: el bloque SHALL inyectar el contenido de la ficha tal cual.

**Given** una ficha con `created_at` y `metadata` (con o sin `last_message_at`)  
**When** se compone el bloque de memoria episódica  
**Then** el texto inyectado de esa ficha SHALL ser su contenido, sin ninguna fecha delante  
**And** el orden de las fichas en el bloque SHALL seguir el de relevancia final descendente fijado en el requisito del decaimiento de `orchestrator`

#### Scenario: El texto inyectado NO incluye ninguna fecha
**Given** una ficha con `created_at = "2026-09-29T10:00:00Z"` y `metadata` con `last_message_at`  
**When** se compone el bloque  
**Then** el texto de la ficha NO contiene ninguna fecha derivada de `created_at` ni de `metadata`  
**And** el texto de la ficha es su contenido tal cual

#### Scenario: El orden es por relevancia final
**Given** dos fichas con la misma similitud y distinta antigüedad  
**When** se compone el bloque  
**Then** la ficha más reciente aparece antes que la más antigua

## MODIFIED Requirements

### Requirement: El formato de ficha SHALL NOT depender de metadata ausente

El formato de ficha SHALL NOT incluir el `[tags]`, porque el `EpisodicMemoryWorker` no escribe la clave `tags` en `metadata`.

**Given** una ficha persistida por el `EpisodicMemoryWorker` con `metadata = {"source","primary_message_ids","date_context","first_message_at","last_message_at"}` y sin `tags`  
**When** se formatea esa ficha para el bloque de memoria  
**Then** el texto SHALL NOT contener el prefijo `[tags]` ni corchetes vacíos (`[]`)

#### Scenario: Ficha del worker sin corchetes vacíos
**Given** una ficha con `metadata` sin la clave `tags`  
**When** se formatea la ficha  
**Then** el resultado NO contiene `[]`  
**And** el resultado NO contiene el prefijo `[tags]`
