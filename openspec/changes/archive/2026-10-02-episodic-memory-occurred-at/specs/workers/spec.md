# Spec Delta: workers

## ADDED Requirements

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
